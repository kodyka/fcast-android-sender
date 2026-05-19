# MVP-PHASE-12 — Step 7: `GstPopBackend` impl on top of `GstPopClient`

> Part 7 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-6](./MVP-PHASE-12-STEP-6-gstpop-websocket-client.md).
> Next: [STEP-8](./MVP-PHASE-12-STEP-8-lifecycle-and-status-writeback.md).

---

## 0. Goal of this step

Implement the second `MediaBackend` impl (`GstPopBackend`), built on
top of the `GstPopClient` from STEP-6. The impl translates the
trait's `dispatch(action, params)` calls into the daemon's JSON-RPC
method calls, mapping migration-style verbs onto gst-pop methods
where it makes sense and surfacing the rest as gst-pop-native verbs
(`create_pipeline`, `set_state`, …).

After this step the gst-pop backend is reachable from the global
selector (`backend::install(...)`), but the *settings UI* still
defaults to migration — STEP-8 wires the `apply-media-backend`
callback that actually flips the selector.

---

## 1. Files

```
src/backend/gstpop/
  mod.rs            ← add `pub mod backend;` + `pub use backend::GstPopBackend;`
  backend.rs        ← new (this step)
```

---

## 2. `GstPopBackend` skeleton

`src/backend/gstpop/backend.rs`:

```rust
//! `MediaBackend` implementation that talks to a gst-pop daemon over
//! WebSocket.
//!
//! The trait surface is the same as `MigrationBackend` (see
//! `src/backend/migration_backend.rs`). The verbs accepted by
//! `dispatch` are a *superset* of gst-pop's native JSON-RPC method
//! names — some migration-style verbs are translated for
//! cross-backend symmetry; see §3.

use std::sync::Arc;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use crate::backend::{BackendKind, BackendStatus, MediaBackend};
use super::client::GstPopClient;

pub struct GstPopBackend {
    /// Lazily-opened client. `Mutex` so reconnects on transient
    /// failures don't race with concurrent dispatches.
    client:      Arc<Mutex<Option<GstPopClient>>>,
    url:         String,
    api_key:     Option<String>,
    pipeline_id: String,
}

impl GstPopBackend {
    pub fn new(url: String, api_key: Option<String>, pipeline_id: String) -> Self {
        Self {
            client: Arc::new(Mutex::new(None)),
            url,
            api_key,
            pipeline_id,
        }
    }

    /// Borrow or open the connection. Holds the mutex for the
    /// duration of the connect — concurrent callers serialize but
    /// only the first one pays the connection cost.
    async fn with_client<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(GstPopClient) -> Fut + Clone,
        Fut: std::future::Future<Output = Result<(T, GstPopClient)>>,
    {
        let mut guard = self.client.lock().await;
        let client = match guard.take() {
            Some(c) => c,
            None => GstPopClient::connect(&self.url, self.api_key.clone()).await
                .with_context(|| format!("connect to {}", self.url))?,
        };
        let (value, client) = f(client).await?;
        *guard = Some(client);
        Ok(value)
    }

    /// Convenience: one-shot call with auto-reconnect on first
    /// failure. Returns the JSON-RPC result Value as the trait
    /// requires.
    async fn raw_call(&self, method: &str, params: Value) -> Result<Value> {
        // First attempt against the cached connection.
        let attempt = {
            let mut guard = self.client.lock().await;
            if guard.is_none() {
                *guard = Some(
                    GstPopClient::connect(&self.url, self.api_key.clone()).await
                        .with_context(|| format!("connect to {}", self.url))?,
                );
            }
            // Safety: just inserted above.
            guard.as_ref().unwrap().call(method, params.clone()).await
        };

        match attempt {
            Ok(v)  => Ok(v),
            Err(e) => {
                log::info!("gst-pop {method} failed: {e}; dropping cached connection");
                // Drop cached client; next call will reconnect.
                self.client.lock().await.take();
                Err(e)
            }
        }
    }
}

#[async_trait::async_trait]
impl MediaBackend for GstPopBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::GstPop
    }

    async fn probe(&self) -> Result<BackendStatus> {
        let info = self.raw_call("get_version", json!({})).await
            .context("probe: get_version")?;
        let version = info.get("version").and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let count_value = self.raw_call("get_pipeline_count", json!({})).await
            .context("probe: get_pipeline_count")?;
        let count = count_value.get("count").and_then(Value::as_u64).unwrap_or(0);
        Ok(BackendStatus {
            status_text: format!("gst-pop {version} — {count} pipeline(s)"),
            error_text:  String::new(),
        })
    }

    async fn dispatch(&self, action: &str, params: Value) -> Result<Value> {
        // Translate migration-style verbs to gst-pop methods where
        // there's a sensible mapping; otherwise pass through.
        // See §3 for the full table.
        let (method, params) = translate(action, params, &self.pipeline_id)?;
        self.raw_call(method, params).await
    }

    async fn list(&self) -> Result<Value> {
        self.raw_call("list_pipelines", json!({})).await
    }

    async fn shutdown(&self) -> Result<()> {
        // Best-effort: remove the bound pipeline and drop the
        // cached connection. We don't kill the daemon — that's a
        // user concern (systemctl / docker stop).
        let _ = self.raw_call(
            "remove_pipeline",
            json!({ "pipeline_id": self.pipeline_id }),
        ).await;
        if let Some(client) = self.client.lock().await.take() {
            client.close().await;
        }
        Ok(())
    }
}
```

