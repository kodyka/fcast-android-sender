use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::client::GstPopClient;
use crate::backend::{BackendKind, BackendStatus, MediaBackend};

pub struct GstPopBackend {
    client: Arc<Mutex<Option<GstPopClient>>>,
    url: String,
    api_key: Option<String>,
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

    pub(crate) async fn raw_call(&self, method: &str, params: Value) -> Result<Value> {
        {
            let mut guard = self.client.lock().await;
            if guard.is_none() {
                *guard = Some(
                    GstPopClient::connect(&self.url, self.api_key.clone())
                        .await
                        .with_context(|| format!("connect to {}", self.url))?,
                );
            }
        }

        let attempt = {
            let guard = self.client.lock().await;
            let client = guard.as_ref().expect("client should be connected");
            client.call(method, params.clone()).await
        };

        match attempt {
            Ok(value) => Ok(value),
            Err(err) => {
                log::info!("gst-pop {method} failed: {err}; dropping cached connection");
                self.client.lock().await.take();
                Err(err)
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
        // Probe is connectivity-only. Daemon lifetime is owned by
        // GstPopService (Android) or by the user (CI / dev machine).
        let info = self
            .raw_call("get_version", json!({}))
            .await
            .context("probe: get_version (is the gst-pop service running?)")?;
        let version = info
            .get("version")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        let count_value = self
            .raw_call("get_pipeline_count", json!({}))
            .await
            .context("probe: get_pipeline_count")?;
        let count = count_value
            .get("count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        Ok(BackendStatus {
            status_text: format!("gst-pop {version} - {count} pipeline(s)"),
            error_text: String::new(),
        })
    }

    async fn dispatch(&self, action: &str, params: Value) -> Result<Value> {
        let (method, params) = translate(action, params, &self.pipeline_id)?;
        self.raw_call(method, params).await
    }

    async fn list(&self) -> Result<Value> {
        self.raw_call("list_pipelines", json!({})).await
    }

    async fn shutdown(&self) -> Result<()> {
        let _ = self
            .raw_call(
                "remove_pipeline",
                json!({ "pipeline_id": self.pipeline_id.clone() }),
            )
            .await;
        if let Some(client) = self.client.lock().await.take() {
            client.close().await;
        }
        Ok(())
    }
}

fn translate(action: &str, params: Value, pipeline_id: &str) -> Result<(&'static str, Value)> {
    let result = match action {
        "start" => ("play", merge_pid(params, pipeline_id)),
        "stop" => ("stop", merge_pid(params, pipeline_id)),
        "pause" => ("pause", merge_pid(params, pipeline_id)),
        "getinfo" => ("get_pipeline_info", merge_pid(params, pipeline_id)),
        "list" => ("list_pipelines", json!({})),
        "remove" => ("remove_pipeline", merge_pid(params, pipeline_id)),
        "createpipeline" => ("create_pipeline", params),
        "createsource" | "createmixer" | "createdestination" | "connect" | "disconnect" => {
            bail!(
                "gst-pop backend does not support migration verb `{action}` - re-pose as createpipeline {{description}}"
            );
        }
        other => return Ok((leak_static(other), params)),
    };
    Ok(result)
}

fn merge_pid(mut params: Value, pipeline_id: &str) -> Value {
    if let Value::Object(map) = &mut params {
        if !map.contains_key("pipeline_id") {
            map.insert("pipeline_id".into(), Value::String(pipeline_id.to_owned()));
        }
        params
    } else if params.is_null() {
        json!({ "pipeline_id": pipeline_id })
    } else {
        json!({ "pipeline_id": pipeline_id, "value": params })
    }
}

fn leak_static(value: &str) -> &'static str {
    Box::leak(value.to_owned().into_boxed_str())
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message};

    #[tokio::test]
    #[ignore = "requires a local gst-pop daemon on ws://127.0.0.1:9000"]
    async fn probe_against_docker() {
        let backend = GstPopBackend::new("ws://127.0.0.1:9000".into(), None, "0".into());
        let status = backend.probe().await.expect("probe should succeed");
        assert!(status.status_text.starts_with("gst-pop "));
    }

    #[test]
    fn translate_passes_through_native_verbs() {
        let (method, params) = translate("snapshot", json!({"details":"all"}), "0").unwrap();
        assert_eq!(method, "snapshot");
        assert_eq!(params["details"], "all");
    }

    #[test]
    fn translate_maps_start_to_play() {
        let (method, params) = translate("start", json!({}), "0").unwrap();
        assert_eq!(method, "play");
        assert_eq!(params["pipeline_id"], "0");
    }

    #[tokio::test]
    async fn round_trip_against_echo_server() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut ws = accept_async(stream).await.unwrap();
            while let Some(msg) = ws.next().await {
                if let Ok(Message::Text(text)) = msg {
                    let req: Value = serde_json::from_str(text.as_str()).unwrap();
                    let id = req["id"].as_str().unwrap().to_owned();
                    let reply = json!({
                        "id": id,
                        "result": { "version": "test-0.0", "count": 0 }
                    });
                    let _ = ws.send(Message::Text(reply.to_string().into())).await;
                }
            }
        });

        let backend = GstPopBackend::new(format!("ws://127.0.0.1:{port}"), None, "0".into());
        let value = backend.raw_call("get_version", json!({})).await.unwrap();
        assert_eq!(value["version"], "test-0.0");
    }
}
