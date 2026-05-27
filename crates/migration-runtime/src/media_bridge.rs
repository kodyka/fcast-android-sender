use gst_app::{AppSink, AppSinkCallbacks, AppSrc};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Forwards samples from one producer `appsink` to many consumer `appsrc`s.
#[derive(Debug, Default)]
pub struct StreamBridge {
    consumers: Arc<Mutex<HashMap<String, AppSrc>>>,
    attached_sink: Option<AppSink>,
    last_caps: Arc<Mutex<Option<gst::Caps>>>,
}

impl StreamBridge {
    pub fn add_consumer(&mut self, consumer_id: &str, consumer: &AppSrc) {
        self.consumers
            .lock()
            .insert(consumer_id.to_string(), consumer.clone());
        if let Some(caps) = self.last_caps.lock().as_ref().cloned() {
            consumer.set_caps(Some(&caps));
        }
    }

    pub fn remove_consumer(&mut self, consumer_id: &str) {
        self.consumers.lock().remove(consumer_id);
    }

    pub fn clear(&mut self) {
        self.consumers.lock().clear();
        *self.last_caps.lock() = None;
        if let Some(sink) = self.attached_sink.take() {
            sink.set_callbacks(AppSinkCallbacks::builder().build());
        }
    }

    pub fn has_consumers(&self) -> bool {
        !self.consumers.lock().is_empty()
    }

    pub fn attach_sink(&mut self, sink: &AppSink) {
        if self
            .attached_sink
            .as_ref()
            .is_some_and(|current| current == sink)
        {
            return;
        }

        if let Some(old_sink) = self.attached_sink.take() {
            old_sink.set_callbacks(AppSinkCallbacks::builder().build());
        }

        *self.last_caps.lock() = None;
        Self::configure_callbacks(sink, self.consumers.clone(), self.last_caps.clone());
        self.attached_sink = Some(sink.clone());
    }

    fn configure_callbacks(
        sink: &AppSink,
        consumers: Arc<Mutex<HashMap<String, AppSrc>>>,
        last_caps: Arc<Mutex<Option<gst::Caps>>>,
    ) {
        let consumers_for_samples = consumers.clone();
        let consumers_for_eos = consumers.clone();
        let last_caps_for_samples = last_caps;

        sink.set_callbacks(
            AppSinkCallbacks::builder()
                .new_sample(move |appsink| {
                    let sample = appsink.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                    let caps = sample.caps().map(|caps| caps.to_owned());
                    let Some(buffer) = sample.buffer() else {
                        return Ok(gst::FlowSuccess::Ok);
                    };

                    let snapshot = {
                        let guard = consumers_for_samples.lock();
                        guard
                            .iter()
                            .map(|(id, appsrc)| (id.clone(), appsrc.clone()))
                            .collect::<Vec<_>>()
                    };

                    if snapshot.is_empty() {
                        return Ok(gst::FlowSuccess::Ok);
                    }

                    let caps_to_set = caps.as_ref().and_then(|incoming| {
                        let mut last = last_caps_for_samples.lock();
                        if last.as_ref() == Some(incoming) {
                            None
                        } else {
                            *last = Some(incoming.clone());
                            Some(incoming.clone())
                        }
                    });

                    let mut stale = Vec::new();
                    for (consumer_id, appsrc) in snapshot {
                        if let Some(caps) = caps_to_set.as_ref() {
                            appsrc.set_caps(Some(caps));
                        }

                        if appsrc.push_buffer(buffer.copy()).is_err() {
                            stale.push(consumer_id);
                        }
                    }

                    if !stale.is_empty() {
                        let mut guard = consumers_for_samples.lock();
                        for consumer_id in stale {
                            guard.remove(&consumer_id);
                        }
                    }

                    Ok(gst::FlowSuccess::Ok)
                })
                .eos(move |_appsink| {
                    let snapshot = {
                        let guard = consumers_for_eos.lock();
                        guard.values().cloned().collect::<Vec<_>>()
                    };
                    for appsrc in snapshot {
                        let _ = appsrc.end_of_stream();
                    }
                })
                .build(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gst::prelude::*;
    use std::sync::Once;

    fn ensure_gst_init() {
        static INIT: Once = Once::new();
        INIT.call_once(|| {
            gst::init().unwrap();
        });
    }

    fn make_appsrc(name: &str) -> AppSrc {
        gst::ElementFactory::make("appsrc")
            .name(name)
            .build()
            .unwrap()
            .downcast::<AppSrc>()
            .unwrap()
    }

    fn make_appsink(name: &str) -> AppSink {
        gst::ElementFactory::make("appsink")
            .name(name)
            .build()
            .unwrap()
            .downcast::<AppSink>()
            .unwrap()
    }

    #[test]
    fn add_remove_and_clear_manage_consumer_lifecycle() {
        ensure_gst_init();

        let mut bridge = StreamBridge::default();
        let src = make_appsrc("bridge-consumer-1");
        let sink = make_appsink("bridge-sink-1");

        assert!(!bridge.has_consumers());
        bridge.add_consumer("consumer-1", &src);
        assert!(bridge.has_consumers());
        assert_eq!(bridge.consumers.lock().len(), 1);

        bridge.attach_sink(&sink);
        assert!(bridge.attached_sink.is_some());

        bridge.remove_consumer("consumer-1");
        assert!(!bridge.has_consumers());

        bridge.add_consumer("consumer-2", &src);
        bridge.clear();
        assert!(!bridge.has_consumers());
        assert!(bridge.attached_sink.is_none());
        assert!(bridge.last_caps.lock().is_none());
    }

    #[test]
    fn add_consumer_applies_cached_caps_to_new_consumer() {
        ensure_gst_init();

        let mut bridge = StreamBridge::default();
        let src = make_appsrc("bridge-consumer-2");
        let caps = gst::Caps::builder("video/x-raw")
            .field("format", "RGB")
            .field("width", 1280i32)
            .field("height", 720i32)
            .build();
        *bridge.last_caps.lock() = Some(caps.clone());

        bridge.add_consumer("consumer-1", &src);
        assert_eq!(src.caps(), Some(caps));
    }

    #[test]
    fn attach_sink_replaces_old_sink_and_resets_cached_caps() {
        ensure_gst_init();

        let mut bridge = StreamBridge::default();
        let sink1 = make_appsink("bridge-sink-2");
        let sink2 = make_appsink("bridge-sink-3");

        bridge.attach_sink(&sink1);
        assert_eq!(bridge.attached_sink.as_ref(), Some(&sink1));

        *bridge.last_caps.lock() = Some(gst::Caps::builder("audio/x-raw").build());
        bridge.attach_sink(&sink2);

        assert_eq!(bridge.attached_sink.as_ref(), Some(&sink2));
        assert!(bridge.last_caps.lock().is_none());
    }
}
