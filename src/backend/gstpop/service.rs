use crate::backend::persistence::StoredBackendConfig;

/// Ask the foreground GstPopService to start the daemon. Idempotent.
#[cfg(target_os = "android")]
pub fn request_service_start(config: &StoredBackendConfig) -> anyhow::Result<()> {
    use anyhow::Context;
    use jni::objects::JValue;

    let ctx = crate::android_context().context("android_context")?;
    let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
    let config_json = serde_json::to_string(config).context("serialize StoredBackendConfig")?;
    let jconfig = env.new_string(config_json).context("new_string(config)")?;
    let bridge = env
        .find_class("org/fcast/android/sender/GstPopServiceBridge")
        .context("find_class GstPopServiceBridge")?;
    env.call_static_method(
        bridge,
        "start",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[
            JValue::Object(&ctx.activity),
            JValue::Object(&jconfig.into()),
        ],
    )
    .context("call GstPopServiceBridge.start")?;
    Ok(())
}

/// Ask the foreground GstPopService to stop. Idempotent; safe if not running.
#[cfg(target_os = "android")]
pub fn request_service_stop() {
    use anyhow::Context;
    use jni::objects::JValue;

    if let Ok(ctx) = crate::android_context() {
        let _ = (|| -> anyhow::Result<()> {
            let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
            let bridge = env
                .find_class("org/fcast/android/sender/GstPopServiceBridge")
                .context("find_class GstPopServiceBridge")?;
            env.call_static_method(
                bridge,
                "stop",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&ctx.activity)],
            )
            .context("call GstPopServiceBridge.stop")?;
            Ok(())
        })();
    }
}

// ── Non-Android stubs ─────────────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn request_service_start(_config: &StoredBackendConfig) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn request_service_stop() {}
