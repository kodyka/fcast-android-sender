# 5 · Rewire `BackendLifecycle`

The lifecycle layer (`src/backend/lifecycle.rs`) is the single funnel
between Slint's `Bridge` callbacks (Apply / Save / Probe) and the Rust
backend traits. After step 2 the daemon has explicit start/stop, but
no one is *calling* them. This step fixes that.

## 5.1 New module: `src/backend/gstpop/service.rs`

JNI dispatch helpers. Add this file alongside `embedded.rs`:

```rust
// src/backend/gstpop/service.rs

#[cfg(target_os = "android")]
use anyhow::{Context, Result};
#[cfg(target_os = "android")]
use jni::objects::{JObject, JValue};

use crate::backend::persistence::StoredBackendConfig;

/// Ask the foreground GstPopService to start the daemon. Idempotent.
#[cfg(target_os = "android")]
pub fn request_service_start(config: &StoredBackendConfig) -> Result<()> {
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
        &[JValue::Object(&ctx.activity), JValue::Object(&jconfig.into())],
    )
    .context("call GstPopServiceBridge.start")?;
    Ok(())
}

/// Ask the foreground GstPopService to stop. Idempotent; safe if the
/// service is not running.
#[cfg(target_os = "android")]
pub fn request_service_stop() {
    if let Ok(ctx) = crate::android_context() {
        let _ = (|| -> Result<()> {
            let mut env = ctx.vm.attach_current_thread()?;
            let bridge = env.find_class("org/fcast/android/sender/GstPopServiceBridge")?;
            env.call_static_method(
                bridge,
                "stop",
                "(Landroid/content/Context;)V",
                &[JValue::Object(&ctx.activity)],
            )?;
            Ok(())
        })();
    }
}

// ── Non-Android stubs ────────────────────────────────────────────────

#[cfg(not(target_os = "android"))]
pub fn request_service_start(_config: &StoredBackendConfig) -> anyhow::Result<()> { Ok(()) }
#[cfg(not(target_os = "android"))]
pub fn request_service_stop() {}
```

Wire the new module in `src/backend/gstpop/mod.rs`:

```rust
// src/backend/gstpop/mod.rs
pub mod backend;
pub mod client;
mod embedded;
pub mod protocol;
pub mod service;          // ← NEW
#[cfg(test)]
mod protocol_tests;

pub use backend::GstPopBackend;
```

## 5.2 Reusable Android context helper

`lib.rs` already converts `vm_as_ptr` / `activity_as_ptr` on demand
in two places (around `src/lib.rs:591-610` and `src/lib.rs:1146-1156`).
Hoist that into a single helper:

```rust
// src/lib.rs — near the other lazy_static!.

#[cfg(target_os = "android")]
pub(crate) struct AndroidCtx {
    pub vm: jni::JavaVM,
    pub activity: jni::objects::JObject<'static>,
}

#[cfg(target_os = "android")]
static ANDROID_APP: once_cell::sync::OnceCell<PlatformApp> = once_cell::sync::OnceCell::new();

#[cfg(target_os = "android")]
pub(crate) fn install_android_app(app: PlatformApp) {
    let _ = ANDROID_APP.set(app);
}

#[cfg(target_os = "android")]
pub(crate) fn android_context() -> anyhow::Result<AndroidCtx> {
    let app = ANDROID_APP.get().ok_or_else(|| anyhow::anyhow!("android app not installed"))?;
    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    let activity_ptr = app.activity_as_ptr() as *mut jni::sys::_jobject;
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr)? };
    let activity = unsafe { jni::objects::JObject::from_raw(activity_ptr) };
    Ok(AndroidCtx { vm, activity })
}
```

Call `install_android_app(app.clone())` at the top of `main_inner`
before the runtime is built. The two existing `vm_as_ptr` /
`activity_as_ptr` blocks can later be refactored to use
`android_context()` for consistency (optional cleanup, not required
for this step to work).

## 5.3 Rewired `BackendLifecycle::apply`

Before — `src/backend/lifecycle.rs:88-99`:

```rust
async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
    config.save(&self.files_dir)?;
    install(build_backend(&config));
    push_state(&weak, crate::MediaBackendState::Probing);
    match current().probe().await {
        Ok(status) => push_status(&weak, status),
        Err(err) => push_error(&weak, &err.to_string()),
    }
    Ok(())
}
```

After:

