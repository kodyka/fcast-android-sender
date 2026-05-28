//! JNI bridge — symbols called from MigrationRuntimeServiceBridge.

#[cfg(target_os = "android")]
use jni::{objects::JClass, JNIEnv};

#[cfg(target_os = "android")]
pub fn native_start<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    _config_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    // Migration runtime currently has no start-time config; the JString is
    // accepted for API symmetry with GstPopServiceBridge and ignored.
    let json = match migration_runtime::runtime::start_graph_runtime(
        migration_runtime::runtime::RuntimeHandles {
            frame_pair: crate::FRAME_PAIR.clone(),
        },
    ) {
        Ok(()) => migration_runtime_status_json("running", None),
        Err(err) => migration_runtime_status_json("error", Some(&err.to_string())),
    };
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
pub fn native_stop<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>) -> jni::sys::jstring {
    let json = match migration_runtime::runtime::shutdown_graph_runtime() {
        Ok(()) => migration_runtime_status_json("stopped", None),
        Err(err) => migration_runtime_status_json("error", Some(&err.to_string())),
    };
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
pub fn native_status<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>) -> jni::sys::jstring {
    let state = if migration_runtime::runtime::is_running() {
        "running"
    } else {
        "stopped"
    };
    let json = migration_runtime_status_json(state, None);
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
fn migration_runtime_status_json(state: &str, last_error: Option<&str>) -> String {
    let mut value = serde_json::json!({ "state": state });
    if let Some(err) = last_error {
        value["last_error"] = serde_json::Value::String(err.to_string());
    }
    serde_json::to_string(&value)
        .unwrap_or_else(|_| format!("{{\"state\":\"{}\"}}", state.replace('"', "'")))
}
