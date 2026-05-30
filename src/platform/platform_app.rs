//! Platform app handle and recording ticker state.

use std::sync::Arc;

use slint::ComponentHandle;

#[derive(Default)]
pub(crate) struct RecordingTickerState {
    pub(crate) state: crate::RecordingState,
    pub(crate) started_at: Option<std::time::Instant>,
    pub(crate) paused_for: std::time::Duration,
    pub(crate) pause_started: Option<std::time::Instant>,
}

pub(crate) fn spawn_recording_ticker(
    ui_handle: slint::Weak<crate::MainWindow>,
    state: Arc<tokio::sync::Mutex<RecordingTickerState>>,
) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            let s = state.lock().await;
            if s.state == crate::RecordingState::Recording {
                if let Some(started) = s.started_at {
                    let elapsed = started.elapsed().saturating_sub(s.paused_for).as_secs() as i32;
                    let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                        ui.global::<crate::Recording>().set_elapsed_s(elapsed);
                    });
                }
            }
        }
    });
}

#[cfg(target_os = "android")]
pub(crate) type PlatformApp = slint::android::AndroidApp;

#[cfg(not(target_os = "android"))]
#[derive(Clone, Debug, Default)]
pub(crate) struct PlatformApp;
