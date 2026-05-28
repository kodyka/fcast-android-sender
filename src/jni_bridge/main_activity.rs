//! JNI bridge — symbols called from MainActivity.
//!
//! Function names mirror the Java method names minus the `native` prefix
//! and converted to snake_case. lib.rs re-exports them with the
//! `Java_org_fcast_android_sender_MainActivity_*` symbol names.

#[cfg(target_os = "android")]
use jni::{
    objects::{JByteBuffer, JClass, JString},
    JNIEnv,
};
#[cfg(target_os = "android")]
use mcore::Event;
#[cfg(target_os = "android")]
use tracing::{debug, error, info, warn};

#[cfg(target_os = "android")]
use crate::jni_bridge::helpers::{handle_back_request, jstring_to_string, process_frame};

#[cfg(target_os = "android")]
pub fn native_graph_command<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    command_json: JString<'local>,
) -> jni::sys::jstring {
    if let Err(err) = migration_runtime::runtime::start_graph_runtime(
        migration_runtime::runtime::RuntimeHandles {
            frame_pair: crate::FRAME_PAIR.clone(),
        },
    ) {
        error!(?err, "Failed to start migrated graph runtime from JNI hook");
    }

    let response = match jstring_to_string(&mut env, &command_json) {
        Ok(json) => migration_runtime::runtime::try_handle_command_json(&json),
        Err(err) => {
            error!(?err, "Failed to decode graph command payload from Java");
            migration_runtime::runtime::try_handle_command_json("")
        }
    };

    match env.new_string(response) {
        Ok(jstr) => jstr.into_raw(),
        Err(err) => {
            error!(?err, "Failed to allocate Java response string");
            std::ptr::null_mut()
        }
    }
}

#[cfg(target_os = "android")]
pub fn native_capture_started<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    debug!("Screen capture was started");
    if let Err(err) = crate::GLOB_EVENT_CHAN.0.send(Event::CaptureStarted) {
        error!(?err, "Failed to send capture started event");
    }
}

#[cfg(target_os = "android")]
pub fn native_capture_stopped<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    debug!("Screen capture was stopped");
    if let Err(err) = crate::GLOB_EVENT_CHAN.0.send(Event::CaptureStopped) {
        error!(?err, "Failed to send capture stopped event");
    }
}

#[cfg(target_os = "android")]
pub fn native_capture_cancelled<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    debug!("Screen capture was cancelled");
    if let Err(err) = crate::GLOB_EVENT_CHAN.0.send(Event::CaptureCancelled) {
        error!(?err, "Failed to send capture cancelled event");
    }
}

#[cfg(target_os = "android")]
pub fn native_process_frame<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    width: jni::sys::jint,
    height: jni::sys::jint,
    buffer_y: JByteBuffer<'local>,
    buffer_u: JByteBuffer<'local>,
    buffer_v: JByteBuffer<'local>,
) {
    if let Err(err) = process_frame(env, width, height, buffer_y, buffer_u, buffer_v) {
        error!(?err, "Failed to process frame");
    }
}

#[cfg(target_os = "android")]
pub fn native_qr_scan_result<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    result: JString<'local>,
) {
    match jstring_to_string(&mut env, &result) {
        Ok(result) => {
            if let Err(err) = crate::GLOB_EVENT_CHAN.0.send(Event::QrScanResult(result)) {
                error!(?err, "Failed to send device removed event");
            }
        }
        Err(err) => error!(?err, "Failed to convert jstring to string"),
    }
}

#[cfg(target_os = "android")]
pub fn native_back_pressed<'local>(_env: JNIEnv<'local>, _class: JClass<'local>) {
    info!("nativeBackPressed");
    let Some(ui_weak) = crate::ANDROID_UI.lock().clone() else {
        warn!("Ignoring back press before UI initialization");
        return;
    };
    let app = crate::ANDROID_APP.lock().clone();

    if let Err(err) = ui_weak.upgrade_in_event_loop(move |ui| {
        handle_back_request(&ui, app.as_ref());
    }) {
        error!(?err, "Failed to dispatch Android back press to UI");
    }
}
