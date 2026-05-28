//! JNI bridge — symbols called from GstPopServiceBridge.

#[cfg(target_os = "android")]
use jni::{
    objects::{JClass, JString},
    JNIEnv,
};

#[cfg(target_os = "android")]
use crate::jni_bridge::helpers::jstring_to_string;

#[cfg(target_os = "android")]
pub fn native_start<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    config_json: JString<'local>,
) -> jni::sys::jstring {
    let config = jstring_to_string(&mut env, &config_json).unwrap_or_default();
    let port = parse_gstpop_config_port(&config).unwrap_or(9000);
    let status = crate::HOST_RUNTIME.block_on(async { gstpop_runtime::start_embedded(port).await });
    let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
pub fn native_stop<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>) -> jni::sys::jstring {
    let status = crate::HOST_RUNTIME.block_on(async { gstpop_runtime::stop_embedded().await });
    let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
pub fn native_status<'local>(mut env: JNIEnv<'local>, _class: JClass<'local>) -> jni::sys::jstring {
    let status = gstpop_runtime::embedded_status();
    let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
fn parse_gstpop_config_port(json: &str) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let url = v.get("gstpop_url")?.as_str()?;
    Some(gstpop_runtime::url_port(url))
}
