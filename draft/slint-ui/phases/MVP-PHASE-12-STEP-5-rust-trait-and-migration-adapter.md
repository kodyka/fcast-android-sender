# MVP-PHASE-12 — Step 5: Rust `MediaBackend` trait + `MigrationBackend` adapter

> Part 5 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-4](./MVP-PHASE-12-STEP-4-settings-page-section.md).
> Next: [STEP-6](./MVP-PHASE-12-STEP-6-gstpop-websocket-client.md).

---

## 0. Goal of this step

Introduce the **`MediaBackend` trait** — the seam every later phase
will use when migrating call sites off direct `run_graph_command(...)`
calls — and ship the first impl: `MigrationBackend`, which wraps the
existing `crate::migration::runtime` interface so the trait is
behaviour-equivalent to the status quo.

No call sites move in this step. The trait and the migration impl
sit behind a global `BackendSelector`; until STEP-7 lands the gst-pop
impl and STEP-8 wires the lifecycle handler, the selector always
returns the migration impl, so runtime behaviour is unchanged.

---

## 1. Module layout

Create the new directory tree (no `Cargo.toml` changes — single crate):

```
src/
  backend/
    mod.rs                ← trait + global selector + helpers
    kind.rs               ← BackendKind enum, mirror of Slint's MediaBackendKind
    migration_backend.rs  ← MigrationBackend impl (this step)
    gstpop/               ← STEP-6 and STEP-7 populate
      mod.rs
```

Register the module in `src/lib.rs` (top, near the other `mod`
declarations — see `src/lib.rs:30-50` for the existing list):

```rust
mod backend;
```

---

## 2. `BackendKind` (mirror of Slint enum)

In `src/backend/kind.rs`:

```rust
//! Rust mirror of the Slint `MediaBackendKind` enum.
//!
//! Defined here (not derived from `slint::generated`) so backend code
//! can be unit-tested without spinning up a `slint::ComponentHandle`.
//! Conversion to/from the generated enum is in
//! `src/backend/mod.rs::into_slint` / `::from_slint`.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendKind {
    #[default]
    Migration,
    GstPop,
}

impl BackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            BackendKind::Migration => "migration",
            BackendKind::GstPop    => "gst-pop",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "migration"          => Some(BackendKind::Migration),
            "gst-pop" | "gstpop" => Some(BackendKind::GstPop),
            _ => None,
        }
    }
}
```

> Rust-side `serde` derive lets STEP-8 persist `{ "backend":
> "gst-pop", "url": "ws://...", "api_key": null, "pipeline_id": "0" }`
> in the settings file with no manual `Display` / `FromStr` glue.

---

## 3. `MediaBackend` trait (minimal surface)

In `src/backend/mod.rs`:

```rust
//! Backend-selector seam for the app's media-pipeline driver.
//!
//! See draft/slint-ui/phases/MVP-PHASE-12-gstpop-backend-toggle.md.
//!
//! This module owns:
//!   * `MediaBackend` trait — the abstraction
//!   * `BackendKind` enum   — re-exported from kind.rs
//!   * `MigrationBackend`   — re-exported from migration_backend.rs
//!   * `BACKEND` static     — `parking_lot::RwLock<Box<dyn MediaBackend>>`
//!
//! Adding gst-pop is a STEP-7 follow-up in the `gstpop` submodule.

mod kind;
mod migration_backend;
pub mod gstpop;          // populated in STEP-6

pub use kind::BackendKind;
pub use migration_backend::MigrationBackend;

use anyhow::Result;
use parking_lot::RwLock;
use serde_json::Value;
use std::sync::Arc;

/// Snapshot of a backend's last known reachability.
#[derive(Clone, Debug, Default)]
pub struct BackendStatus {
    pub status_text: String,  // e.g. "Migration runtime ready — nodes=2"
    pub error_text:  String,  // empty when ok
}

/// Operations the app's UI and call sites need from *any* media-pipeline backend.
///
/// Methods are `async` because the gst-pop impl issues network calls.
/// `MigrationBackend` implements every method synchronously via
/// `tokio::task::spawn_blocking` (the migration runtime is sync).
#[async_trait::async_trait]
pub trait MediaBackend: Send + Sync {
    fn kind(&self) -> BackendKind;

    /// Lightweight reachability check. Should be cheap (<200 ms).
    async fn probe(&self) -> Result<BackendStatus>;

    /// Send a single command and return the deserialized result.
    /// For Migration this is the existing run_graph_command verb +
    /// params shape. For gst-pop it's a JSON-RPC method + params.
    /// STEP-7 documents the verb mapping.
    async fn dispatch(&self, action: &str, params: Value) -> Result<Value>;

    /// Optional: list active pipelines / node graphs. Returns an opaque
    /// Value so the UI can render whatever the backend gives back.
    async fn list(&self) -> Result<Value>;

    /// Optional: stop everything the backend is doing — for "Disconnect"
    /// or app-shutdown. MigrationBackend calls shutdown_graph_runtime();
    /// GstPopBackend disconnects the WS and removes its pipeline.
    async fn shutdown(&self) -> Result<()>;
}

/// Process-wide active backend. Replaced on Apply (STEP-8).
static BACKEND: once_cell::sync::Lazy<RwLock<Arc<dyn MediaBackend>>> =
    once_cell::sync::Lazy::new(|| {
        // Default to MigrationBackend so first launch behaviour is
        // bit-identical with pre-PHASE-12 behaviour.
        RwLock::new(Arc::new(MigrationBackend::new()))
    });

/// Read-only handle to the currently active backend.
pub fn current() -> Arc<dyn MediaBackend> {
    BACKEND.read().clone()
}

/// Atomically swap the active backend. Called from STEP-8's
/// `apply-media-backend` handler.
pub fn install(new_backend: Arc<dyn MediaBackend>) {
    *BACKEND.write() = new_backend;
}
```

