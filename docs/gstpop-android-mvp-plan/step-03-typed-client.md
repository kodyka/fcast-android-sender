# Step 3 — Typed client helpers

**Phase:** 1 — Android MVP
**Priority:** highest
**Depends on:** Step 1 (so tests can spin up an embedded server)
**Unblocks:** Steps 5, 7

## Goal

Expose a `TypedGstPopClient` wrapping the existing `GstPopClient::call(method,
params)` so app code does not build raw JSON for every RPC. Mirror the methods
the Android UI actually needs: pipeline lifecycle and playback control.

This is a **pure façade** — no transport changes, no new dependencies.

## Files touched

- `crates/gstpop-runtime/Cargo.toml` (add `typed-client` feature)
- `crates/gstpop-runtime/src/typed_client.rs` (new)
- `crates/gstpop-runtime/src/lib.rs` (feature-gated re-export)

## Implementation

### 1. Feature flag

Edit `crates/gstpop-runtime/Cargo.toml`:

```toml
[features]
default = []
typed-client = []
```

### 2. The wrapper

Create `crates/gstpop-runtime/src/typed_client.rs`:

```rust
//! Typed convenience façade over `GstPopClient::call(method, params)`.
//!
//! Mirrors the JSON-RPC method names exposed by `gstpop::websocket::manager`.
//! Keep this file dependency-free apart from `serde` and `serde_json` so the
//! mobile build stays small.

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::client::GstPopClient;

#[derive(Debug, Clone, Deserialize)]
pub struct PipelineSummary {
    pub id: String,
    pub description: String,
    pub state: String,
    #[serde(default)]
    pub streaming: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreatePipelineResult {
    pub pipeline_id: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct PositionInfo {
    #[serde(default)]
    pub position_ns: Option<u64>,
    #[serde(default)]
    pub duration_ns: Option<u64>,
    #[serde(default)]
    pub progress: Option<f64>,
}

/// Typed façade. Cheap to construct; share the underlying `GstPopClient`
/// across calls.
pub struct TypedGstPopClient {
    inner: GstPopClient,
}

impl TypedGstPopClient {
    pub fn new(inner: GstPopClient) -> Self {
        Self { inner }
    }

    pub fn inner(&self) -> &GstPopClient {
        &self.inner
    }

    pub async fn create_pipeline(&self, description: impl Into<String>) -> Result<String> {
        let value = self
            .inner
            .call("create_pipeline", json!({ "description": description.into() }))
            .await
            .context("create_pipeline RPC failed")?;
        let parsed: CreatePipelineResult =
            serde_json::from_value(value).context("create_pipeline result shape")?;
        Ok(parsed.pipeline_id)
    }

    pub async fn list_pipelines(&self) -> Result<Vec<PipelineSummary>> {
        let value = self
            .inner
            .call("list_pipelines", json!({}))
            .await
            .context("list_pipelines RPC failed")?;
        serde_json::from_value(value).context("list_pipelines result shape")
    }

    pub async fn play(&self, pipeline_id: Option<&str>) -> Result<()> {
        self.inner
            .call("play", pipeline_params(pipeline_id))
            .await
            .context("play RPC failed")?;
        Ok(())
    }

    pub async fn pause(&self, pipeline_id: Option<&str>) -> Result<()> {
        self.inner
            .call("pause", pipeline_params(pipeline_id))
            .await
            .context("pause RPC failed")?;
        Ok(())
    }

    pub async fn stop(&self, pipeline_id: Option<&str>) -> Result<()> {
        self.inner
            .call("stop", pipeline_params(pipeline_id))
            .await
            .context("stop RPC failed")?;
        Ok(())
    }

    pub async fn remove_pipeline(&self, pipeline_id: &str) -> Result<()> {
        self.inner
            .call("remove_pipeline", json!({ "pipeline_id": pipeline_id }))
            .await
            .context("remove_pipeline RPC failed")?;
        Ok(())
    }

    pub async fn update_pipeline(
        &self,
        pipeline_id: &str,
        description: impl Into<String>,
    ) -> Result<()> {
        self.inner
            .call(
                "update_pipeline",
                json!({
                    "pipeline_id": pipeline_id,
                    "description": description.into(),
                }),
            )
            .await
            .context("update_pipeline RPC failed")?;
        Ok(())
    }

    pub async fn get_position(&self, pipeline_id: Option<&str>) -> Result<PositionInfo> {
        let value = self
            .inner
            .call("get_position", pipeline_params(pipeline_id))
            .await
            .context("get_position RPC failed")?;
        serde_json::from_value(value).context("get_position result shape")
    }

    /// Returns the raw inner client for RPC methods not yet typed here.
    pub async fn raw(&self, method: &str, params: Value) -> Result<Value> {
        self.inner.call(method, params).await
    }
}

fn pipeline_params(pipeline_id: Option<&str>) -> Value {
    match pipeline_id {
        Some(id) => json!({ "pipeline_id": id }),
        None => json!({}),
    }
}
```

### 3. Re-export

Edit `crates/gstpop-runtime/src/lib.rs`:

```rust
#[cfg(feature = "typed-client")]
pub mod typed_client;

#[cfg(feature = "typed-client")]
pub use typed_client::{
    PipelineSummary, PositionInfo, TypedGstPopClient,
};
```

## Tests

Inline unit test for the JSON-shaping logic without needing a server. Add to
`typed_client.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pipeline_params_some() {
        assert_eq!(
            pipeline_params(Some("abc")),
            json!({ "pipeline_id": "abc" })
        );
    }

    #[test]
    fn pipeline_params_none() {
        assert_eq!(pipeline_params(None), json!({}));
    }

    #[test]
    fn position_info_tolerates_missing_fields() {
        let v: PositionInfo = serde_json::from_value(json!({})).unwrap();
        assert!(v.position_ns.is_none());
        assert!(v.duration_ns.is_none());
        assert!(v.progress.is_none());
    }

    #[test]
    fn position_info_parses_full() {
        let v: PositionInfo = serde_json::from_value(json!({
            "position_ns": 1234u64,
            "duration_ns": 56789u64,
            "progress": 0.25,
        }))
        .unwrap();
        assert_eq!(v.position_ns, Some(1234));
        assert_eq!(v.duration_ns, Some(56789));
        assert_eq!(v.progress, Some(0.25));
    }
}
```

End-to-end round-trip tests against a live embedded server live in
[Step 5](./step-05-integration-tests.md).

## Verification

```bash
cargo build -p gstpop-runtime --features typed-client
cargo test  -p gstpop-runtime --features typed-client --lib typed_client
```

## Done when

- `cargo build -p gstpop-runtime --features typed-client` is clean.
- `TypedGstPopClient` is callable from `gstpop_runtime::TypedGstPopClient`.
- Unit tests for parameter shaping and `PositionInfo` parsing pass.
