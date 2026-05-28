//! Legacy command smoke tests triggered from the debug UI.

#[cfg(target_os = "android")]
use serde_json::{json, Value};
#[cfg(target_os = "android")]
use tracing::{info, warn};

#[cfg(target_os = "android")]
use crate::command::http_runner::{
    run_graph_command, run_graph_http_command, start_migrated_command_server,
};

#[cfg(target_os = "android")]
pub(crate) fn run_legacy_http_getinfo_test(bind_addr: &str) -> String {
    if let Err(err) = start_migrated_command_server(bind_addr) {
        return format!("FAIL {err}");
    }

    match run_graph_http_command(bind_addr, json!({ "getinfo": {} })) {
        Ok(info) => {
            let node_count = info
                .get("result")
                .and_then(|result| result.get("info"))
                .and_then(|info| info.get("nodes"))
                .and_then(Value::as_object)
                .map(|nodes| nodes.len())
                .unwrap_or(0);
            format!("PASS legacy getinfo (/command) nodes={node_count}")
        }
        Err(err) => format!("FAIL {err}"),
    }
}

#[cfg(target_os = "android")]
pub(crate) fn run_legacy_http_crossfade_test(bind_addr: &str) -> String {
    if let Err(err) = start_migrated_command_server(bind_addr) {
        return format!("FAIL {err}");
    }

    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let mixer_id = format!("legacy-channel-{millis}");
    let destination_id = format!("legacy-output-{millis}");
    let link_id = format!("{mixer_id}->{destination_id}-{millis}");
    let slot_source_id = format!("legacy-source-slot-{millis}");
    let slot_link_id = format!("{slot_source_id}->{mixer_id}-{millis}");

    let mut mixer_created = false;
    let mut destination_created = false;
    let mut slot_source_created = false;

    let result = (|| -> std::result::Result<String, String> {
        // Derived from scripts_test_api/crossfade.py bootstrap sequence.
        run_graph_http_command(
            bind_addr,
            json!({
                "createmixer": {
                    "id": mixer_id.clone(),
                    "config": {
                        "width": 1280,
                        "height": 720,
                        "sample-rate": 44100
                    }
                }
            }),
        )?;
        mixer_created = true;

        run_graph_http_command(
            bind_addr,
            json!({
                "createdestination": {
                    "id": destination_id.clone(),
                    "family": "LocalPlayback"
                }
            }),
        )?;
        destination_created = true;

        run_graph_http_command(
            bind_addr,
            json!({
                "connect": {
                    "link_id": link_id.clone(),
                    "src_id": mixer_id.clone(),
                    "sink_id": destination_id.clone()
                }
            }),
        )?;
        run_graph_http_command(
            bind_addr,
            json!({
                "start": {
                    "id": destination_id.clone()
                }
            }),
        )?;
        run_graph_http_command(
            bind_addr,
            json!({
                "start": {
                    "id": mixer_id.clone()
                }
            }),
        )?;

        run_graph_http_command(
            bind_addr,
            json!({
                "createvideogenerator": {
                    "id": slot_source_id.clone()
                }
            }),
        )?;
        slot_source_created = true;

        run_graph_http_command(
            bind_addr,
            json!({
                "connect": {
                    "link_id": slot_link_id.clone(),
                    "src_id": slot_source_id.clone(),
                    "sink_id": mixer_id.clone(),
                    "audio": false,
                    "video": true,
                    "config": {
                        "video::zorder": 2,
                        "video::alpha": 1.0,
                        "video::width": 1280,
                        "video::height": 720,
                        "video::sizing-policy": "keep-aspect-ratio"
                    }
                }
            }),
        )?;
        run_graph_http_command(
            bind_addr,
            json!({
                "start": {
                    "id": slot_source_id.clone()
                }
            }),
        )?;

        let info = run_graph_http_command(bind_addr, json!({ "getinfo": {} }))?;
        let node_count = info
            .get("result")
            .and_then(|result| result.get("info"))
            .and_then(|info| info.get("nodes"))
            .and_then(Value::as_object)
            .map(|nodes| nodes.len())
            .unwrap_or(0);
        Ok(format!(
            "legacy crossfade bootstrap ok mixer={mixer_id} destination={destination_id} slot_source={slot_source_id} nodes={node_count}"
        ))
    })();

    if slot_source_created {
        let _ = run_graph_http_command(
            bind_addr,
            json!({
                "remove": {
                    "id": slot_source_id.clone()
                }
            }),
        );
    }
    if destination_created {
        let _ = run_graph_http_command(
            bind_addr,
            json!({
                "remove": {
                    "id": destination_id.clone()
                }
            }),
        );
    }
    if mixer_created {
        let _ = run_graph_http_command(
            bind_addr,
            json!({
                "remove": {
                    "id": mixer_id.clone()
                }
            }),
        );
    }

    match result {
        Ok(success) => format!("PASS {success}"),
        Err(err) => format!("FAIL {err}"),
    }
}