### 3.1 Cargo dependency notes for this step

`once_cell` and `parking_lot` are **already in tree**
(`parking_lot = "0.12"` at `Cargo.toml:28`; `once_cell` is pulled in
transitively by `slint`). `async-trait` is **not yet** in tree —
STEP-9 adds:

```toml
async-trait = "0.1"
```

If pulling in `async-trait` is undesirable for build-time reasons,
the alternative is to hand-write the `impl Future` futures (verbose
but possible). STEP-9 chooses `async-trait` because the trait is
~4 methods and the readability win outweighs the dependency.

### 3.2 Why an `Arc<dyn MediaBackend>`, not a `Box<dyn MediaBackend>`

The trait object is shared across many `Bridge` callback handlers
that fire on different threads. `Box` would be moved into a single
handler at install-time and immediately need re-boxing on the next
fire. `Arc` is the conventional pattern for "one immutable handle,
many readers".

### 3.3 Why `async-trait`, not raw async fn in traits

Stable Rust (as of the project's 1.74+ edition) supports async fns
in traits — but the syntax doesn't yet allow `dyn` trait objects to
have async methods (the compiler errors with "`async fn` cannot be
used in a `dyn` trait"). Since `current()` returns a `dyn` trait
object, we use `#[async_trait::async_trait]` until stable Rust
catches up. The trait is small enough that the heap-allocated
`Pin<Box<dyn Future>>` overhead is irrelevant.

---

## 4. `MigrationBackend` impl

In `src/backend/migration_backend.rs`:

```rust
//! `MediaBackend` impl that wraps the in-process migration runtime.
//!
//! Every method here is a thin shim over the existing functions in
//! `crate::migration::runtime` (`src/migration/runtime.rs`). The
//! migration runtime is synchronous and runs on a dedicated thread
//! pool of its own; we wrap calls in `tokio::task::spawn_blocking`
//! so the async surface this trait demands is satisfied without
//! blocking a tokio worker.

use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendKind, BackendStatus, MediaBackend};
use crate::migration::runtime;

#[derive(Debug, Default)]
pub struct MigrationBackend;

impl MigrationBackend {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait::async_trait]
impl MediaBackend for MigrationBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Migration
    }

    async fn probe(&self) -> Result<BackendStatus> {
        // Calling getinfo lazily starts the runtime via the
        // existing try_handle_command_json path (see
        // src/migration/runtime.rs:349-356 + lib.rs:267 onward).
        let response = self.dispatch("getinfo", json!({})).await
            .context("probe: getinfo")?;

        // Successful getinfo returns either Value::String("success")
        // or an object with a "nodes" map — see lib.rs:259-264.
        let node_count = response
            .get("info")
            .and_then(|info| info.get("nodes"))
            .and_then(Value::as_object)
            .map(|nodes| nodes.len())
            .unwrap_or(0);

        Ok(BackendStatus {
            status_text: format!("Migration runtime ready — nodes={node_count}"),
            error_text:  String::new(),
        })
    }

    async fn dispatch(&self, action: &str, params: Value) -> Result<Value> {
        // The migration runtime is synchronous + thread-safe. Wrap
        // the call so the tokio worker isn't blocked.
        let action = action.to_owned();
        let payload = json!({ &action: params }).to_string();

        tokio::task::spawn_blocking(move || -> Result<Value> {
            let response_json = runtime::try_handle_command_json(&payload);
            let root: Value = serde_json::from_str(&response_json)
                .with_context(|| format!(
                    "{action} parse failure: raw={response_json}"))?;

            // Match the lib.rs:217-241 result-shape contract.
            let result = root.get("result").cloned()
                .ok_or_else(|| anyhow!(
                    "{action} missing result field; raw={response_json}"))?;

            match &result {
                Value::String(ok) if ok == "success" => Ok(result),
                Value::Object(map) => {
                    if let Some(err) = map.get("error").and_then(Value::as_str) {
                        Err(anyhow!("{action} error: {err}"))
                    } else {
                        Ok(result)
                    }
                }
                _ => Err(anyhow!(
                    "{action} unsupported result shape: {response_json}")),
            }
        })
        .await
        .context("dispatch: spawn_blocking join")?
    }

    async fn list(&self) -> Result<Value> {
        // For the migration backend, list ≡ getinfo (returns every
        // node + every connection).
        self.dispatch("getinfo", json!({})).await
    }

    async fn shutdown(&self) -> Result<()> {
        let _ = tokio::task::spawn_blocking(runtime::shutdown_graph_runtime)
            .await
            .context("shutdown: spawn_blocking join")?
            .map_err(|err| anyhow!("shutdown_graph_runtime: {err}"))?;
        Ok(())
    }
}
```

### 4.1 Why `dispatch("getinfo", json!({}))` rather than a typed
helper

The migration runtime is fully `serde_json` driven (see
`run_graph_command` at `src/lib.rs:217-241`). Typed wrappers
would be welcome, but they don't belong in this phase — STEP-12+
can introduce a `MigrationCommand` enum in a follow-on phase that
both `MigrationBackend::dispatch` and direct callers use. For now
the trait's `dispatch(&str, Value)` matches the runtime's actual
contract 1:1.

### 4.2 Why every call funnels through `dispatch`

`probe`, `list`, and `shutdown` all delegate to `dispatch` (or to
the runtime's own functions). That keeps the migration impl small
enough to audit and ensures the result-shape contract is asserted in
one place — if the runtime ever changes its response shape, only
`dispatch` breaks.

---

## 5. `from_slint` / `into_slint` helpers

Add to the bottom of `src/backend/mod.rs`:

```rust
/// Convert from the Slint-generated enum to the Rust mirror.
///
/// The Slint code generator names the variants `Migration` and
/// `GstPop` (CamelCase) and exposes them under
/// `slint::generated::MediaBackendKind`.
pub fn from_slint(kind: slint_generatedMediaBackendKind) -> BackendKind {
    match kind {
        slint_generatedMediaBackendKind::Migration => BackendKind::Migration,
        slint_generatedMediaBackendKind::GstPop    => BackendKind::GstPop,
    }
}

pub fn into_slint(kind: BackendKind) -> slint_generatedMediaBackendKind {
    match kind {
        BackendKind::Migration => slint_generatedMediaBackendKind::Migration,
        BackendKind::GstPop    => slint_generatedMediaBackendKind::GstPop,
    }
}
```

> The exact import path for the generated enum depends on the
> `slint::include_modules!()` invocation in `src/lib.rs`. The
> conventional path in this repo is:
>
> ```rust
> use crate::MediaBackendKind as slint_generatedMediaBackendKind;
> ```
>
> Adjust the alias to match wherever `include_modules!` resolves to.

---

## 6. Expected diff size

- `src/lib.rs`: +1 line (`mod backend;`).
- `src/backend/mod.rs`: ~110 lines.
- `src/backend/kind.rs`: ~30 lines.
- `src/backend/migration_backend.rs`: ~110 lines.

Total: ~250 lines.

---

## 7. Verification

```sh
# Should compile cleanly even though Cargo.toml hasn't pulled
# async-trait yet — STEP-9 closes that. Until then, gate this
# step's commit on STEP-9 also being applied.
cargo build -p android-sender --target aarch64-linux-android

# Unit test the migration adapter against the in-process runtime.
# This is a smoke test — STEP-9 ships a richer suite.
cargo test --target aarch64-linux-android -p android-sender backend::migration_backend
```

Smoke test (Rust unit test, `src/backend/migration_backend.rs` `#[cfg(test)]`):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn probe_succeeds_when_runtime_is_lazy_initialized() {
        let backend = MigrationBackend::new();
        let status = backend.probe().await.expect("probe should succeed");
        assert!(status.status_text.starts_with("Migration runtime ready"));
        assert!(status.error_text.is_empty());
    }

    #[tokio::test]
    async fn dispatch_round_trips_a_known_command() {
        let backend = MigrationBackend::new();
        let result = backend.dispatch("getinfo", json!({})).await.unwrap();
        // getinfo always returns an object (never the bare "success"
        // string), so an Object assertion is safe.
        assert!(result.is_object(), "result = {result:?}");
    }
}
```

---

## 8. Exit gate

- [ ] `src/backend/` directory exists with `mod.rs`, `kind.rs`,
      `migration_backend.rs`, and an empty `gstpop/` stub.
- [ ] `BackendKind` mirrors the Slint enum 1:1 and serdes
      round-trip.
- [ ] `MediaBackend` trait is declared and has the four methods
      from §3.
- [ ] `MigrationBackend` implements every method via the existing
      `runtime::try_handle_command_json` path.
- [ ] `cargo build` and `cargo test backend::migration_backend`
      both pass (after STEP-9 adds `async-trait`).

Proceed to [STEP-6](./MVP-PHASE-12-STEP-6-gstpop-websocket-client.md).
