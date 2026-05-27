// src/migration_service.rs
//
// Rust → Java reflection helper for the MigrationRuntimeService Android
// foreground service. Mirrors src/backend/gstpop/service.rs.

// ── Android impl ──────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
mod android {
    use anyhow::{Context, Result};
    use jni::objects::{JObject, JValue};
    use jni::JNIEnv;

    /// `env.find_class()` uses the bootstrap ClassLoader on non-JVM-spawned
    /// threads and cannot see app classes. Use the activity's ClassLoader
    /// instead.
    pub(super) fn load_app_class<'e>(
        env: &mut JNIEnv<'e>,
        activity: &JObject<'_>,
        class_name: &str,
    ) -> Result<jni::objects::JClass<'e>> {
        let loader = env
            .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
            .context("getClassLoader")?
            .l()
            .context("getClassLoader result")?;
        let jname = env.new_string(class_name).context("new_string class name")?;
        let class = env
            .call_method(
                &loader,
                "loadClass",
                "(Ljava/lang/String;)Ljava/lang/Class;",
                &[JValue::Object(&jname.into())],
            )
            .context("loadClass")?
            .l()
            .context("loadClass result")?;
        Ok(jni::objects::JClass::from(class))
    }
}

/// Ask the foreground MigrationRuntimeService to start the runtime. Idempotent.
///
/// The runtime has no start-time config today; an empty JSON object is
/// passed to keep the call signature symmetric with
/// `GstPopServiceBridge.start(Context, String)`.
#[cfg(target_os = "android")]
pub fn request_service_start() -> anyhow::Result<()> {
    use anyhow::Context;
    use jni::objects::JValue;

    let ctx = crate::android_context().context("android_context")?;
    let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
    let jconfig = env.new_string("{}").context("new_string(config)")?;
    let bridge = android::load_app_class(
        &mut env,
        &ctx.activity,
        "org.fcast.android.sender.MigrationRuntimeServiceBridge",
    )
    .context("load MigrationRuntimeServiceBridge")?;
    env.call_static_method(
        bridge,
        "start",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[
            JValue::Object(&ctx.activity),
            JValue::Object(&jconfig.into()),
        ],
    )
    .context("call MigrationRuntimeServiceBridge.start")?;
    Ok(())
}

/// Ask the foreground MigrationRuntimeService to stop. Idempotent; safe if
/// not running.
#[cfg(target_os = "android")]
pub fn request_service_stop() {
    use anyhow::Context;
    use jni::objects::JValue;

    if let Ok(ctx) = crate::android_context() {
        let _ = (|| -> anyhow::Result<()> {
            let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
            let bridge = android::load_app_class(
                &mut env,
                &ctx.activity,
                "org.fcast.android.sender.MigrationRuntimeServiceBridge",
            )
            .context("load MigrationRuntimeServiceBridge")?;
            env.call_static_method(
                bridge,
                "stop",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&ctx.activity)],
            )
            .context("call MigrationRuntimeServiceBridge.stop")?;
            Ok(())
        })();
    }
}

/// Query the runtime status synchronously via the bridge's static
/// `queryStatus()` method. Returns the JSON string verbatim (the same
/// shape produced by `migration_runtime_status_json` in lib.rs).
#[cfg(target_os = "android")]
pub fn query_status() -> anyhow::Result<String> {
    use anyhow::Context;

    let ctx = crate::android_context().context("android_context")?;
    let mut env = ctx.vm.attach_current_thread().context("attach_current_thread")?;
    let bridge = android::load_app_class(
        &mut env,
        &ctx.activity,
        "org.fcast.android.sender.MigrationRuntimeServiceBridge",
    )
    .context("load MigrationRuntimeServiceBridge")?;
    let result = env
        .call_static_method(bridge, "queryStatus", "()Ljava/lang/String;", &[])
        .context("call MigrationRuntimeServiceBridge.queryStatus")?
        .l()
        .context("queryStatus result")?;
    let jstr: jni::objects::JString = result.into();
    let value: String = env
        .get_string(&jstr)
        .context("get_string queryStatus")?
        .into();
    Ok(value)
}

// ── Non-Android stubs ─────────────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn request_service_start() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(target_os = "android"))]
pub fn request_service_stop() {}

#[cfg(not(target_os = "android"))]
pub fn query_status() -> anyhow::Result<String> {
    Ok(r#"{"state":"stopped"}"#.to_string())
}