#[cfg(target_os = "android")]
pub(crate) fn run_graph_smoke_test() -> String {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let source_id = format!("slint-smoke-videogen-{millis}");
    let mixer_id = format!("slint-smoke-mixer-{millis}");
    let link_id = format!("slint-smoke-link-{millis}");

    let mut source_created = false;
    let mut mixer_created = false;
    let result = (|| -> std::result::Result<String, String> {
        run_graph_command("createvideogenerator", json!({ "id": source_id.clone() }))?;
        source_created = true;

        run_graph_command(
            "createmixer",
            json!({
                "id": mixer_id.clone(),
                "audio": false,
                "video": true
            }),
        )?;
        mixer_created = true;

        run_graph_command(
            "connect",
            json!({
                "link_id": link_id.clone(),
                "src_id": source_id.clone(),
                "sink_id": mixer_id.clone(),
                "audio": false,
                "video": true
            }),
        )?;
        run_graph_command("start", json!({ "id": mixer_id.clone() }))?;
        run_graph_command("start", json!({ "id": source_id.clone() }))?;

        let info = run_graph_command("getinfo", json!({}))?;
        let node_count = info
            .get("info")
            .and_then(|info| info.get("nodes"))
            .and_then(Value::as_object)
            .map(|nodes| nodes.len())
            .unwrap_or(0);

        Ok(format!(
            "smoke ok source={source_id} mixer={mixer_id} link={link_id} nodes={node_count}"
        ))
    })();

    if source_created {
        let _ = run_graph_command("remove", json!({ "id": source_id.clone() }));
    }
    if mixer_created {
        let _ = run_graph_command("remove", json!({ "id": mixer_id.clone() }));
    }

    match result {
        Ok(success) => format!("PASS {success}"),
        Err(err) => format!("FAIL {err}"),
    }
}

#[cfg(target_os = "android")]
pub(crate) fn log_ui_test_status(test_name: &'static str, status: &str) {
    if status.starts_with("PASS") {
        info!(test = test_name, status = status, "UI test completed");
    } else {
        warn!(test = test_name, status = status, "UI test failed");
    }
}

pub(crate) fn migration_test_log_name(test_id: &str) -> &'static str {
    match test_id {
        "getinfo" => "legacy-getinfo",
        "crossfade" => "legacy-crossfade",
        "smoke" => "graph-smoke",
        _ => "unknown",
    }
}

#[cfg(test)]
mod phase9_dispatch_tests {
    use super::migration_test_log_name;

    #[test]
    fn migration_test_log_name_known_ids() {
        assert_eq!(migration_test_log_name("getinfo"), "legacy-getinfo");
        assert_eq!(migration_test_log_name("crossfade"), "legacy-crossfade");
        assert_eq!(migration_test_log_name("smoke"), "graph-smoke");
    }

    #[test]
    fn migration_test_log_name_unknown_id() {
        assert_eq!(migration_test_log_name(""), "unknown");
        assert_eq!(migration_test_log_name("bogus"), "unknown");
        assert_eq!(migration_test_log_name("GetInfo"), "unknown");
    }

    #[test]
    fn migration_test_id_count_invariant() {
        const KNOWN: &[&str] = &["getinfo", "crossfade", "smoke"];

        for id in KNOWN {
            assert_ne!(
                migration_test_log_name(id),
                "unknown",
                "test id {id} should be in the dispatch table"
            );
        }
    }
}
