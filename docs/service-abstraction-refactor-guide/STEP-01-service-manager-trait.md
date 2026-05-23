# STEP 01 — Service Manager Trait

**Phase:** 1 (Service Abstraction Layer)
**New file:** `src/service/mod.rs`

---

## Goal

Create a `ServiceManager` trait that abstracts both GstPopService and the
Migration Runtime behind a uniform start / stop / status interface so that
either service can be enabled, disabled, or swapped at runtime.

---

## 1. Define the configuration enum

```rust
// src/service/mod.rs

use serde::{Deserialize, Serialize};

/// How the service process is hosted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceMode {
    /// Run the engine inside the app process (current Migration default).
    #[default]
    Embedded,
    /// Managed Android foreground Service (current GstPop default on Android).
    AndroidService,
    /// Connect to a user-supplied external daemon (CI / dev machine).
    External,
}
```

## 2. Define the service options struct

```rust
/// Per-service toggles persisted alongside `StoredBackendConfig`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceOptions {
    pub enabled: bool,
    pub auto_start: bool,
    pub mode: ServiceMode,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_start: true,
            mode: ServiceMode::Embedded,
        }
    }
}
```

## 3. Define the trait

```rust
use anyhow::Result;

/// Lifecycle status reported by a managed service.
#[derive(Clone, Debug, Default)]
pub struct ServiceStatus {
    pub running: bool,
    pub healthy: bool,
    pub status_text: String,
    pub error_text: String,
}

/// Uniform abstraction over any managed background service.
#[async_trait::async_trait]
pub trait ServiceManager: Send + Sync {
    /// Human-readable service name (e.g. "gst-pop", "migration").
    fn name(&self) -> &str;

    /// Current options (enabled, auto-start, mode).
    fn options(&self) -> &ServiceOptions;

    /// Mutate options at runtime.  Implementations should persist if needed.
    fn set_options(&mut self, options: ServiceOptions);

    /// Bring the service up.  Idempotent — calling twice is safe.
    async fn start(&self) -> Result<ServiceStatus>;

    /// Tear the service down.  Idempotent.
    async fn stop(&self) -> Result<ServiceStatus>;

    /// Cheap health-check (no heavy I/O).
    async fn status(&self) -> Result<ServiceStatus>;
}
```

## 4. Register the module

In `src/lib.rs`, add the new module declaration next to the existing
`backend` and `migration` modules:

```rust
// src/lib.rs  (add near line 36)
pub mod service;
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `src/service/mod.rs` with the code above | new file |
| 2 | Add `pub mod service;` to `src/lib.rs` | line ~36 |
| 3 | Verify `cargo check` passes (no consumers yet, trait is unused) | terminal |

---

## Notes

* The trait is `async` because GstPopService start/stop may cross JNI
  boundaries that block.  Migration's implementation can wrap synchronous
  calls in `tokio::task::spawn_blocking`.
* `ServiceOptions` is intentionally a plain struct — it will be embedded
  inside `StoredBackendConfig` in STEP 04.
