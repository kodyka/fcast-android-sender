use anyhow::Result;
use fcast_sender_sdk::{context::CastContext, device, device::DeviceInfo};
#[cfg(target_os = "android")]
use jni::{
    objects::{JByteBuffer, JObject, JString},
    JavaVM,
};
#[cfg(not(target_os = "android"))]
use mcore::transmission::WhepSink;
use mcore::{DeviceEvent, Event, ShouldQuit};
use parking_lot::Mutex;
use std::sync::atomic::AtomicU64;
#[cfg(not(target_os = "android"))]
use std::sync::atomic::Ordering;
#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::{collections::HashMap, sync::Arc};
use tracing::{debug, error};

pub mod app;
pub mod application;
pub mod command;
pub mod config;
pub mod jni_bridge;
pub mod log_ring;
pub mod platform;
pub mod secret;

mod backend;
mod gstpop_service;
mod migration_service;

#[cfg(target_os = "android")]
use crate::application::defaults::{default_presets, default_quick_actions};
#[cfg(target_os = "android")]
use crate::command::http_runner::start_migrated_command_server;
#[cfg(target_os = "android")]
use crate::command::legacy_tests::{
    log_ui_test_status, run_graph_smoke_test, run_legacy_http_crossfade_test,
    run_legacy_http_getinfo_test,
};
#[cfg(any(test, target_os = "android"))]
use crate::command::legacy_tests::migration_test_log_name;
#[cfg(target_os = "android")]
use crate::jni_bridge::helpers::handle_back_request;
use crate::jni_bridge::helpers::{call_java_method_no_args, JavaMethod};
#[cfg(target_os = "android")]
use crate::jni_bridge::helpers::resolve_android_files_dir;
use crate::platform::gst_init::ensure_gstreamer_initialized;
#[cfg(target_os = "android")]
use crate::platform::panel_stack::PanelStack;
use crate::platform::platform_app::PlatformApp;
#[cfg(target_os = "android")]
use crate::platform::platform_app::{spawn_recording_ticker, RecordingTickerState};

lazy_static::lazy_static! {
    pub static ref GLOB_EVENT_CHAN: (crossbeam_channel::Sender<Event>, crossbeam_channel::Receiver<Event>)
        = crossbeam_channel::bounded(2);
    pub static ref FRAME_PAIR: Arc<migration_runtime::FramePair> = migration_runtime::FramePair::new();
    pub static ref FRAME_POOL: Mutex<gst_video::VideoBufferPool> = Mutex::new(gst_video::VideoBufferPool::new());
}

#[cfg(target_os = "android")]
lazy_static::lazy_static! {
    static ref ANDROID_UI: Mutex<Option<slint::Weak<MainWindow>>> = Mutex::new(None);
    static ref ANDROID_APP: Mutex<Option<PlatformApp>> = Mutex::new(None);
    /// Dedicated runtime for gst-pop service JNI calls. Separate from the
    /// Slint event-loop runtime so binder-thread calls never block the UI.
    pub(crate) static ref HOST_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("gstpop-host")
            .build()
            .expect("build HOST_RUNTIME");
}

#[cfg(target_os = "android")]
pub(crate) struct AndroidCtx {
    pub vm: jni::JavaVM,
    pub activity: jni::objects::JObject<'static>,
}

#[cfg(target_os = "android")]
pub(crate) fn android_context() -> anyhow::Result<AndroidCtx> {
    let app = ANDROID_APP
        .lock()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("android app not installed"))?;
    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    let activity_ptr = app.activity_as_ptr() as *mut jni::sys::_jobject;
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr)? };
    let activity = unsafe { jni::objects::JObject::from_raw(activity_ptr) };
    Ok(AndroidCtx { vm, activity })
}

#[cfg(target_os = "android")]
static CAPTURE_ACTIVE: AtomicBool = AtomicBool::new(false);

#[cfg(target_os = "android")]
const CAST_SOURCE_ID: &str = "cast-screen-1";
#[cfg(target_os = "android")]
const CAST_DESTINATION_ID: &str = "cast-whep-1";
#[cfg(target_os = "android")]
const CAST_LINK_ID: &str = "cast-link-1";

slint::include_modules!();

macro_rules! log_err {
    ($res:expr, $msg: expr) => {
        if let Err(err) = ($res) {
            error!(?err, $msg);
        }
    };
}

#[cfg(target_os = "android")]
const LEGACY_COMMAND_BIND_ADDR: &str = "0.0.0.0:8080";

#[cfg(target_os = "android")]
fn set_capture_active(active: bool) {
    CAPTURE_ACTIVE.store(active, Ordering::SeqCst);
    if !active {
        let mut frame = FRAME_PAIR.frame.lock();
        *frame = None;
        FRAME_PAIR.cond.notify_all();
    }
}

struct Application {
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

