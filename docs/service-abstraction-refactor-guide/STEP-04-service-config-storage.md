# STEP 04 — Service Configuration Storage

**Phase:** 2 (Configuration System)
**Modified file:** `src/backend/persistence.rs`

---

## Goal

Extend `StoredBackendConfig` with per-service configuration so the user's
enable/disable and service-mode choices survive app restarts.

---

## 1. Extend the stored config

```rust
// src/backend/persistence.rs  (add alongside existing fields)

use crate::service::{ServiceMode, ServiceOptions};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoredBackendConfig {
    // --- existing fields (unchanged) ---
    pub kind: BackendKind,
    pub gstpop_url: String,
    pub gstpop_api_key: Option<String>,
    pub gstpop_pipeline_id: String,

    // --- NEW: per-service options ---
    #[serde(default)]
    pub gstpop_service: Option<ServiceOptions>,
    #[serde(default)]
    pub migration_service: Option<ServiceOptions>,
    #[serde(default)]
    pub auto_start_services: bool,
    #[serde(default)]
    pub service_mode: ServiceMode,
}
```

Using `Option<ServiceOptions>` + `#[serde(default)]` ensures that loading
an older `backend.json` (without these keys) still deserializes correctly.

## 2. Update `defaults()`

```rust
impl StoredBackendConfig {
    pub fn defaults() -> Self {
        Self {
            kind: BackendKind::Migration,
            gstpop_url: "ws://127.0.0.1:9000".into(),
            gstpop_api_key: None,
            gstpop_pipeline_id: "0".into(),

            gstpop_service: Some(ServiceOptions {
                enabled: true,
                auto_start: true,
                mode: ServiceMode::AndroidService,
            }),
            migration_service: Some(ServiceOptions {
                enabled: true,
                auto_start: true,
                mode: ServiceMode::Embedded,
            }),
            auto_start_services: true,
            service_mode: ServiceMode::Embedded,
        }
    }
}
```

## 3. Helper accessors

```rust
impl StoredBackendConfig {
    /// Convenience: resolved gstpop service options (with defaults).
    pub fn gstpop_opts(&self) -> ServiceOptions {
        self.gstpop_service
            .clone()
            .unwrap_or(ServiceOptions {
                enabled: true,
                auto_start: true,
                mode: ServiceMode::AndroidService,
            })
    }

    /// Convenience: resolved migration service options (with defaults).
    pub fn migration_opts(&self) -> ServiceOptions {
        self.migration_service
            .clone()
            .unwrap_or_default()
    }
}
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Add `ServiceOptions` / `ServiceMode` imports | `persistence.rs` top |
| 2 | Add new fields to `StoredBackendConfig` with `#[serde(default)]` | `persistence.rs` |
| 3 | Update `defaults()` to populate the new fields | `persistence.rs` |
| 4 | Add accessor helpers `gstpop_opts()` and `migration_opts()` | `persistence.rs` |
| 5 | Verify existing `backend.json` files still load (backward-compat) | manual test |
| 6 | Update `BackendLifecycle::autostart` to read `auto_start` from options | `lifecycle.rs:129` |

---

## Notes

* The JSON file is forward-compatible: unknown keys are silently ignored
  by `serde_json::from_slice` with `#[serde(default)]`.
* The `service_mode` top-level field is a convenience for the UI combo box
  (see STEP 05).  The per-service `ServiceOptions.mode` takes precedence
  when the two disagree — document this in the UI tooltip.
