# 05 — Rust caller helper (`src/migration/service.rs`)

A new Rust module that lets in-process Rust code (lifecycle, Slint
callbacks, debug surfaces) drive the `MigrationRuntimeService` via JNI
reflection. Mirrors `src/backend/gstpop/service.rs` (see lines 1-102 of
that file).

This step is **optional for the Java side to work** — the service can
already be started via `adb` or the Slint UI step 6 once steps 1-4 are
in place. But this helper is required if you want lifecycle.rs or any
other Rust path to start/stop the service the same way it does
gst-pop.

## 5.1 Full file content

```rust
// src/migration/service.rs
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
```

## 5.2 Module registration

```diff
 // src/migration/mod.rs
 pub mod media_bridge;
 pub mod messages;
 pub mod node_manager;
 pub mod nodes;
 pub mod protocol;
 pub mod runtime;
+pub mod service;
```

## 5.3 DRY note — `load_app_class`

The `load_app_class` helper here is **literal copy-paste** from
`src/backend/gstpop/service.rs:13-35`. That is intentional for the
first implementation PR — duplication is cheaper than the wrong
abstraction, and keeping each service file self-contained makes
review trivial.

A follow-up PR can lift the helper to a shared module — suggested
location: `src/android.rs` with `pub fn load_app_class(...)`. Both
`backend::gstpop::service` and `migration::service` would then import
it. **Do not** bundle that refactor with the implementation PR — keep
the two changes separately reviewable.

## 5.4 Why no `Context` parameter on `request_service_start`?

`backend::gstpop::service::request_service_start` takes a
`&StoredBackendConfig` because the gst-pop daemon needs the URL/port
encoded into the start intent. The migration runtime has no such
config, so the function takes nothing. When the runtime grows
configuration (a `MigrationRuntimeConfig` struct, say), update the
signature to mirror `gstpop::service::request_service_start`.

## 5.5 Calling from lifecycle.rs (illustrative)

Once the helper exists, wiring it up from
`src/backend/lifecycle.rs:77-98` looks like:

```rust
// Pseudo-code — actual integration depends on whether
// MediaBackendKind::Migration is the selected backend.

let start_weak = ui.as_weak();
bridge.on_start_migration_runtime_service(move || {
    let weak = start_weak.clone();
    tokio::spawn(async move {
        push_migration_state(&weak, "starting");
        if let Err(err) = crate::migration::service::request_service_start() {
            push_migration_error(&weak, &format!("service start failed: {err}"));
        }
    });
});

let stop_weak = ui.as_weak();
bridge.on_stop_migration_runtime_service(move || {
    crate::migration::service::request_service_stop();
    let weak = stop_weak.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        bridge.set_migration_runtime_service_state("stopping".into());
    });
});
```

The Slint property and callback names referenced here
(`on_start_migration_runtime_service`,
`set_migration_runtime_service_state`) come from step 6.
