# STEP 03 — Migration Runtime Service Wrapper

**Phase:** 1 (Service Abstraction Layer)
**New file:** `src/migration/service.rs`

---

## Goal

Wrap the existing migration runtime (`runtime::start_graph_runtime`,
`runtime::shutdown_graph_runtime`) behind the `ServiceManager` trait so
that it can be toggled and health-checked uniformly.

---

## 1. Create the wrapper

```rust
// src/migration/service.rs

use anyhow::Result;

use crate::migration::runtime;
use crate::service::{ServiceManager, ServiceOptions, ServiceStatus};

pub struct MigrationServiceManager {
    options: parking_lot::RwLock<ServiceOptions>,
}

impl MigrationServiceManager {
    pub fn new(options: ServiceOptions) -> Self {
        Self {
            options: parking_lot::RwLock::new(options),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for MigrationServiceManager {
    fn name(&self) -> &str {
        "migration"
    }

    fn options(&self) -> &ServiceOptions {
        self.options.read().clone()
    }

    fn set_options(&mut self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> Result<ServiceStatus> {
        let opts = self.options.read().clone();
        if !opts.enabled {
            return Ok(ServiceStatus {
                running: false,
                healthy: true,
                status_text: "migration runtime disabled".into(),
                error_text: String::new(),
            });
        }

        // Migration runtime start is synchronous — run off the async executor.
        tokio::task::spawn_blocking(runtime::start_graph_runtime)
            .await
            .map_err(|e| anyhow::anyhow!("join error: {e}"))??;

        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: "migration runtime started".into(),
            error_text: String::new(),
        })
    }

    async fn stop(&self) -> Result<ServiceStatus> {
        tokio::task::spawn_blocking(runtime::shutdown_graph_runtime)
            .await
            .map_err(|e| anyhow::anyhow!("join error: {e}"))??;

        Ok(ServiceStatus {
            running: false,
            healthy: true,
            status_text: "migration runtime stopped".into(),
            error_text: String::new(),
        })
    }

    async fn status(&self) -> Result<ServiceStatus> {
        // The migration runtime exposes GRAPH_REFRESH_RUNNING.
        // Quick check: try a no-op command.
        let payload = r#"{"getinfo":{}}"#;
        let response = runtime::try_handle_command_json(payload);
        let healthy = response.contains("\"result\"");

        Ok(ServiceStatus {
            running: healthy,
            healthy,
            status_text: if healthy {
                "migration runtime responsive".into()
            } else {
                "migration runtime not responding".into()
            },
            error_text: String::new(),
        })
    }
}
```

## 2. Register the module

```rust
// src/migration/mod.rs  (add a new line)
pub mod service;
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `src/migration/service.rs` with the code above | new file |
| 2 | Add `pub mod service;` to `src/migration/mod.rs` | line ~7 |
| 3 | Verify `cargo check` passes | terminal |

---

## Notes

* The on-demand startup semantics are preserved: `start()` calls
  `start_graph_runtime()` which is already idempotent (checks the
  `GRAPH_REFRESH_RUNNING` flag).
* `status()` uses the lightweight `getinfo` command.  If the runtime is
  not started, `try_handle_command_json` returns an error-shaped JSON
  string, so `contains("\"result\"")` correctly reports unhealthy.
* A future health-monitoring task (Phase 1.3 stretch goal) can call
  `status()` on a timer and push results into the Slint Bridge.