    // Helper for any callback that needs to flash a banner. Centralised so we
    // only own one upgrade-on-event-loop pattern.
    //
    // A monotonic generation counter is bumped on every `set_banner` /
    // `clear_banner` call. `flash_banner` captures the generation it
    // installed and its spawned auto-hide task only clears the banner if
    // that generation is still current — otherwise a newer banner has
    // taken over and the old timer is a no-op. This avoids an earlier
    // flash_banner racing with a later one and hiding it prematurely.
    fn banner_generation() -> &'static AtomicU64 {
        static GEN: AtomicU64 = AtomicU64::new(0);
        &GEN
    }

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

    fn flash_banner(
        ui_handle: slint::Weak<MainWindow>,
        msg: String,
        severity: BannerSeverity,
        duration: std::time::Duration,
    ) {
        let gen = Self::set_banner(ui_handle.clone(), msg, severity);
        tokio::spawn(async move {
            tokio::time::sleep(duration).await;
            // Only clear if no newer set_banner / clear_banner has run.
            if Self::banner_generation().load(Ordering::SeqCst) == gen {
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
                    log_err!(active_device.stop_playback(), "Failed to stop playback");
                    // NOTE: Instead of waiting for the PlaybackState::Idle event in the main loop we just sleep here
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
                debug!("Disconnecting from active device");
                log_err!(
                    active_device.disconnect(),
                    "Failed to disconnect from active device"
                );
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

// TODO: handle errs
#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: PlatformApp) {
    let vm = unsafe {
        let ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
        assert!(!ptr.is_null(), "JavaVM ptr is null");
        jni::JavaVM::from_raw(ptr).unwrap()
    };
    crate::app::init(crate::app::App::production(vm));
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );

    let app_clone = app.clone();

    if let Ok(files_dir) = resolve_android_files_dir(&app_clone) {
        if let Err(e) = crate::config::migration::migrate_config_file(&files_dir) {
            tracing::warn!("migration config migrate failed: {e}");
        }
    }

    slint::android::init(app).unwrap();

    let ui = MainWindow::new().unwrap();
    *ANDROID_UI.lock() = Some(ui.as_weak());
    *ANDROID_APP.lock() = Some(app_clone.clone());

    // Cached snapshot. Re-pushed in full whenever any signal changes.
    #[derive(Clone, Default)]
    struct StatusSnapshot {
        network_label: String,
        thermal_label: String,
        battery_pct: i32,
        charging: bool,
    }

    fn push_status(ui_handle: slint::Weak<MainWindow>, snap: StatusSnapshot) {
        let _ = ui_handle.upgrade_in_event_loop(move |ui| {
            let bridge = ui.global::<Bridge>();
            let items = vec![
                StatusItem {
                    label: "network".into(),
                    value: snap.network_label.into(),
                    severity: StatusSeverity::Info,
                    icon_glyph: "📶".into(),
                },
                StatusItem {
                    label: "thermal".into(),
                    value: snap.thermal_label.clone().into(),
                    severity: match snap.thermal_label.as_str() {
                        "Critical" => StatusSeverity::Error,
                        "Serious" => StatusSeverity::Warning,
                        _ => StatusSeverity::Info,
                    },
                    icon_glyph: if snap.thermal_label == "Critical" {
                        "🔥".into()
                    } else {
                        "🌡".into()
                    },
                },
                StatusItem {
                    label: "battery".into(),
                    value: format!("{}%", snap.battery_pct).into(),
                    severity: if snap.battery_pct < 20 {
                        StatusSeverity::Error
                    } else {
                        StatusSeverity::Info
                    },
                    icon_glyph: if snap.charging {
                        "⚡".into()
                    } else {
                        "🔋".into()
                    },
                },
            ];
            let model: slint::ModelRc<StatusItem> =
                std::rc::Rc::new(slint::VecModel::from(items)).into();
            bridge.set_status_items(model);
        });
    }

    let show_debug = cfg!(debug_assertions);
    ui.global::<Bridge>().set_show_debug(show_debug);
    ui.global::<Bridge>()
        .set_app_version(env!("CARGO_PKG_VERSION").into());

    let bar_actions: Arc<Mutex<Vec<QuickAction>>> = Arc::new(Mutex::new(default_quick_actions()));
    // Initial push is synchronous — we still hold the strong `ui` handle,
    // so the control bar is populated before `ui.run()` paints the first
    // frame. Subsequent mutations from callbacks use `push_bar()` which
    // hops through `upgrade_in_event_loop` because they only have a weak.
    {
        let snapshot = bar_actions.lock().clone();
        ui.global::<Bridge>()
            .set_quick_actions(std::rc::Rc::new(slint::VecModel::from(snapshot)).into());
    }
    let push_bar = {
        let bar_actions = bar_actions.clone();
        let ui_weak = ui.as_weak();
        move || {
            let snapshot = bar_actions.lock().clone();
            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                ui.global::<Bridge>()
                    .set_quick_actions(std::rc::Rc::new(slint::VecModel::from(snapshot)).into());
            });
        }
    };

    ui.global::<Bridge>().on_move_bar_action({
        let bar_actions = bar_actions.clone();
        let push = push_bar.clone();
        move |from, to| {
            let mut g = bar_actions.lock();
            if let (Ok(from_u), Ok(to_u)) = (usize::try_from(from), usize::try_from(to)) {
                if from_u < g.len() && to_u < g.len() && from_u != to_u {
                    let item = g.remove(from_u);
                    g.insert(to_u, item);
                }
            }
            drop(g);
            push();
        }
    });

    ui.global::<Bridge>().on_set_bar_action_enabled({
        let bar_actions = bar_actions.clone();
        let push = push_bar.clone();
        move |idx, enabled| {
            let mut g = bar_actions.lock();
            if let Ok(i) = usize::try_from(idx) {
                if let Some(a) = g.get_mut(i) {
                    a.enabled = enabled;
                }
            }
            drop(g);
            push();
        }
    });

    ui.global::<Bridge>().on_save_bar_actions({
        let _bar_actions = bar_actions.clone();
        let push = push_bar.clone();
        move || {
            // Phase 11: persist to DataStore via JNI here.
            // For now, just re-push the in-memory state.
            push();
        }
    });

    let history: Arc<Mutex<Vec<crate::CastHistoryEntry>>> = Arc::new(Mutex::new(vec![
        crate::CastHistoryEntry {
            id: "h1".into(),
            receiver: "Living Room TV".into(),
            started_at: "Today 12:34".into(),
            duration_s: 765,
            status: "Completed".into(),
        },
        crate::CastHistoryEntry {
            id: "h2".into(),
            receiver: "Bedroom TV".into(),
            started_at: "Yesterday 22:10".into(),
            duration_s: 68,
            status: "Cancelled".into(),
        },
        crate::CastHistoryEntry {
            id: "h3".into(),
            receiver: "Office Mac".into(),
            started_at: "Yesterday 09:00".into(),
            duration_s: 1920,
            status: "Completed".into(),
        },
    ]));

    use slint::Model;
    use std::sync::atomic::AtomicUsize;
    let macros: Arc<std::sync::Mutex<Vec<Macro>>> = Arc::new(std::sync::Mutex::new(vec![]));
    let draft_macro_steps: Arc<std::sync::Mutex<Vec<MacroStep>>> =
        Arc::new(std::sync::Mutex::new(vec![]));
    let next_macro_id = Arc::new(AtomicUsize::new(0));

    // Both push_* helpers apply synchronously via `upgrade()` rather than
    // `upgrade_in_event_loop()`. Every caller is a Slint callback (e.g.
    // on_save_macro, on_draft_move_step) that already runs on the UI
    // thread, so deferral is unnecessary — and would let the consumer
    // page render one frame with stale data when a panel switch happens
    // immediately after the callback (see on_load_draft_macro for the
    // same rationale).
    let push_macros = {
        let macros = macros.clone();
        let ui_weak = ui.as_weak();
        move || {
            let snap = macros.lock().unwrap().clone();
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<Bridge>()
                    .set_macros(std::rc::Rc::new(slint::VecModel::from(snap)).into());
            }
        }
    };
    push_macros();

    let push_draft_steps = {
        let draft_macro_steps = draft_macro_steps.clone();
        let ui_weak = ui.as_weak();
        move || {
            let snap = draft_macro_steps.lock().unwrap().clone();
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<Bridge>()
                    .set_draft_macro_steps(std::rc::Rc::new(slint::VecModel::from(snap)).into());
            }
        }
    };
    push_draft_steps();

    ui.global::<Bridge>().on_save_macro({
        let macros = macros.clone();
        let next_id = next_macro_id.clone();
        let push = push_macros.clone();
        move |id, name, steps, enabled| {
            let steps_vec: Vec<MacroStep> = steps.iter().collect();
            let mut g = macros.lock().unwrap();
            if id.is_empty() {
                let new_id = format!("macro-{}", next_id.fetch_add(1, Ordering::Relaxed));
                g.push(Macro {
                    id: new_id.into(),
                    name: name.into(),
                    steps: std::rc::Rc::new(slint::VecModel::from(steps_vec)).into(),
                    enabled,
                });
            } else if let Some(m) = g.iter_mut().find(|m| m.id == id) {
                m.name = name.into();
                m.enabled = enabled;
                m.steps = std::rc::Rc::new(slint::VecModel::from(steps_vec)).into();
            }
            drop(g);
            push();
        }
    });

    ui.global::<Bridge>().on_delete_macro({
        let macros = macros.clone();
        let push = push_macros.clone();
        move |id| {
            macros.lock().unwrap().retain(|m| m.id != id);
            push();
        }
    });

    ui.global::<Bridge>().on_run_macro({
        let macros = macros.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            let snap = macros.lock().unwrap().iter().find(|m| m.id == id).cloned();
            let Some(m) = snap else {
                Application::flash_banner(
                    ui_weak.clone(),
                    format!("Macro {} not found", id),
                    BannerSeverity::Error,
                    std::time::Duration::from_secs(3),
                );
                return;
            };
            // Phase 11: real macro engine (iterate m.steps, dispatch each via on_invoke_action).
            Application::flash_banner(
                ui_weak.clone(),
                format!("Ran macro: {}", m.name),
                BannerSeverity::Success,
                std::time::Duration::from_secs(2),
            );
        }
    });

    ui.global::<Bridge>().on_load_draft_macro({
        let macros = macros.clone();
        let draft_macro_steps = draft_macro_steps.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            let mut draft_name = "".to_string();
            let mut draft_enabled = true;
            let steps_snap: Vec<MacroStep> = {
                let mut draft_g = draft_macro_steps.lock().unwrap();
                if id.is_empty() {
                    draft_g.clear();
                } else {
                    let mg = macros.lock().unwrap();
                    if let Some(m) = mg.iter().find(|m| m.id == id) {
                        *draft_g = m.steps.iter().collect();
                        draft_name = m.name.to_string();
                        draft_enabled = m.enabled;
                    } else {
                        draft_g.clear();
                    }
                }
                draft_g.clone()
            };
            // Slint callbacks run on the UI thread, so we can apply the
            // draft state synchronously. This matters because callers
            // (macros_page.slint) switch to Panel.macro-edit immediately
            // after this callback returns — a deferred upgrade_in_event_loop
            // would let MacroEditPage render one frame with stale values.
            if let Some(ui) = ui_weak.upgrade() {
                let bridge = ui.global::<Bridge>();
                bridge.set_draft_macro_name(draft_name.into());
                bridge.set_draft_macro_enabled(draft_enabled);
                bridge.set_draft_macro_steps(
                    std::rc::Rc::new(slint::VecModel::from(steps_snap)).into(),
                );
            }
        }
    });

    ui.global::<Bridge>().on_draft_add_step({
        let draft_macro_steps = draft_macro_steps.clone();
        let push = push_draft_steps.clone();
        move |kind| {
            let mut g = draft_macro_steps.lock().unwrap();
            let label = match kind {
                QuickActionKind::ScanQr => "Scan QR",
                QuickActionKind::OpenAudio => "Open Audio",
                QuickActionKind::OpenCamera => "Open Camera",
                QuickActionKind::StartRecord => "Start Recording",
                QuickActionKind::StopRecord => "Stop Recording",
                QuickActionKind::StopCast => "Stop Cast",
                _ => "",
            };
            g.push(MacroStep {
                kind,
                label: label.into(),
            });
            drop(g);
            push();
        }
    });

    ui.global::<Bridge>().on_draft_remove_step({
        let draft_macro_steps = draft_macro_steps.clone();
        let push = push_draft_steps.clone();
        move |idx| {
            let mut g = draft_macro_steps.lock().unwrap();
            if let Ok(i) = usize::try_from(idx) {
                if i < g.len() {
                    g.remove(i);
                }
            }
            drop(g);
            push();
        }
    });

    ui.global::<Bridge>().on_draft_move_step({
        let draft_macro_steps = draft_macro_steps.clone();
        let push = push_draft_steps.clone();
        move |from, to| {
            let mut g = draft_macro_steps.lock().unwrap();
            if let (Ok(from_u), Ok(to_u)) = (usize::try_from(from), usize::try_from(to)) {
                if from_u < g.len() && to_u < g.len() && from_u != to_u {
                    let s = g.remove(from_u);
                    g.insert(to_u, s);
                }
            }
            drop(g);
            push();
        }
    });

    let push_history = {
        let history = history.clone();
        let ui_weak = ui.as_weak();
        move || {
            let snap = history.lock().clone();
            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                ui.global::<Bridge>()
                    .set_history(std::rc::Rc::new(slint::VecModel::from(snap)).into());
            });
        }
    };
    push_history();

    // ── Bitrate presets (Phase 8 / Cluster C1) — data + pusher ──────────
    // Created here (above the D1 handlers) so `on_reset_settings` can
    // restore the factory presets list. The C1 callback registrations
    // (save / delete / set-active) live further down and capture these
    // same handles by clone.
    //
    // The factory-default literal lives in `default_presets()` so init
    // and reset share a single source of truth — same pattern as
    // `default_quick_actions()`.
    let presets: Arc<Mutex<Vec<BitratePreset>>> = Arc::new(Mutex::new(default_presets()));
    // Monotonic id source for user-created presets. Never use `g.len()`:
    // after a delete-then-add cycle len() can collide with a previously
    // issued id.
    let next_preset_id: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));
    let push_presets = {
        let presets = presets.clone();
        let ui_weak = ui.as_weak();
        move || {
            let snapshot = presets.lock().clone();
            let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                ui.global::<Bridge>()
                    .set_presets(std::rc::Rc::new(slint::VecModel::from(snapshot)).into());
            });
        }
    };
    push_presets();

    // Create the tokio runtime *before* registering Slint callbacks that
    // call `tokio::spawn` (directly or via `Application::flash_banner`).
    // Slint callbacks run on the UI thread during `ui.run()`, which has no
    // tokio context by default — `tokio::spawn` would panic with "there is
    // no reactor running". The `_runtime_guard` registers this thread as a
    // runtime context for the lifetime of the guard. It MUST be dropped
    // before `runtime.block_on(...)` later in this function, otherwise
    // `block_on` panics ("Cannot start a runtime from within a runtime").
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let _runtime_guard = runtime.enter();

    let files_dir = resolve_android_files_dir(&app_clone).unwrap_or_else(|err| {
        error!(
            ?err,
            "Failed to resolve Android files dir for backend settings"
        );
        std::env::temp_dir()
    });
    let backend_lifecycle =
        std::sync::Arc::new(backend::lifecycle::BackendLifecycle::new(files_dir));
    backend_lifecycle.register(&ui);

    // ── Phase 8 / Cluster D1 — Backup / reset handlers ──────────────────
    ui.global::<Bridge>().on_export_settings({
        let ui_weak = ui.as_weak();
        move || {
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                // Phase 11: ACTION_CREATE_DOCUMENT via JNI; serialise + write.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                Application::flash_banner(
                    ui_weak,
                    "Settings exported (placeholder).".into(),
                    BannerSeverity::Success,
                    std::time::Duration::from_secs(3),
                );
            });
        }
    });

    ui.global::<Bridge>().on_import_settings({
        let ui_weak = ui.as_weak();
        move || {
            let ui_weak = ui_weak.clone();
            tokio::spawn(async move {
                // Phase 11: ACTION_OPEN_DOCUMENT, parse JSON, write to DataStore.
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                Application::flash_banner(
                    ui_weak,
                    "Settings imported (placeholder).".into(),
                    BannerSeverity::Success,
                    std::time::Duration::from_secs(3),
                );
            });
        }
    });

    ui.global::<Bridge>().on_reset_settings({
        let bar_actions = bar_actions.clone();
        let history = history.clone();
        let presets = presets.clone();
        let next_preset_id = next_preset_id.clone();
        let macros = macros.clone();
        let next_macro_id = next_macro_id.clone();
        let push_bar = push_bar.clone();
        let push_history = push_history.clone();
        let push_presets = push_presets.clone();
        let push_macros = push_macros.clone();
        let ui_weak = ui.as_weak();
        move || {
            // Reset every Cluster-C/D model owned by Rust to factory
            // defaults.
            //
            // `next_preset_id` / `next_macro_id` are also rewound so
            // user-created ids restart at `custom-0` / `macro-0` after
            // a reset, matching the factory state. Without this, a
            // freshly-reset device would still hand out `custom-N` /
            // `macro-N` for some N > 0 the moment the user added an
            // entry.
            *bar_actions.lock() = default_quick_actions();
            *presets.lock() = default_presets();
            next_preset_id.store(0, Ordering::Relaxed);
            history.lock().clear();
            macros.lock().unwrap().clear();
            next_macro_id.store(0, Ordering::Relaxed);

            push_bar();
            push_presets();
            push_history();
            push_macros();

            // Phase 11: also clear DataStore / SharedPreferences via JNI.

            Application::flash_banner(
                ui_weak.clone(),
                "Settings reset to defaults".into(),
                BannerSeverity::Success,
                std::time::Duration::from_secs(3),
            );
        }
    });

    ui.global::<Bridge>().on_clear_cast_history({
        let history = history.clone();
        let push_history = push_history.clone();
        let ui_weak = ui.as_weak();
        move || {
            history.lock().clear();
            push_history();

            Application::flash_banner(
                ui_weak.clone(),
                "Cast history cleared".into(),
                BannerSeverity::Success,
                std::time::Duration::from_secs(2),
            );
        }
    });

    ui.global::<Bridge>().on_clear_known_receivers({
        let ui_weak = ui.as_weak();
        move || {
            // Phase 11: clear known-receivers DataStore. For now, announce.
            Application::flash_banner(
                ui_weak.clone(),
                "Known receivers cleared".into(),
                BannerSeverity::Success,
                std::time::Duration::from_secs(2),
            );
        }
    });

    // ── Phase 8 / Cluster D2 — Cast history handlers ────────────────────
    ui.global::<Bridge>().on_clear_history({
        let history = history.clone();
        let push_history = push_history.clone();
        move || {
            history.lock().clear();
            push_history();
        }
    });

    ui.global::<Bridge>().on_delete_history_entry({
        let history = history.clone();
        let push_history = push_history.clone();
        move |id| {
            let id = id.to_string();
            history.lock().retain(|e| e.id != id);
            push_history();
        }
    });

    ui.global::<Bridge>().on_recast({
        let history = history.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            let id = id.to_string();
            let entry_opt = history.lock().iter().find(|e| e.id == id).cloned();
            let Some(entry) = entry_opt else {
                return;
            };
            // Phase 11: trigger reconnection + start_casting with the same receiver.
            Application::flash_banner(
                ui_weak.clone(),
                format!("Recasting to {}", entry.receiver),
                BannerSeverity::Info,
                std::time::Duration::from_secs(2),
            );
        }
    });

    // Push selected-history-entry when a row is tapped. Uses the explicit
    // open-history-detail callback — no `changed` re-emit needed.
    //
    // Called synchronously (Slint UI thread) so the detail page always
    // renders with fresh data on the same frame it becomes visible.
    ui.global::<Bridge>().on_open_history_detail({
        let history = history.clone();
        let ui_weak = ui.as_weak();
        move |entry_id: slint::SharedString| {
            let id = entry_id.to_string();
            let entry = history.lock().iter().find(|e| e.id == id).cloned();
            let Some(entry) = entry else {
                return;
            };
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<Bridge>().set_selected_history_id(entry_id);
                ui.global::<Bridge>().set_selected_history_entry(entry);
            }
        }
    });

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();

    let ui_handle = ui.as_weak();
    // NOTE: The shared `Arc<Mutex<StatusSnapshot>>` cache that producers
    // (battery / thermal / network listeners) will update lands with
    // Cluster B (Phase 8 Section 3). For now the ticker just rebuilds a
    // hardcoded snapshot on every tick — no shared state needed yet.
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            tick.tick().await;
            let snap_now = StatusSnapshot {
                network_label: "Wi-Fi".into(),
                thermal_label: "Nominal".into(),
                battery_pct: 87,
                charging: false,
            };
            push_status(ui_handle.clone(), snap_now);
        }
    });
    fn enumerate_interfaces() -> Vec<NetworkInterface> {
        vec![
            NetworkInterface {
                name: "wlan0".into(),
                kind: NetworkKind::Wifi,
                address_v4: "192.168.1.42".into(),
                address_v6: "fe80::1234".into(),
                enabled: true,
            },
            NetworkInterface {
                name: "rmnet0".into(),
                kind: NetworkKind::Cellular,
                address_v4: "10.20.30.40".into(),
                address_v6: "".into(),
                enabled: false,
            },
            NetworkInterface {
                name: "lo".into(),
                kind: NetworkKind::Loopback,
                address_v4: "127.0.0.1".into(),
                address_v6: "::1".into(),
                enabled: true,
            },
        ]
    }
    fn push_interfaces(ui_handle: slint::Weak<MainWindow>, list: Vec<NetworkInterface>) {
        let _ = ui_handle.upgrade_in_event_loop(move |ui| {
            let model: slint::ModelRc<NetworkInterface> =
                std::rc::Rc::new(slint::VecModel::from(list)).into();
            ui.global::<Bridge>().set_network_interfaces(model);
        });
    }
    push_interfaces(ui.as_weak(), enumerate_interfaces());
    let interfaces = std::sync::Arc::new(tokio::sync::Mutex::new(enumerate_interfaces()));
    let interfaces_for_callback = interfaces.clone();
    let ui_for_callback = ui.as_weak();
    ui.global::<Bridge>()
        .on_set_interface_enabled(move |name, value| {
            let interfaces = interfaces_for_callback.clone();
            let ui_handle = ui_for_callback.clone();
            tokio::spawn(async move {
                let mut list = interfaces.lock().await;
                if let Some(iface) = list.iter_mut().find(|i| i.name == name.as_str()) {
                    iface.enabled = value;
                }
                push_interfaces(ui_handle, list.clone());
            });
        });
    let log_ring = log_ring::LogRing::new(ui.as_weak());
    let log_ring_for_clear = log_ring.clone();
    use tracing_subscriber::filter::LevelFilter;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::Layer;
    // Cap the LogRing layer at DEBUG so the firehose of GStreamer `Fixme`
    // / TRACE events forwarded by `tracing_gstreamer::integrate_events`
    // (see `Application::run_event_loop`) never reaches the ring buffer.
    // Without this filter, an active media pipeline can produce thousands
    // of TRACE events per second, each one mutating the ring and dirtying
    // the UI pusher — pointless for a human-readable debug log.
    //
    // `try_init` (not `init`) so re-entries of `android_main` (Android can
    // trigger on activity destroy/recreate) don't panic from
    // `set_global_default()` being called twice. Mirrors `init_once` above.
    if let Err(err) = tracing_subscriber::registry()
        .with(log_ring.clone().with_filter(LevelFilter::DEBUG))
        .try_init()
    {
        debug!(
            ?err,
            "tracing subscriber already initialised — re-entry of android_main"
        );
    }
    ui.global::<Bridge>().on_clear_log_entries(move || {
        log_ring_for_clear.clear();
    });
    // ── Bitrate presets (Phase 8 / Cluster C1) — callbacks ──────────────
    // The shared `presets` Arc + `push_presets` closure are declared
    // above (next to `history`) so the D1 `on_reset_settings` handler
    // can also restore the factory list.
    ui.global::<Bridge>().on_save_preset({
        let presets = presets.clone();
        let next_id = next_preset_id.clone();
        let push = push_presets.clone();
        move |id, name, kbps| {
            let mut g = presets.lock();
            if id.is_empty() {
                let new_id = format!("custom-{}", next_id.fetch_add(1, Ordering::Relaxed));
                g.push(BitratePreset {
                    id: new_id.into(),
                    name: name.into(),
                    bitrate_kbps: kbps,
                    active: false,
                });
            } else if let Some(p) = g.iter_mut().find(|p| p.id == id) {
                p.name = name.into();
                p.bitrate_kbps = kbps;
            }
            drop(g);
            push();
        }
    });
    ui.global::<Bridge>().on_delete_preset({
        let presets = presets.clone();
        let push = push_presets.clone();
        move |id| {
            let mut g = presets.lock();
            g.retain(|p| p.id != id);
            // If the deleted preset was the active one, promote the first
            // remaining preset to active so the user is never left without
            // a selection.
            if !g.iter().any(|p| p.active) {
                if let Some(first) = g.first_mut() {
                    first.active = true;
                }
            }
            drop(g);
            push();
        }
    });
    ui.global::<Bridge>().on_set_active_preset({
        let presets = presets.clone();
        let push = push_presets.clone();
        move |id| {
            let mut g = presets.lock();
            for p in g.iter_mut() {
                p.active = p.id == id;
            }
            drop(g);
            push();
        }
    });

    ui.global::<Bridge>().on_connect_receiver({
        let event_tx = event_tx.clone();
        move |device_name| {
            event_tx
                .send(Event::ConnectToDevice(device_name.to_string()))
                .unwrap();
        }
    });

    let panel_stack = std::rc::Rc::new(PanelStack::new());

    ui.global::<PanelBridge>().on_push({
        let stack = panel_stack.clone();
        let ui_weak = ui.as_weak();
        move |p: Panel| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let pb = ui.global::<PanelBridge>();
            let current = pb.get_active();
            if current == p {
                return;
            }
            stack.push_panel(current);
            pb.set_active(p);
            pb.set_stack(stack.as_model());
        }
    });

    ui.global::<PanelBridge>().on_pop({
        let stack = panel_stack.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let pb = ui.global::<PanelBridge>();
            pb.set_active(stack.pop_panel());
            pb.set_stack(stack.as_model());
        }
    });

    ui.global::<PanelBridge>().on_replace({
        let ui_weak = ui.as_weak();
        move |p: Panel| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<PanelBridge>().set_active(p);
            }
        }
    });

    ui.global::<PanelBridge>().on_close_all({
        let stack = panel_stack.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            stack.clear();
            let pb = ui.global::<PanelBridge>();
            pb.set_active(Panel::None);
            pb.set_stack(stack.as_model());
        }
    });

    ui.global::<Bridge>().on_back_requested({
        let ui_weak = ui.as_weak();
        let app_clone = app_clone.clone();
        move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            handle_back_request(&ui, Some(&app_clone));
        }
    });

    ui.global::<Bridge>().on_start_casting({
        let event_tx = event_tx.clone();
        move |scale_width: i32, scale_height: i32, max_framerate: i32| {
            event_tx
                .send(Event::StartCast {
                    scale_width: scale_width as u32,
                    scale_height: scale_height as u32,
                    max_framerate: max_framerate as u32,
                })
                .unwrap();
        }
    });

    ui.global::<Bridge>().on_stop_casting({
        let event_tx = event_tx.clone();
        move || {
            event_tx
                .send(Event::EndSession { disconnect: true })
                .unwrap();
        }
    });

    let recorder_state = Arc::new(tokio::sync::Mutex::new(RecordingTickerState::default()));

    ui.global::<Recording>().on_start({
        let recorder_state = recorder_state.clone();
        let ui_handle = ui.as_weak();
        move || {
            let recorder_state = recorder_state.clone();
            let ui_handle = ui_handle.clone();
            tokio::spawn(async move {
                let mut s = recorder_state.lock().await;
                s.started_at = Some(std::time::Instant::now());
                s.paused_for = std::time::Duration::ZERO;
                s.pause_started = None;
                s.state = RecordingState::Recording;
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.global::<Recording>()
                        .set_state(RecordingState::Recording);
                    ui.global::<Recording>().set_elapsed_s(0);
                });
            });
        }
    });

    ui.global::<Recording>().on_pause({
        let recorder_state = recorder_state.clone();
        let ui_handle = ui.as_weak();
        move || {
            let recorder_state = recorder_state.clone();
            let ui_handle = ui_handle.clone();
            tokio::spawn(async move {
                let mut s = recorder_state.lock().await;
                if s.state != RecordingState::Recording {
                    return;
                }
                s.pause_started = Some(std::time::Instant::now());
                s.state = RecordingState::Paused;
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.global::<Recording>().set_state(RecordingState::Paused);
                });
            });
        }
    });

    ui.global::<Recording>().on_resume({
        let recorder_state = recorder_state.clone();
        let ui_handle = ui.as_weak();
        move || {
            let recorder_state = recorder_state.clone();
            let ui_handle = ui_handle.clone();
            tokio::spawn(async move {
                let mut s = recorder_state.lock().await;
                if s.state != RecordingState::Paused {
                    return;
                }
                if let Some(started) = s.pause_started.take() {
                    s.paused_for += started.elapsed();
                }
                s.state = RecordingState::Recording;
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.global::<Recording>()
                        .set_state(RecordingState::Recording);
                });
            });
        }
    });

    ui.global::<Recording>().on_stop({
        let recorder_state = recorder_state.clone();
        let ui_handle = ui.as_weak();
        move || {
            let recorder_state = recorder_state.clone();
            let ui_handle = ui_handle.clone();
            tokio::spawn(async move {
                {
                    let mut s = recorder_state.lock().await;
                    s.state = RecordingState::Finalizing;
                }
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    ui.global::<Recording>()
                        .set_state(RecordingState::Finalizing);
                });

                let mut s = recorder_state.lock().await;
                s.started_at = None;
                s.paused_for = std::time::Duration::ZERO;
                s.pause_started = None;
                s.state = RecordingState::Idle;
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    let rec = ui.global::<Recording>();
                    rec.set_state(RecordingState::Idle);
                    rec.set_elapsed_s(0);
                });
            });
        }
    });

    spawn_recording_ticker(ui.as_weak(), recorder_state.clone());

    ui.global::<Bridge>().on_engage_lock({
        let ui_handle = ui.as_weak();
        move || {
            let _ = ui_handle.upgrade_in_event_loop(|ui| {
                ui.global::<Bridge>()
                    .set_lifecycle(LifecycleMode::LockScreen);
            });
        }
    });

    ui.global::<Bridge>().on_engage_stealth({
        let ui_handle = ui.as_weak();
        move || {
            let _ = ui_handle.upgrade_in_event_loop(|ui| {
                ui.global::<Bridge>().set_lifecycle(LifecycleMode::Stealth);
            });
        }
    });

    ui.global::<Bridge>().on_start_snapshot_countdown({
        let ui_handle = ui.as_weak();
        // Monotonic generation counter. Each new countdown bumps it and
        // captures the new value; the spawned timer only resets lifecycle
        // if its captured generation is still current. This makes any
        // older, still-sleeping timer a no-op when a newer countdown is
        // triggered. Mirrors `Application::banner_generation`.
        static SNAPSHOT_GEN: AtomicU64 = AtomicU64::new(0);
        move |seconds: i32| {
            let ui_handle = ui_handle.clone();
            let gen = SNAPSHOT_GEN.fetch_add(1, Ordering::SeqCst) + 1;
            tokio::spawn(async move {
                let _ = ui_handle.upgrade_in_event_loop(|ui| {
                    ui.global::<Bridge>()
                        .set_lifecycle(LifecycleMode::SnapshotCountdown);
                });
                tokio::time::sleep(std::time::Duration::from_secs(seconds.max(0) as u64)).await;
                // Only reset to Normal if (a) no newer countdown has started
                // (otherwise we'd cut the new one short) and (b) we are still
                // in SnapshotCountdown (otherwise the user cancelled or
                // engaged lock/stealth and we must not clobber their choice).
                if SNAPSHOT_GEN.load(Ordering::SeqCst) != gen {
                    return;
                }
                let _ = ui_handle.upgrade_in_event_loop(|ui| {
                    let bridge = ui.global::<Bridge>();
                    if bridge.get_lifecycle() == LifecycleMode::SnapshotCountdown {
                        bridge.set_lifecycle(LifecycleMode::Normal);
                    }
                });
            });
        }
    });

    ui.global::<Bridge>().on_exit_lifecycle({
        let ui_handle = ui.as_weak();
        move || {
            let _ = ui_handle.upgrade_in_event_loop(|ui| {
                ui.global::<Bridge>().set_lifecycle(LifecycleMode::Normal);
            });
        }
    });

    ui.global::<Bridge>().on_set_wifi_aware({
        let ui_handle = ui.as_weak();
        move |enabled| {
            let ui_handle = ui_handle.clone();
            tokio::spawn(async move {
                let success = true;
                let _ = ui_handle.upgrade_in_event_loop(move |ui| {
                    let bridge = ui.global::<Bridge>();
                    bridge.set_wifi_aware_enabled(enabled && success);
                });

                Application::flash_banner(
                    ui_handle,
                    if enabled {
                        "Wi-Fi Aware enabled (placeholder — no permission requested).".into()
                    } else {
                        "Wi-Fi Aware disabled.".into()
                    },
                    BannerSeverity::Info,
                    std::time::Duration::from_secs(3),
                );
            });
        }
    });

    ui.global::<Bridge>().on_invoke_action({
        let app_clone = app_clone.clone();
        let ui_weak = ui.as_weak();
        move |id| {
            let id_str = id.as_str();
            match id_str {
                "scan-qr" => {
                    call_java_method_no_args(&app_clone, JavaMethod::ScanQr);
                }
                "migrated-server" => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.global::<Bridge>()
                            .invoke_start_migration_server(LEGACY_COMMAND_BIND_ADDR.into());
                    });
                }
                "test-getinfo" => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.global::<Bridge>()
                            .invoke_run_migration_test("getinfo".into());
                    });
                }
                "test-crossfade" => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.global::<Bridge>()
                            .invoke_run_migration_test("crossfade".into());
                    });
                }
                "test-smoke" => {
                    let _ = ui_weak.upgrade_in_event_loop(|ui| {
                        ui.global::<Bridge>()
                            .invoke_run_migration_test("smoke".into());
                    });
                }
                _ => {}
            }
        }
    });

    #[cfg(debug_assertions)]
    {
        ui.global::<Bridge>().on_start_migration_server({
            let ui_weak = ui.as_weak();
            move |bind_addr| {
                let status = match start_migrated_command_server(bind_addr.as_str()) {
                    Ok(message) => format!("PASS {message}"),
                    Err(err) => format!("FAIL {err}"),
                };
                log_ui_test_status("start-migration-server", &status);
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Bridge>().set_test_status(status.into());
                });
            }
        });

        ui.global::<Bridge>().on_run_migration_test({
            let ui_weak = ui.as_weak();
            move |test_id| {
                let test_id = test_id.to_string();
                let _ = ui_weak.upgrade_in_event_loop({
                    let test_id = test_id.clone();
                    move |ui| {
                        ui.global::<Bridge>().set_test_status(
                            format!("Running migration test '{test_id}'...").into(),
                        );
                    }
                });
                let ui_weak_clone = ui_weak.clone();
                std::thread::spawn(move || {
                    let status = match test_id.as_str() {
                        "getinfo" => run_legacy_http_getinfo_test(LEGACY_COMMAND_BIND_ADDR),
                        "crossfade" => run_legacy_http_crossfade_test(LEGACY_COMMAND_BIND_ADDR),
                        "smoke" => run_graph_smoke_test(),
                        other => format!("FAIL unknown migration-test id: {other}"),
                    };
                    log_ui_test_status(migration_test_log_name(&test_id), &status);
                    let _ = ui_weak_clone.upgrade_in_event_loop(move |ui| {
                        ui.global::<Bridge>().set_test_status(status.into());
                    });
                });
            }
        });

        ui.global::<Bridge>().on_stop_migration_server({
            let ui_weak = ui.as_weak();
            move || {
                let status = match migration_runtime::runtime::shutdown_graph_runtime() {
                    Ok(()) => "PASS migration server stopped".to_string(),
                    Err(err) => format!("FAIL migration server stop: {err}"),
                };
                log_ui_test_status("stop-migration-server", &status);
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<Bridge>().set_test_status(status.into());
                });
            }
        });
    }

    // ── Test functionality callbacks ─────────────────────────────────────
    ui.global::<Bridge>().on_start_test({
        let ui_weak = ui.as_weak();
        move || {
            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                ui.global::<Bridge>().set_test_state(MixerState::Starting);
            });
            // TODO: build and start GStreamer test pipeline.
            // Pattern: see crates/migration-runtime/src/nodes/mixer.rs for compositor pad setup.
            // On success call set_test_state(MixerState::Running).
            // On error call set_test_state(MixerState::Error) + set_test_error_text.
            log::info!("start-test: stub — pipeline not yet implemented");
        }
    });

    ui.global::<Bridge>().on_stop_test({
        let ui_weak = ui.as_weak();
        move || {
            // TODO: stop GStreamer test pipeline and release resources.
            let _ = ui_weak.upgrade_in_event_loop(|ui| {
                ui.global::<Bridge>().set_test_state(MixerState::Idle);
                ui.global::<Bridge>().set_test_error_text("".into());
            });
            log::info!("stop-test: stub — pipeline not yet implemented");
        }
    });

    ui.global::<Bridge>().on_pick_test_overlay_image(|| {
        // TODO(android): launch ACTION_GET_CONTENT intent via JNI, write
        //   result back with set_test_overlay_image_path.
        // TODO(desktop): use rfd::FileDialog to pick a file.
        log::info!("pick-test-overlay-image: stub — file picker not yet implemented");
    });

    let ui_weak = ui.as_weak();

    let event_tx_clone = event_tx.clone();
    let app_jh = runtime.spawn(async move {
        Application::new(ui_weak, event_tx_clone, app_clone)
            .await
            .unwrap()
            .run_event_loop(event_rx)
            .await
            .unwrap();
    });

    ui.run().unwrap();

    // Drop the runtime context guard before `block_on`, which panics if
    // called from within a tokio runtime context.
    drop(_runtime_guard);

    runtime.block_on(async move {
        if let Err(err) = event_tx.send(Event::Quit) {
            error!(?err, "Failed to send quit event");
        }
        if let Err(err) = app_jh.await {
            error!(?err, "Android application task join failed");
        }
    });

    debug!("Finished");
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeGraphCommand<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    command_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::main_activity::native_graph_command(env, class, command_json)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_FCastDiscoveryListener_serviceFound<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    name: JString<'local>,
    addrs: jni::objects::JObject,
    port: jni::sys::jint,
) {
    crate::jni_bridge::discovery::service_found(env, class, name, addrs, port)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_FCastDiscoveryListener_serviceLost<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    name: jni::objects::JString<'local>,
) {
    crate::jni_bridge::discovery::service_lost(env, class, name)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeCaptureStarted<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) {
    crate::jni_bridge::main_activity::native_capture_started(env, class)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeCaptureStopped<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) {
    crate::jni_bridge::main_activity::native_capture_stopped(env, class)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeCaptureCancelled<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) {
    crate::jni_bridge::main_activity::native_capture_cancelled(env, class)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeProcessFrame<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    width: jni::sys::jint,
    height: jni::sys::jint,
    buffer_y: JByteBuffer<'local>,
    buffer_u: JByteBuffer<'local>,
    buffer_v: JByteBuffer<'local>,
) {
    crate::jni_bridge::main_activity::native_process_frame(
        env, class, width, height, buffer_y, buffer_u, buffer_v,
    )
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeQrScanResult<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    result: jni::objects::JString<'local>,
) {
    crate::jni_bridge::main_activity::native_qr_scan_result(env, class, result)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MainActivity_nativeBackPressed<'local>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) {
    crate::jni_bridge::main_activity::native_back_pressed(env, class)
}

