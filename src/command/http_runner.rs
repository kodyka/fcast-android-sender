//! Migrated graph command server runners.

#[cfg(target_os = "android")]
use serde_json::{json, Value};

#[cfg(target_os = "android")]
use crate::command::probe::send_http_request;
#[cfg(target_os = "android")]
use crate::platform::gst_init::ensure_gstreamer_initialized;

#[cfg(target_os = "android")]
const MIGRATION_COMMAND_BIND_ENV: &str = "MIGRATION_COMMAND_BIND";

#[cfg(target_os = "android")]
pub(crate) fn start_migrated_command_server(
    bind_addr: &str,
) -> std::result::Result<String, String> {
    ensure_gstreamer_initialized()?;
    std::env::set_var(MIGRATION_COMMAND_BIND_ENV, bind_addr);
    migration_runtime::runtime::start_graph_runtime(migration_runtime::runtime::RuntimeHandles {
        frame_pair: crate::FRAME_PAIR.clone(),
    })
    .map_err(|err| format!("Failed to start migrated graph runtime: {err}"))?;
    let health_body = send_http_request(bind_addr, "GET", "/health", None)?;
    Ok(format!(
        "migrated server active bind={bind_addr} health={}",
        health_body.trim()
    ))
}

#[cfg(target_os = "android")]
pub(crate) fn run_graph_http_command(
    bind_addr: &str,
    payload: Value,
) -> std::result::Result<Value, String> {
    let payload_json = payload.to_string();
    let body = send_http_request(bind_addr, "POST", "/command", Some(&payload_json))?;
    let response: Value = serde_json::from_str(&body)
        .map_err(|err| format!("Failed to parse migrated server response: {err}; raw={body}"))?;
    let result = response
        .get("result")
        .ok_or_else(|| format!("Missing result in migrated server response: {body}"))?;

    if let Some(err) = result.get("error").and_then(Value::as_str) {
        return Err(format!("Migrated server command error: {err}"));
    }

    Ok(response)
}

#[cfg(target_os = "android")]
pub(crate) fn run_graph_command(action: &str, params: Value) -> std::result::Result<Value, String> {
    let payload = json!({ action: params });
    let response_json = migration_runtime::runtime::try_handle_command_json(&payload.to_string());
    let root: Value = serde_json::from_str(&response_json)
        .map_err(|err| format!("{action} parse failure: {err}; raw={response_json}"))?;
    let result = root
        .get("result")
        .cloned()
        .ok_or_else(|| format!("{action} missing result field; raw={response_json}"))?;
    match &result {
        Value::String(ok) if ok == "success" => Ok(result),
        Value::Object(map) => {
            if let Some(err) = map.get("error").and_then(Value::as_str) {
                Err(format!("{action} error: {err}"))
            } else {
                Ok(result)
            }
        }
        _ => Err(format!(
            "{action} unsupported result shape: {response_json}"
        )),
    }
}