---

## 3. Verb translation table

The migration backend speaks **node-graph** verbs (`createsource`,
`createmixer`, `connect`); gst-pop speaks **pipeline-string** verbs
(`create_pipeline { description }`). They are **not 1:1**. For
PHASE-12, `dispatch` translates the small subset of verbs the app
fires today; everything else is passed through to gst-pop literally.
Later phases that migrate more call sites can extend this table.

```rust
/// Translate a trait-level (action, params) pair into the
/// (method, params) pair the daemon expects.
///
/// `pipeline_id` is the backend's bound pipeline (settable in the
/// settings page; default "0").
fn translate(
    action:      &str,
    params:      Value,
    pipeline_id: &str,
) -> Result<(&'static str, Value)> {
    let bound = json!({ "pipeline_id": pipeline_id });

    let result: (&'static str, Value) = match action {
        // ── 1:1 mappings ──────────────────────────────────────────────
        "start"          => ("play",                merge_pid(params, pipeline_id)),
        "stop"           => ("stop",                merge_pid(params, pipeline_id)),
        "pause"          => ("pause",               merge_pid(params, pipeline_id)),
        "getinfo"        => ("get_pipeline_info",   merge_pid(params, pipeline_id)),
        "list"           => ("list_pipelines",      json!({})),
        "remove"         => ("remove_pipeline",     merge_pid(params, pipeline_id)),

        // ── Composer verbs (need a pre-built pipeline string) ─────────
        // For these, `params.description` is expected to be a
        // gst-launch expression that *somebody* (the caller, or a
        // future composer) has already built.
        "createpipeline" => ("create_pipeline",     params),

        // ── No native gst-pop equivalent ──────────────────────────────
        // The migration node-graph verbs (createsource, createmixer,
        // connect, …) don't map to gst-pop's single-string pipeline
        // model. We refuse them with a clear error so the call site
        // knows the verb is migration-only — a future composer phase
        // can turn each node-graph mutation into a fresh
        // create_pipeline + remove_pipeline pair.
        "createsource" | "createmixer" | "createdestination"
        | "connect"    | "disconnect" => {
            bail!(
                "gst-pop backend does not support migration verb `{action}` \
                 — re-pose as createpipeline {{description}}"
            );
        }

        // ── Pass-through for gst-pop-native methods ────────────────────
        other => return Ok((leak_static(other), params)),
    };

    let _ = bound; // silence unused
    Ok(result)
}

/// Merge `pipeline_id` into params if absent.
fn merge_pid(mut params: Value, pipeline_id: &str) -> Value {
    if let Value::Object(map) = &mut params {
        if !map.contains_key("pipeline_id") {
            map.insert("pipeline_id".into(), Value::String(pipeline_id.to_owned()));
        }
    } else if params.is_null() {
        params = json!({ "pipeline_id": pipeline_id });
    }
    params
}

/// Leak the method name to `'static`. Acceptable because:
///   - The set of action verbs the trait sees is bounded.
///   - This is only called for the pass-through branch.
fn leak_static(s: &str) -> &'static str {
    Box::leak(s.to_owned().into_boxed_str())
}
```

> **Why leak?** `async_trait` wants the method-name `&str` to outlive
> the await. The simplest way is `Box::leak` — for a method-name
> string this allocates a few bytes per *unique* pass-through verb
> over the lifetime of the process; bounded by the cardinality of
> gst-pop's API (~14 methods), so the leak is constant-bounded, not
> per-call.

---

## 4. Caveat: verb mapping is a one-way bridge

The translation in §3 is **one-way**: migration verbs that have a
clear gst-pop equivalent become pass-throughs; verbs that depend on
the node-graph model error with a clear message. This is deliberate:

- The trait users that call `dispatch("start", ...)` and
  `dispatch("stop", ...)` work against either backend.
- Users that call `dispatch("createsource", ...)` get a backend
  mismatch error at the dispatch site, prompting them to migrate
  their flow to `createpipeline { description }` (or to refuse to
  run when `current().kind() != Migration`).

The alternative — silently building a pipeline string from a
node-graph spec — is *possible* (see e.g. `gst-launch` syntax for
`videomixer`-based 2-source mix at
`gstreamer-rs/examples/src/bin/mixers/mixer.rs`) but is the subject
of a follow-on phase, not PHASE-12.

---

## 5. Expected diff size

- `src/backend/gstpop/mod.rs`: +2 lines (`pub mod backend; pub use
  backend::GstPopBackend;`).
- `src/backend/gstpop/backend.rs`: ~210 lines.

Total: ~215 lines.

---

## 6. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android

# Local smoke test against the Docker daemon (no Android device
# required). STEP-9 ships the full script.
docker run --rm -d -p 9000:9000 --name gst-pop-smoke ghcr.io/dabrain34/gstpop:latest
cargo test --target aarch64-linux-android backend::gstpop::backend::tests::probe_against_docker -- --ignored
docker rm -f gst-pop-smoke
```

