# STEP 13 — Module Reorganisation

**Phase:** 5 (Codebase Restructuring)
**Modified file:** `src/lib.rs`

---

## Goal

Reorganise the Rust source tree into well-defined modules, reduce circular
dependencies, and add module-level documentation.

---

## 1. Target module layout

```text
src/
├── lib.rs                  # Thin root — re-exports + Android JNI entry
├── service/
│   └── mod.rs              # ServiceManager trait + ServiceOptions   (STEP 01)
├── srt/
│   └── mod.rs              # SrtSourceManager                        (STEP 09)
├── overlay/
│   └── mod.rs              # OverlayManager + image loading          (STEP 10)
├── backend/
│   ├── mod.rs              # MediaBackend trait + BACKEND singleton
│   ├── kind.rs             # BackendKind enum
│   ├── lifecycle.rs        # BackendLifecycle
│   ├── persistence.rs      # StoredBackendConfig
│   ├── migration_backend.rs
│   └── gstpop/
│       ├── mod.rs
│       ├── backend.rs      # GstPopBackend (MediaBackend impl)
│       ├── client.rs       # WebSocket JSON-RPC client
│       ├── embedded.rs     # In-process gst-pop server
│       ├── protocol.rs
│       └── service.rs      # GstPopServiceManager                   (STEP 02)
├── migration/
│   ├── mod.rs
│   ├── runtime.rs
│   ├── protocol.rs
│   ├── messages.rs
│   ├── media_bridge.rs
│   ├── node_manager.rs
│   ├── service.rs          # MigrationServiceManager                (STEP 03)
│   └── nodes/
│       ├── mod.rs
│       ├── source.rs
│       ├── destination.rs
│       ├── mixer.rs        # + overlay integration                  (STEP 11)
│       ├── screen_capture.rs
│       ├── video_generator.rs
│       └── control.rs
├── log_ring.rs
└── whep_signaller_compat.rs
```

## 2. Update `lib.rs`

```rust
// src/lib.rs  — cleaned-up module declarations

// ── Platform imports (unchanged, keep cfg gates) ─────────────────────
#[cfg(target_os = "android")]
use anyhow::bail;
use anyhow::Result;
// ... (existing imports stay the same) ...

// ── Module declarations ──────────────────────────────────────────────
pub mod log_ring;

/// Service lifecycle abstraction (STEP 01).
pub mod service;

/// SRT source management with health monitoring (STEP 09).
pub mod srt;

/// Image overlay composition (STEP 10).
pub mod overlay;

/// Media backend trait, configuration, and lifecycle.
mod backend;

/// Migration runtime — in-process media engine.
pub mod migration;

/// WHEP signaller compatibility shim.
mod whep_signaller_compat;

// ── Re-exports for convenience ───────────────────────────────────────
pub use backend::{BackendKind, MediaBackend, MigrationBackend};
pub use backend::gstpop::GstPopBackend;
```

## 3. Eliminate circular dependency paths

The current code has these cross-references that create implicit coupling:

| From | To | Via |
|------|----|-----|
| `backend::lifecycle` | `backend::gstpop::service` | `request_service_start()` |
| `backend::gstpop::service` | `backend::persistence` | `StoredBackendConfig` |
| `lib.rs` (JNI entry) | `backend::*` | Multiple direct calls |

**Resolution strategy:**

1. `lifecycle.rs` should depend on `service::ServiceManager` trait, not
   concrete `gstpop::service` functions.  Replace:
   ```rust
   // Before
   service::request_service_start(&config);
   // After
   service_manager.start().await;
   ```

2. `StoredBackendConfig` can stay in `backend::persistence` — it is a
   data struct with no behaviour that creates cycles.

3. JNI entry points in `lib.rs` should delegate to a thin
   `backend::lifecycle::init()` function instead of reaching into
   sub-modules directly.

## 4. Add module-level doc comments

Each `mod.rs` should start with a `//!` doc comment:

```rust
// src/service/mod.rs
//! Service lifecycle abstraction layer.
//!
//! Provides [`ServiceManager`] — a uniform interface to start, stop,
//! and health-check any managed background service (gst-pop, migration
//! runtime, future services).

// src/srt/mod.rs
//! SRT source management with health monitoring and auto-reconnection.

// src/overlay/mod.rs
//! Image overlay composition for SRT video sources.
//! Manages overlay images (position, size, alpha, z-order) and
//! translates them into GStreamer compositor pads.
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Add doc comments to each module's `mod.rs` | all `mod.rs` files |
| 2 | Update `lib.rs` module declarations to match target layout | `lib.rs` |
| 3 | Replace direct `gstpop::service::request_*` calls with `ServiceManager` | `lifecycle.rs` |
| 4 | Extract JNI init into `backend::lifecycle::init()` if not already | `lib.rs` |
| 5 | Run `cargo check` and fix any import paths | terminal |
| 6 | Run `cargo doc --no-deps` to verify doc comments render | terminal |

---

## Notes

* This step can be done incrementally — move one module at a time and keep
  `cargo check` passing after each move.
* The `pub mod` vs `mod` visibility is intentional: `service`, `srt`,
  `overlay`, and `migration` are `pub` so tests and future crates can
  access them.  `backend` is `mod` (private) with selected re-exports.
