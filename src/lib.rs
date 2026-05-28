#[cfg(target_os = "android")]
use jni::objects::{JByteBuffer, JString};
use mcore::Event;
use parking_lot::Mutex;
#[cfg(target_os = "android")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub mod app;
pub mod application;
pub mod command;
pub mod config;
pub mod jni_bridge;
pub mod log_ring;
pub mod platform;
pub mod secret;

#[cfg(target_os = "android")]
mod android_main;

mod backend;
mod gstpop_service;
mod migration_service;

#[cfg(target_os = "android")]
use crate::platform::platform_app::PlatformApp;

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
    pub vm: Arc<jni::JavaVM>,
    pub activity: jni::objects::JObject<'static>,
}

#[cfg(target_os = "android")]
pub(crate) fn android_context() -> anyhow::Result<AndroidCtx> {
    let app = ANDROID_APP
        .lock()
        .clone()
        .ok_or_else(|| anyhow::anyhow!("android app not installed"))?;
    let vm = crate::jni_bridge::helpers::vm();
    let activity_ptr = app.activity_as_ptr() as *mut jni::sys::_jobject;
    // SAFETY: ANDROID_APP is set from the live PlatformApp during android_main
    // bootstrap and remains installed while Android service calls can reach
    // this context. The JObject wrapper is not retained beyond the caller's
    // immediate JNI invocation.
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

#[cfg(target_os = "android")]
fn set_capture_active(active: bool) {
    CAPTURE_ACTIVE.store(active, Ordering::SeqCst);
    if !active {
        let mut frame = FRAME_PAIR.frame.lock();
        *frame = None;
        FRAME_PAIR.cond.notify_all();
    }
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