// ── gst-pop service host JNI bridge ──────────────────────────────────────────
// Symbols match GstPopServiceBridge in the `org.fcast.android.sender` package.

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost<
    'local,
>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    config_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::gstpop_bridge::native_start(env, class, config_json)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStopGstPopServiceHost<
    'local,
>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::gstpop_bridge::native_stop(env, class)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeGetGstPopServiceStatus<
    'local,
>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::gstpop_bridge::native_status(env, class)
}

// ── migration runtime service host JNI bridge ────────────────────────────────
// Symbols match MigrationRuntimeServiceBridge in the
// `org.fcast.android.sender` package.

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost<
    'local,
>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
    config_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::migration_bridge::native_start(env, class, config_json)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStopMigrationRuntimeHost<
    'local,
>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::migration_bridge::native_stop(env, class)
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeGetMigrationRuntimeStatus<
    'local,
>(
    env: jni::JNIEnv<'local>,
    class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    crate::jni_bridge::migration_bridge::native_status(env, class)
}
#[cfg(test)]
mod phase9_dispatch_tests {
    use super::migration_test_log_name;

    #[test]
    fn migration_test_log_name_known_ids() {
        assert_eq!(migration_test_log_name("getinfo"), "legacy-getinfo");
        assert_eq!(migration_test_log_name("crossfade"), "legacy-crossfade");
        assert_eq!(migration_test_log_name("smoke"), "graph-smoke");
    }

    #[test]
    fn migration_test_log_name_unknown_id() {
        assert_eq!(migration_test_log_name(""), "unknown");
        assert_eq!(migration_test_log_name("bogus"), "unknown");
        assert_eq!(migration_test_log_name("GetInfo"), "unknown");
    }

    #[test]
    fn migration_test_id_count_invariant() {
        const KNOWN: &[&str] = &["getinfo", "crossfade", "smoke"];

        for id in KNOWN {
            assert_ne!(
                migration_test_log_name(id),
                "unknown",
                "test id {id} should be in the dispatch table"
            );
        }
    }
}
