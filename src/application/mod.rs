//! Application-level orchestration (struct Application, status pipeline,
//! preset defaults).

#![cfg_attr(not(target_os = "android"), allow(dead_code))]

pub mod defaults;
pub mod status;

use std::sync::atomic::{AtomicU64, Ordering};
use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use fcast_sender_sdk::{context::CastContext, device, device::DeviceInfo};
#[cfg(target_os = "android")]
use jni::{objects::JObject, JavaVM};
#[cfg(not(target_os = "android"))]
use mcore::transmission::WhepSink;
use mcore::{DeviceEvent, Event, ShouldQuit};
use slint::ComponentHandle;
use tracing::{debug, error};

use crate::jni_bridge::helpers::{call_java_method_no_args, JavaMethod};
use crate::platform::gst_init::ensure_gstreamer_initialized;
use crate::platform::platform_app::PlatformApp;
use crate::{AppState, BannerSeverity, Bridge, MainWindow, ReceiverItem, GLOB_EVENT_CHAN};
#[cfg(target_os = "android")]
use crate::{set_capture_active, CAST_DESTINATION_ID, CAST_LINK_ID, CAST_SOURCE_ID, FRAME_PAIR};

pub(crate) struct Application {
    ui_weak: slint::Weak<MainWindow>,
    event_tx: tokio::sync::mpsc::UnboundedSender<Event>,
    devices: HashMap<String, DeviceInfo>,
    cast_ctx: CastContext,
    active_device: Option<Arc<dyn device::CastingDevice>>,
    current_device_id: usize,
    local_address: Option<fcast_sender_sdk::IpAddr>,
    android_app: PlatformApp,
    #[cfg(not(target_os = "android"))]
    tx_sink: Option<WhepSink>,
    our_source_url: Option<String>,
    #[cfg(target_os = "android")]
    last_cast_request_scale_width: Option<u32>,
    #[cfg(target_os = "android")]
    last_cast_request_scale_height: Option<u32>,
    #[cfg(target_os = "android")]
    last_cast_request_max_framerate: Option<u32>,
}

impl Application {
    pub async fn new(
        ui_weak: slint::Weak<MainWindow>,
        event_tx: tokio::sync::mpsc::UnboundedSender<Event>,
        android_app: PlatformApp,
    ) -> Result<Self> {
        std::thread::spawn({
            let event_tx = event_tx.clone();
            move || loop {
                match GLOB_EVENT_CHAN.1.recv() {
                    Ok(event) => {
                        if let Err(err) = event_tx.send(event) {
                            error!("Failed to forward event to event loop: {err}");
                            break;
                        }
                    }
                    Err(err) => {
                        error!("Failed to receive event from the global event channel: {err}");
                        break;
                    }
                }
            }
        });

        Ok(Self {
            ui_weak,
            event_tx,
            devices: HashMap::new(),
            cast_ctx: CastContext::new()?,
            active_device: None,
            current_device_id: 0,
            local_address: None,
            android_app,
            #[cfg(not(target_os = "android"))]
            tx_sink: None,
            our_source_url: None,
            #[cfg(target_os = "android")]
            last_cast_request_scale_width: None,
            #[cfg(target_os = "android")]
            last_cast_request_scale_height: None,
            #[cfg(target_os = "android")]
            last_cast_request_max_framerate: None,
        })
    }