A minimal integration test under `#[cfg(test)]` at the bottom of
`backend.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires a local gst-pop daemon on ws://127.0.0.1:9000"]
    async fn probe_against_docker() {
        let backend = GstPopBackend::new(
            "ws://127.0.0.1:9000".into(),
            None,
            "0".into(),
        );
        let status = backend.probe().await.expect("probe should succeed");
        assert!(status.status_text.starts_with("gst-pop "),
            "got {:?}", status.status_text);
    }

    #[test]
    fn translate_passes_through_native_verbs() {
        let (method, params) = translate("snapshot",
            serde_json::json!({"details":"all"}), "0").unwrap();
        assert_eq!(method, "snapshot");
        assert_eq!(params["details"], "all");
    }

    #[test]
    fn translate_maps_start_to_play() {
        let (method, params) = translate("start",
            serde_json::json!({}), "0").unwrap();
        assert_eq!(method, "play");
        assert_eq!(params["pipeline_id"], "0");
    }

    #[test]
    fn translate_refuses_node_graph_verbs() {
        let err = translate("createmixer", serde_json::json!({}), "0")
            .unwrap_err();
        assert!(err.to_string().contains("createmixer"),
            "{err}");
    }
}
```

---

## 7. Exit gate

- [ ] `GstPopBackend::new` takes (url, api_key, pipeline_id) and
      defers connection until first call.
- [ ] `probe` returns a populated `BackendStatus.status_text` with
      the daemon version and current pipeline count.
- [ ] `dispatch` translates the 6 1:1 verbs in §3 and errors loudly
      on the 5 node-graph verbs.
- [ ] All three `translate` unit tests pass.
- [ ] `probe_against_docker` (ignored by default) passes when a
      local daemon is running.

Proceed to [STEP-8](./MVP-PHASE-12-STEP-8-lifecycle-and-status-writeback.md).
