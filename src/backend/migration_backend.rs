use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};

use crate::backend::{BackendKind, BackendStatus, MediaBackend};
use crate::migration::runtime::{self, RuntimeHandles};

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
        #[cfg(not(target_os = "android"))]
        {
            // Ensures the in-process runtime threads are running; idempotent.
            tokio::task::spawn_blocking(|| {
                runtime::start_graph_runtime(RuntimeHandles {
                    frame_pair: migration_runtime::FramePair::new(),
                })
            })
            .await
            .context("migration probe: spawn_blocking join")??;
        }

        #[cfg(target_os = "android")]
        {
            if !runtime::is_running() {
                return Ok(BackendStatus {
                    status_text: "Migration runtime stopped".to_string(),
                    error_text: String::new(),
                    is_connected: false,
                });
            }
        }

        let response = self
            .dispatch("getinfo", json!({}))
            .await
            .context("probe: getinfo")?;
        let node_count = response
            .get("info")
            .and_then(|info| info.get("nodes"))
            .and_then(Value::as_object)
            .map(|nodes| nodes.len())
            .unwrap_or(0);

        Ok(BackendStatus {
            status_text: format!("Migration runtime ready - nodes={node_count}"),
            error_text: String::new(),
            is_connected: true,
        })
    }

    async fn dispatch(&self, action: &str, params: Value) -> Result<Value> {
        let action = action.to_owned();
        tokio::task::spawn_blocking(move || -> Result<Value> {
            let payload = json!({ &action: params }).to_string();
            let response_json = runtime::try_handle_command_json(&payload);
            let root: Value = serde_json::from_str(&response_json).with_context(|| {
                format!("failed to parse response for {action}: {response_json}")
            })?;
            let result = root
                .get("result")
                .cloned()
                .ok_or_else(|| anyhow!("{action} missing result field"))?;
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
                    "{action} unsupported result shape: {response_json}"
                )),
            }
        })
        .await
        .context("migration spawn_blocking join")?
    }

    async fn list(&self) -> Result<Value> {
        self.dispatch("getinfo", json!({})).await
    }

    async fn shutdown(&self) -> Result<()> {
        tokio::task::spawn_blocking(runtime::shutdown_graph_runtime)
            .await
            .context("migration shutdown join")??;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn shutdown_is_idempotent() {
        let backend = MigrationBackend::new();
        backend.shutdown().await.expect("first shutdown ok");
        backend.shutdown().await.expect("second shutdown ok");
    }

    #[tokio::test]
    async fn dispatch_surfaces_errors_from_runtime() {
        let backend = MigrationBackend::new();
        let result = backend.dispatch("nonsense", json!({ "id": "nope" })).await;
        assert!(result.is_err(), "{result:?}");
    }
}
