# STEP 02 — GstPopService Refactor

**Phase:** 1 (Service Abstraction Layer)
**Modified file:** `src/backend/gstpop/service.rs`

---

## Goal

Wrap the existing `request_service_start()` / `request_service_stop()`
functions inside a `ServiceManager` implementation.  Add a configuration
guard so calls become no-ops when the service is disabled, and add a
fallback path to the embedded gst-pop daemon when `ServiceMode::Embedded`
is selected.

---

## 1. Implement `ServiceManager` for GstPopService

```rust
// src/backend/gstpop/service.rs  (add below existing code)

use crate::service::{ServiceManager, ServiceMode, ServiceOptions, ServiceStatus};

pub struct GstPopServiceManager {
    options: parking_lot::RwLock<ServiceOptions>,
}

impl GstPopServiceManager {
    pub fn new(options: ServiceOptions) -> Self {
        Self {
            options: parking_lot::RwLock::new(options),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for GstPopServiceManager {
    fn name(&self) -> &str {
        "gst-pop"
    }

    fn options(&self) -> &ServiceOptions {
        self.options.read().clone()
        // NOTE: clone through the lock — cheap for this small struct.
    }

    fn set_options(&mut self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> anyhow::Result<ServiceStatus> {
        let opts = self.options.read().clone();
        if !opts.enabled {
            return Ok(ServiceStatus {
                running: false,
                healthy: true,
                status_text: "gst-pop service disabled by configuration".into(),
                error_text: String::new(),
            });
        }

        match opts.mode {
            ServiceMode::Embedded => {
                // Delegate to the embedded gst-pop server.
                let status = super::embedded::start_embedded(9000).await;
                Ok(ServiceStatus {
                    running: status.state == super::embedded::EmbeddedState::Running,
                    healthy: status.last_error.is_none(),
                    status_text: format!("embedded gst-pop on port {}", status.port),
                    error_text: status.last_error.unwrap_or_default(),
                })
            }
            ServiceMode::AndroidService => {
                // Existing JNI path — requires a StoredBackendConfig.
                let config = crate::backend::persistence::StoredBackendConfig::defaults();
                request_service_start(&config)?;
                Ok(ServiceStatus {
                    running: true,
                    healthy: true,
                    status_text: "Android service start requested".into(),
                    error_text: String::new(),
                })
            }
            ServiceMode::External => {
                // External daemon — nothing to start, just report ok.
                Ok(ServiceStatus {
                    running: true,
                    healthy: true,
                    status_text: "using external gst-pop daemon".into(),
                    error_text: String::new(),
                })
            }
        }
    }

    async fn stop(&self) -> anyhow::Result<ServiceStatus> {
        let opts = self.options.read().clone();
        match opts.mode {
            ServiceMode::Embedded => {
                let status = super::embedded::stop_embedded().await;
                Ok(ServiceStatus {
                    running: status.state == super::embedded::EmbeddedState::Running,
                    healthy: true,
                    status_text: "embedded gst-pop stopped".into(),
                    error_text: String::new(),
                })
            }
            ServiceMode::AndroidService => {
                request_service_stop();
                Ok(ServiceStatus {
                    running: false,
                    healthy: true,
                    status_text: "Android service stop requested".into(),
                    error_text: String::new(),
                })
            }
            ServiceMode::External => Ok(ServiceStatus {
                running: true,
                healthy: true,
                status_text: "external daemon — stop is a no-op".into(),
                error_text: String::new(),
            }),
        }
    }

    async fn status(&self) -> anyhow::Result<ServiceStatus> {
        let es = super::embedded::embedded_status();
        Ok(ServiceStatus {
            running: es.state == super::embedded::EmbeddedState::Running,
            healthy: es.last_error.is_none(),
            status_text: format!("{:?}", es.state),
            error_text: es.last_error.unwrap_or_default(),
        })
    }
}
```

## 2. Refactor `request_service_start` guard

Add a configuration check at the top of the existing function:

```rust
/// Ask the foreground GstPopService to start the daemon. Idempotent.
/// Now respects the service configuration — returns early if disabled.
#[cfg(target_os = "android")]
pub fn request_service_start(config: &StoredBackendConfig) -> anyhow::Result<()> {
    // ── NEW: skip if the caller has disabled the service ──────────────
    // (StoredBackendConfig gains gstpop_service field in STEP 04.)
    if let Some(ref svc) = config.gstpop_service {
        if !svc.enabled {
            tracing::info!("gst-pop service disabled; skipping start");
            return Ok(());
        }
    }

    // ... existing JNI code unchanged ...
}
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Add `GstPopServiceManager` struct + `impl ServiceManager` to `service.rs` | existing file |
| 2 | Add configuration guard to `request_service_start` | existing function |
| 3 | Update `BackendLifecycle::autostart` to check `ServiceOptions::auto_start` | `src/backend/lifecycle.rs:129` |
| 4 | Verify `cargo check --target aarch64-linux-android` still passes | terminal |

---

## Notes

* The `ServiceOptions` reference inside `StoredBackendConfig` is added in
  STEP 04.  Until then the `config.gstpop_service` field doesn't exist —
  guard with `if let Some(...)` so STEP 02 can land independently.
* On non-Android targets the stubs already return `Ok(())`, so the guard
  is only meaningful on Android.