```rust
async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
    use super::gstpop::{embedded, service};

    // 1. Persist first so a crash before service start still recovers cleanly.
    config.save(&self.files_dir)?;

    // 2. Service-lifecycle side effects. All Android-only; stubs on host.
    match config.kind {
        BackendKind::GstPop if embedded::is_localhost(&config.gstpop_url) => {
            push_state(&weak, crate::MediaBackendState::Starting);
            if let Err(err) = service::request_service_start(&config) {
                // Surface the bridge failure but keep going — probe will
                // likely fail too and the user will see a coherent error.
                tracing::error!(?err, "request_service_start failed");
                push_error(&weak, &format!("service start failed: {err}"));
            }
        }
        BackendKind::GstPop => {
            // Remote URL — no service needed. Tear down anything we own.
            service::request_service_stop();
        }
        BackendKind::Migration => {
            service::request_service_stop();
        }
    }

    // 3. Swap the in-process backend.
    install(build_backend(&config));

    // 4. Probe. For local gst-pop, wait_for_port inside start_embedded
    //    already ensured the listener is up before the service returned,
    //    so the probe should succeed on the first try.
    push_state(&weak, crate::MediaBackendState::Probing);
    match current().probe().await {
        Ok(status) => push_status(&weak, status),
        Err(err) => push_error(&weak, &err.to_string()),
    }
    Ok(())
}
```

## 5.4 Rewired `BackendLifecycle::autostart`

Before — `src/backend/lifecycle.rs:80-87`:

```rust
fn autostart(self: Arc<Self>, weak: Weak<MainWindow>) {
    push_state(&weak, crate::MediaBackendState::Probing);
    tokio::spawn(async move {
        match current().probe().await {
            Ok(status) => push_status(&weak, status),
            Err(err) => push_error(&weak, &err.to_string()),
        }
    });
}
```

After:

```rust
fn autostart(self: Arc<Self>, weak: Weak<MainWindow>) {
    use super::gstpop::{embedded, service};

    // If the persisted config wants a local gst-pop, fire the service
    // up *before* probing. The service start is async (it returns
    // before nativeStart resolves), so the probe call below will catch
    // the still-starting state and retry as needed.
    if self.initial_config.kind == BackendKind::GstPop
        && embedded::is_localhost(&self.initial_config.gstpop_url)
    {
        if let Err(err) = service::request_service_start(&self.initial_config) {
            tracing::error!(?err, "autostart: request_service_start failed");
        }
        push_state(&weak, crate::MediaBackendState::Starting);
    } else {
        push_state(&weak, crate::MediaBackendState::Probing);
    }

    tokio::spawn(async move {
        // Poll until the daemon is up or we give up.
        for attempt in 0..25 {
            match current().probe().await {
                Ok(status) => { push_status(&weak, status); return; }
                Err(err) if attempt < 24 => {
                    tracing::debug!(?err, attempt, "autostart probe retry");
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
                Err(err) => { push_error(&weak, &err.to_string()); return; }
            }
        }
    });
}
```

The retry loop matters: `request_service_start` returns as soon as
`startForegroundService` returns; the *daemon* may still be in
`Starting` state when the first `probe()` runs. Without the loop the
user sees an error pill that clears itself a second later.

## 5.5 Switch-away path: shutdown the outgoing backend

A subtle current bug: `apply` never calls `MediaBackend::shutdown` on
the **outgoing** backend. When switching gst-pop → migration, the
`GstPopBackend`'s cached client connection leaks until the process
dies. Symmetric fix:

```rust
async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
    use super::gstpop::{embedded, service};

    let previous = current();
    config.save(&self.files_dir)?;

    // … service start/stop block from 5.3 …

    // Shut down the *old* backend before we install the new one.
    if previous.kind() != config.kind {
        if let Err(err) = previous.shutdown().await {
            tracing::warn!(?err, "previous backend shutdown failed");
        }
    }

    install(build_backend(&config));
    push_state(&weak, crate::MediaBackendState::Probing);
    match current().probe().await {
        Ok(status) => push_status(&weak, status),
        Err(err) => push_error(&weak, &err.to_string()),
    }
    Ok(())
}
```

`GstPopBackend::shutdown` is already implemented
(`src/backend/gstpop/backend.rs`), so this is purely a wiring fix.

Next: [06-tighten-probe.md](./06-tighten-probe.md).