    fn banner_generation() -> &'static AtomicU64 {
        static GEN: AtomicU64 = AtomicU64::new(0);
        &GEN
    }

    // Helper for any callback that needs to flash a banner. Centralised so we
    // only own one upgrade-on-event-loop pattern.
    //
    // A monotonic generation counter is bumped on every `set_banner` /
    // `clear_banner` call. `flash_banner` captures the generation it
    // installed and its spawned auto-hide task only clears the banner if
    // that generation is still current - otherwise a newer banner has
    // taken over and the old timer is a no-op. This avoids an earlier
    // flash_banner racing with a later one and hiding it prematurely.
    fn set_banner(
        ui_handle: slint::Weak<MainWindow>,
        msg: String,
        severity: BannerSeverity,
    ) -> u64 {
        let gen = Self::banner_generation().fetch_add(1, Ordering::SeqCst) + 1;
        let _ = ui_handle.upgrade_in_event_loop(move |ui| {
            let bridge = ui.global::<Bridge>();
            bridge.set_banner_message(msg.into());
            bridge.set_banner_severity(severity);
            bridge.set_banner_visible(true);
        });
        gen
    }

    fn clear_banner(ui_handle: slint::Weak<MainWindow>) {
        Self::banner_generation().fetch_add(1, Ordering::SeqCst);
        let _ = ui_handle.upgrade_in_event_loop(move |ui| {
            ui.global::<Bridge>().set_banner_visible(false);
        });
    }

    pub(crate) fn flash_banner(
        ui_handle: slint::Weak<MainWindow>,
        msg: String,
        severity: BannerSeverity,
        duration: std::time::Duration,
    ) {
        let gen = Self::set_banner(ui_handle.clone(), msg, severity);
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            if Self::banner_generation().load(Ordering::SeqCst) == gen {
                // Only clear if no newer set_banner / clear_banner has run.
                Self::clear_banner(ui_handle);
            }
        });
    }

    fn update_receivers_in_ui(&mut self) -> Result<()> {
        let receivers = self
            .devices
            .iter()
            .filter(|(_, info)| !info.addresses.is_empty() && info.port != 0)
            .map(|(name, info)| {
                let first_addr = info
                    .addresses
                    .first()
                    .map(|addr| addr.to_string())
                    .unwrap_or_default();
                let address = match info.addresses.first() {
                    Some(fcast_sender_sdk::IpAddr::V6 { .. }) => {
                        format!("[{first_addr}]:{}", info.port)
                    }
                    Some(_) => format!("{first_addr}:{}", info.port),
                    None => String::new(),
                };
                let kind = match info.protocol {
                    device::ProtocolType::FCast => "fcast",
                };

                ReceiverItem {
                    id: name.clone().into(),
                    name: name.clone().into(),
                    address: address.into(),
                    ip: first_addr.into(),
                    port: i32::from(info.port),
                    kind: kind.into(),
                    is_default: false,
                }
            })
            .collect::<Vec<ReceiverItem>>();
        self.ui_weak.upgrade_in_event_loop(move |ui| {
            let model = std::rc::Rc::new(slint::VecModel::<ReceiverItem>::from_iter(
                receivers.into_iter(),
            ));
            ui.global::<Bridge>().set_devices(model.into());
        })?;

        Ok(())
    }

    fn add_or_update_device(&mut self, device_info: DeviceInfo) -> Result<()> {
        self.devices.insert(device_info.name.clone(), device_info);
        self.update_receivers_in_ui()?;
        Ok(())
    }

    async fn stop_cast(&mut self, stop_playback: bool) -> Result<()> {
        #[cfg(target_os = "android")]
        set_capture_active(false);

        let android_app = self.android_app.clone();
        self.ui_weak.upgrade_in_event_loop(move |_| {
            call_java_method_no_args(&android_app, JavaMethod::StopCapture);
        })?;

        self.our_source_url = None;

        if let Some(active_device) = self.active_device.take() {
            tokio::spawn(async move {
                if stop_playback {
                    debug!("Stopping playback");
                    if let Err(err) = active_device.stop_playback() {
                        error!(?err, "Failed to stop playback");
                    }
                    // NOTE: Instead of waiting for the PlaybackState::Idle event in the main loop we just sleep here
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                debug!("Disconnecting from active device");
                if let Err(err) = active_device.disconnect() {
                    error!(?err, "Failed to disconnect from active device");
                }
            });
        }

        #[cfg(target_os = "android")]
        {
            let _ = migration_runtime::runtime::handle_command(
                migration_runtime::Command::Disconnect {
                    link_id: CAST_LINK_ID.into(),
                },
            );
            for id in [CAST_SOURCE_ID, CAST_DESTINATION_ID] {
                let _ = migration_runtime::runtime::handle_command(
                    migration_runtime::Command::Remove { id: id.into() },
                );
            }
        }

        #[cfg(not(target_os = "android"))]
        if let Some(mut tx_sink) = self.tx_sink.take() {
            tx_sink.shutdown();
        }

        Ok(())
    }

    fn connect_with_device_info(&mut self, device_info: DeviceInfo) -> Result<()> {
        let device = self.cast_ctx.create_device_from_info(device_info);
        self.current_device_id += 1;
        device
            .connect(
                None,
                Arc::new(mcore::DeviceHandler::new(
                    self.current_device_id,
                    self.event_tx.clone(),
                )),
                1000,
            )
            .unwrap();
        self.active_device = Some(device);
        self.ui_weak.upgrade_in_event_loop(|ui| {
            ui.global::<Bridge>()
                .invoke_change_state(AppState::Connecting);
        })?;

        Ok(())
    }

    /// Returns `true` if the event loop should quit
    async fn handle_event(&mut self, event: Event) -> Result<ShouldQuit> {
        debug!("Handling event: {event:?}");

        #[allow(unreachable_patterns)]
        match event {
            Event::EndSession { .. } => {
                self.ui_weak.upgrade_in_event_loop(|ui| {
                    // Phase 8 (deferred): clear Bridge.status-items here.
                    ui.global::<Bridge>()
                        .invoke_change_state(AppState::Disconnected);
                })?;

                self.stop_cast(true).await?;
            }
            Event::ConnectToDevice(device_name) => {
                if let Some(device_info) = self.devices.get(&device_name) {
                    self.connect_with_device_info(device_info.clone())?;
                } else {
                    error!("No device with name `{device_name}` found");
                }
            }
            Event::SignallerStarted {
                bound_port_v4,
                bound_port_v6,
            } => {
                let Some(addr) = self.local_address.as_ref() else {
                    error!("Local address is missing");
                    return Ok(ShouldQuit::No);
                };
                let bound_port = match addr {
                    fcast_sender_sdk::IpAddr::V4 { .. } => bound_port_v4,
                    fcast_sender_sdk::IpAddr::V6 { .. } => bound_port_v6,
                };

                let (content_type, url) =
                    mcore::transmission::build_whep_play_msg(addr.into(), bound_port);

                debug!(content_type, url, "Sending play message");
                self.our_source_url = Some(url.clone());

                match self.active_device.as_ref() {
                    Some(device) => {
                        device.load(device::LoadRequest::Url {
                            content_type,
                            url,
                            resume_position: None,
                            speed: None,
                            volume: None,
                            metadata: None,
                            request_headers: None,
                        })?;
                    }
                    None => error!("Active device is missing, cannot send play message"),
                }

                // self.ui_weak.upgrade_in_event_loop(|ui| {
                //     ui.global::<Bridge>().invoke_change_state(AppState::Casting);
                // })?;
            }
            Event::Quit => return Ok(ShouldQuit::Yes),
            Event::DeviceAvailable(device_info) => self.add_or_update_device(device_info)?,
            Event::DeviceRemoved(device_name) => {
                if self.devices.remove(&device_name).is_some() {
                    self.update_receivers_in_ui()?;
                } else {
                    debug!(device_name, "Tried to remove device but it was not found");
                }
            }
            Event::DeviceChanged(device_info) => self.add_or_update_device(device_info)?,
            Event::FromDevice { id, event } => {
                if id != self.current_device_id {
                    debug!(
                        "Got message from old device (id: {id} current: {})",
                        self.current_device_id
                    );
                } else {
                    match event {
                        DeviceEvent::StateChanged(device_connection_state) => {
                            if let device::DeviceConnectionState::Connected { local_addr, .. } =
                                device_connection_state
                            {
                                self.local_address = Some(local_addr);

                                self.ui_weak.upgrade_in_event_loop(|ui| {
                                    ui.global::<Bridge>()
                                        .invoke_change_state(AppState::SelectingSettings);
                                })?;
                            }
                        }
                        DeviceEvent::SourceChanged(new_source) => {
                            #[cfg(target_os = "android")]
                            let should_monitor_source = self.our_source_url.is_some();
                            #[cfg(not(target_os = "android"))]
                            let should_monitor_source = self.tx_sink.is_some();

                            if should_monitor_source {
                                if let fcast_sender_sdk::device::Source::Url { ref url, .. } =
                                    new_source
                                {
                                    if Some(url) != self.our_source_url.as_ref() {
                                        // At this point the receiver has stopped playing our stream
                                        debug!(
                                            ?new_source,
                                            "The source on the receiver changed, disconnecting"
                                        );

                                        self.ui_weak.upgrade_in_event_loop(|ui| {
                                            // Phase 8 (deferred): clear Bridge.status-items here.
                                            ui.global::<Bridge>()
                                                .invoke_change_state(AppState::Disconnected);
                                        })?;

                                        self.stop_cast(false).await?;
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            #[cfg(target_os = "android")]
            Event::CaptureStopped => {
                set_capture_active(false);
            }
            #[cfg(target_os = "android")]
            Event::CaptureCancelled => {
                set_capture_active(false);
                self.ui_weak.upgrade_in_event_loop(|ui| {
                    // Phase 8 (deferred): clear Bridge.status-items here.
                    ui.global::<Bridge>()
                        .invoke_change_state(AppState::Disconnected);
                })?;

                self.stop_cast(false).await?;
            }
            #[cfg(target_os = "android")]
            Event::QrScanResult(result) => {
                match fcast_sender_sdk::device::device_info_from_url(result) {
                    Some(device_info) => {
                        self.connect_with_device_info(device_info)?;
                    }
                    None => {
                        error!("QR code scan result is not a valid device");
                    }
                }
            }
            #[cfg(target_os = "android")]
            Event::CaptureStarted => {
                set_capture_active(true);
                self.our_source_url = None;

                if let Err(err) = migration_runtime::runtime::start_graph_runtime(
                    migration_runtime::runtime::RuntimeHandles {
                        frame_pair: FRAME_PAIR.clone(),
                    },
                ) {
                    error!(?err, "Failed to start migrated graph runtime");
                    self.stop_cast(false).await?;
                    return Ok(ShouldQuit::No);
                }

                let scale_width = self.last_cast_request_scale_width.unwrap_or(1280);
                let scale_height = self.last_cast_request_scale_height.unwrap_or(720);
                let fps = self.last_cast_request_max_framerate.unwrap_or(30);

                let commands = [
                    migration_runtime::Command::CreateScreenCaptureSource {
                        id: CAST_SOURCE_ID.into(),
                        width: scale_width,
                        height: scale_height,
                        fps,
                    },
                    migration_runtime::Command::CreateDestination {
                        id: CAST_DESTINATION_ID.into(),
                        family: migration_runtime::DestinationFamily::Whep { server_port: 0 },
                        audio: false,
                        video: true,
                    },
                    migration_runtime::Command::Connect {
                        link_id: CAST_LINK_ID.into(),
                        src_id: CAST_SOURCE_ID.into(),
                        sink_id: CAST_DESTINATION_ID.into(),
                        audio: false,
                        video: true,
                        config: None,
                    },
                    migration_runtime::Command::Start {
                        id: CAST_DESTINATION_ID.into(),
                        cue_time: None,
                        end_time: None,
                    },
                    migration_runtime::Command::Start {
                        id: CAST_SOURCE_ID.into(),
                        cue_time: None,
                        end_time: None,
                    },
                ];

                for command in commands {
                    if let migration_runtime::CommandResult::Error(err) =
                        migration_runtime::runtime::handle_command(command)
                    {
                        error!(?err, "Failed to build unified cast graph");
                        self.stop_cast(false).await?;
                        return Ok(ShouldQuit::No);
                    }
                }

                let event_tx = self.event_tx.clone();
                tokio::spawn(async move {
                    for _ in 0..200 {
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        let info = migration_runtime::runtime::handle_command(
                            migration_runtime::Command::GetInfo {
                                id: Some(CAST_DESTINATION_ID.into()),
                            },
                        );

                        if let migration_runtime::CommandResult::Info(snapshot) = info {
                            if let Some(migration_runtime::NodeInfo::Destination(destination)) =
                                snapshot.nodes.get(CAST_DESTINATION_ID)
                            {
                                if let (Some(bound_port_v4), Some(bound_port_v6)) =
                                    (destination.bound_port_v4, destination.bound_port_v6)
                                {
                                    let _ = event_tx.send(Event::SignallerStarted {
                                        bound_port_v4,
                                        bound_port_v6,
                                    });
                                    return;
                                }
                            }
                        }
                    }

                    error!("WHEP destination did not publish bound ports within 20s");
                });

                let _receiver_name = self
                    .active_device
                    .as_ref()
                    .map(|d| d.name())
                    .unwrap_or_default();
                let _encoder_name = "Hardware"; // Blocked by P0-1: Placeholder until encoder selection works
                let _network_info = self
                    .local_address
                    .as_ref()
                    .map(|a| a.to_string())
                    .unwrap_or_default();
                // Phase 8 (deferred): wire Bridge.status-items here from
                // build_status_items(&_receiver_name, _encoder_name, &_network_info).

                self.ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Bridge>().invoke_change_state(AppState::Casting);
                })?;
            }
            #[cfg(target_os = "android")]
            Event::StartCast {
                scale_width,
                scale_height,
                max_framerate,
            } => {
                self.last_cast_request_scale_width = Some(scale_width);
                self.last_cast_request_scale_height = Some(scale_height);
                self.last_cast_request_max_framerate = Some(max_framerate);

                let android_app = self.android_app.clone();
                self.ui_weak.upgrade_in_event_loop(move |ui| {
                    let vm = unsafe {
                        let ptr = android_app.vm_as_ptr() as *mut jni::sys::JavaVM;
                        assert!(!ptr.is_null(), "JavaVM ptr is null");
                        JavaVM::from_raw(ptr).unwrap()
                    };
                    let activity = unsafe {
                        let ptr = android_app.activity_as_ptr() as *mut jni::sys::_jobject;
                        assert!(!ptr.is_null(), "Activity ptr is null");
                        JObject::from_raw(ptr)
                    };

                    let scale_width = scale_width as jni::sys::jint;
                    let scale_height = scale_height as jni::sys::jint;
                    let max_framerate = max_framerate as jni::sys::jint;

                    match vm.get_env() {
                        Ok(mut env) => match env.call_method(
                            activity,
                            "startScreenCapture",
                            "(III)V",
                            &[
                                scale_width.into(),
                                scale_height.into(),
                                max_framerate.into(),
                            ],
                        ) {
                            Ok(_) => (),
                            Err(err) => error!(
                                ?err,
                                method = "startScreenCapture",
                                "Failed to call java method"
                            ),
                        },
                        Err(err) => error!(?err, "Failed to get env from VM"),
                    }

                    ui.global::<Bridge>()
                        .invoke_change_state(AppState::WaitingForMedia);
                })?;
            }
            #[cfg(not(target_os = "android"))]
            Event::StartCast {
                scale_width: _,
                scale_height: _,
                max_framerate: _,
                ..
            } => {
                debug!("Ignoring StartCast in non-android build of android-sender");
            }
            _ => {}
        }

        Ok(ShouldQuit::No)
    }

    pub async fn run_event_loop(
        mut self,
        mut event_rx: tokio::sync::mpsc::UnboundedReceiver<Event>,
    ) -> Result<()> {
        tracing_gstreamer::integrate_events();
        gst::log::remove_default_log_function();
        gst::log::set_default_threshold(gst::DebugLevel::Fixme);
        ensure_gstreamer_initialized()
            .map_err(|err| anyhow::anyhow!("Failed to initialize GStreamer: {err}"))?;
        debug!("GStreamer version: {:?}", gst::version());
        // PHASE-9: start the migration runtime on demand at its call sites.
        // `shutdown_graph_runtime()` below remains safe if nothing started it.

        // self.add_or_update_device(fcast_sender_sdk::device::DeviceInfo::fcast("Localhost for android emulator".to_owned(), vec![fcast_sender_sdk::IpAddr::v4(10, 0, 2, 2)], 46899))?;

        loop {
            let Some(event) = event_rx.recv().await else {
                debug!("No more events");
                break;
            };

            if self.handle_event(event).await? == ShouldQuit::Yes {
                break;
            }
        }

        debug!("Quitting event loop");
        if let Err(err) = migration_runtime::runtime::shutdown_graph_runtime() {
            error!(?err, "Failed to shut down migrated graph runtime");
        }

        Ok(())
    }
}
