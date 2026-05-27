use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use slint::{ComponentHandle, Weak};

use super::persistence::StoredBackendConfig;
use super::{
    current, from_slint, install, into_slint, BackendKind, BackendStatus, MediaBackend,
    MigrationBackend,
};
use crate::backend::gstpop::GstPopBackend;
use crate::MainWindow;

pub struct BackendLifecycle {
    files_dir: PathBuf,
    initial_config: StoredBackendConfig,
}

impl BackendLifecycle {
    pub fn new(files_dir: PathBuf) -> Self {
        let initial_config = StoredBackendConfig::load(&files_dir)
            .unwrap_or_else(|_| StoredBackendConfig::defaults());
        install(build_backend(&initial_config));
        Self {
            files_dir,
            initial_config,
        }
    }

    pub fn register(self: Arc<Self>, ui: &MainWindow) {
        push_config(&ui.as_weak(), &self.initial_config);

        let bridge = ui.global::<crate::Bridge>();

        let apply_lifecycle = Arc::clone(&self);
        let apply_weak = ui.as_weak();
        bridge.on_apply_media_backend(move || {
            let config = read_config_from_bridge(&apply_weak);
            let lifecycle = Arc::clone(&apply_lifecycle);
            let weak = apply_weak.clone();
            tokio::spawn(async move {
                if let Err(err) = lifecycle.apply(config, weak.clone()).await {
                    push_error(&weak, &err.to_string());
                }
            });
        });

        let save_lifecycle = Arc::clone(&self);
        let save_weak = ui.as_weak();
        bridge.on_save_media_backend_settings(move || {
            let config = read_config_from_bridge(&save_weak);
            let lifecycle = Arc::clone(&save_lifecycle);
            let weak = save_weak.clone();
            tokio::spawn(async move {
                match config.save(&lifecycle.files_dir) {
                    Ok(()) => push_saved(&weak),
                    Err(err) => push_error(&weak, &format!("save failed: {err}")),
                }
            });
        });

        let probe_weak = ui.as_weak();
        bridge.on_probe_media_backend(move || {
            let config = read_config_from_bridge(&probe_weak);
            let weak = probe_weak.clone();
            tokio::spawn(async move {
                push_state(&weak, crate::MediaBackendState::Probing);
                let backend = build_backend(&config);
                match backend.probe().await {
                    Ok(status) => push_status(&weak, status),
                    Err(err) => push_error(&weak, &err.to_string()),
                }
            });
        });

        // ── Start / Stop service ──────────────────────────────────────────────
        let start_weak = ui.as_weak();
        bridge.on_start_gstpop_service(move || {
            let config = read_config_from_bridge(&start_weak);
            let weak = start_weak.clone();
            tokio::spawn(async move {
                push_state(&weak, crate::MediaBackendState::Starting);
                if let Err(err) = super::gstpop::service::request_service_start(&config) {
                    push_error(&weak, &format!("service start failed: {err}"));
                }
            });
        });

        let stop_weak = ui.as_weak();
        bridge.on_stop_gstpop_service(move || {
            super::gstpop::service::request_service_stop();
            let weak = stop_weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                let bridge = ui.global::<crate::Bridge>();
                bridge.set_gstpop_service_state("stopping".into());
            });
        });

        // ── Migration runtime service start / stop ──────────────────────────────
        let start_mig_weak = ui.as_weak();
        bridge.on_start_migration_runtime_service(move || {
            let weak = start_mig_weak.clone();
            tokio::spawn(async move {
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.global::<crate::Bridge>()
                        .set_migration_runtime_service_state("starting".into());
                });
                if let Err(err) = crate::migration_service::request_service_start() {
                    tracing::error!(?err, "request_service_start (migration runtime)");
                    let _ = weak.upgrade_in_event_loop(move |ui| {
                        ui.global::<crate::Bridge>()
                            .set_migration_runtime_service_state("error".into());
                    });
                }
            });
        });

        let stop_mig_weak = ui.as_weak();
        bridge.on_stop_migration_runtime_service(move || {
            crate::migration_service::request_service_stop();
            let weak = stop_mig_weak.clone();
            let _ = weak.upgrade_in_event_loop(move |ui| {
                ui.global::<crate::Bridge>()
                    .set_migration_runtime_service_state("stopping".into());
            });
        });

        // ── 1Hz daemon status poller ──────────────────────────────────────────
        let poll_weak = ui.as_weak();
        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(std::time::Duration::from_millis(1000));
            loop {
                ticker.tick().await;
                let status = super::gstpop::embedded::embedded_status();
                let state_str: &'static str = match status.state {
                    super::gstpop::embedded::EmbeddedState::Stopped => "stopped",
                    super::gstpop::embedded::EmbeddedState::Starting => "starting",
                    super::gstpop::embedded::EmbeddedState::Running => "running",
                    super::gstpop::embedded::EmbeddedState::Error => "error",
                };
                let externally = status.externally_owned;
                let _ = poll_weak.upgrade_in_event_loop(move |ui| {
                    let b = ui.global::<crate::Bridge>();
                    if ui.global::<crate::PanelBridge>().get_active() != crate::Panel::MediaBackend {
                        return;
                    }
                    b.set_gstpop_service_state(state_str.into());
                    b.set_gstpop_service_externally_owned(externally);
                });
            }
        });

        // ── Migration runtime: 1Hz status poller ──────────────────────────────────
        let poll_mig_weak = ui.as_weak();
        tokio::spawn(async move {
            let mut ticker =
                tokio::time::interval(std::time::Duration::from_millis(1000));
            loop {
                ticker.tick().await;
                let state_str: &'static str = match crate::migration_service::query_status() {
                    Ok(json) => {
                        if json.contains("\"running\"") {
                            "running"
                        } else if json.contains("\"error\"") {
                            "error"
                        } else {
                            "stopped"
                        }
                    }
                    Err(_) => "stopped",
                };
                let _ = poll_mig_weak.upgrade_in_event_loop(move |ui| {
                    let b = ui.global::<crate::Bridge>();
                    if ui.global::<crate::PanelBridge>().get_active() != crate::Panel::MediaBackend {
                        return;
                    }
                    b.set_migration_runtime_service_state(state_str.into());
                    // Keep the top-level status pill in sync when migration is the
                    // active backend — poller is the single source of truth.
                    if b.get_media_backend() == crate::MediaBackendKind::Migration {
                        let (mbs, text): (crate::MediaBackendState, &str) = match state_str {
                            "running"  => (crate::MediaBackendState::Ready,        "Migration runtime running"),
                            "starting" => (crate::MediaBackendState::Starting,     ""),
                            "error"    => (crate::MediaBackendState::Error,        ""),
                            _          => (crate::MediaBackendState::Disconnected, "Migration runtime stopped"),
                        };
                        b.set_media_backend_state(mbs);
                        b.set_media_backend_status_text(text.into());
                    }
                });
            }
        });

        Arc::clone(&self).autostart(ui.as_weak());
    }

    fn autostart(self: Arc<Self>, weak: Weak<MainWindow>) {
        use super::gstpop::{embedded, service};

        if self.initial_config.kind == BackendKind::GstPop
            && embedded::is_localhost(&self.initial_config.gstpop_url)
        {
            if let Err(err) = service::request_service_start(&self.initial_config) {
                tracing::error!(?err, "autostart: request_service_start failed");
            }
            push_state(&weak, crate::MediaBackendState::Starting);
        } else {
            push_state(&weak, crate::MediaBackendState::Probing);
        }

        tokio::spawn(async move {
            for attempt in 0..25 {
                match current().probe().await {
                    Ok(status) => {
                        push_status(&weak, status);
                        return;
                    }
                    Err(err) if attempt < 24 => {
                        tracing::debug!(?err, attempt, "autostart probe retry");
                        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    }
                    Err(err) => {
                        push_error(&weak, &err.to_string());
                        return;
                    }
                }
            }
        });
    }

    async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
        use super::gstpop::{embedded, service};

        let previous = current();

        // Persist first so a crash before service start still recovers cleanly.
        config.save(&self.files_dir)?;

        // Service-lifecycle side effects (no-ops on non-Android).
        match config.kind {
            BackendKind::GstPop if embedded::is_localhost(&config.gstpop_url) => {
                push_state(&weak, crate::MediaBackendState::Starting);
                if let Err(err) = service::request_service_start(&config) {
                    tracing::error!(?err, "request_service_start failed");
                    push_error(&weak, &format!("service start failed: {err}"));
                }
            }
            BackendKind::GstPop => {
                service::request_service_stop();
            }
            BackendKind::Migration => {
                service::request_service_stop();
            }
        }

        // Shut down the outgoing backend before installing the new one.
        if previous.kind() != config.kind {
            if let Err(err) = previous.shutdown().await {
                tracing::warn!(?err, "previous backend shutdown failed");
            }
        }

        install(build_backend(&config));
        push_state(&weak, crate::MediaBackendState::Probing);
        match current().probe().await {
            Ok(status) => push_status(&weak, status),
            Err(err) => push_error(&weak, &err.to_string()),
        }
        Ok(())
    }
}

fn build_backend(stored: &StoredBackendConfig) -> Arc<dyn MediaBackend> {
    match stored.kind {
        BackendKind::Migration => Arc::new(MigrationBackend::new()),
        BackendKind::GstPop => Arc::new(GstPopBackend::new(
            stored.gstpop_url.clone(),
            stored
                .gstpop_api_key
                .clone()
                .filter(|value| !value.is_empty()),
            stored.gstpop_pipeline_id.clone(),
        )),
    }
}

fn push_config(weak: &Weak<MainWindow>, config: &StoredBackendConfig) {
    let config = config.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        bridge.set_media_backend(into_slint(config.kind));
        bridge.set_gstpop_url(config.gstpop_url.into());
        bridge.set_gstpop_api_key(config.gstpop_api_key.unwrap_or_default().into());
        bridge.set_gstpop_pipeline_id(config.gstpop_pipeline_id.into());
        bridge.set_media_backend_state(crate::MediaBackendState::Disconnected);
        bridge.set_media_backend_status_text("".into());
        bridge.set_media_backend_error_text("".into());
    });
}

fn read_config_from_bridge(weak: &Weak<MainWindow>) -> StoredBackendConfig {
    let Some(ui) = weak.upgrade() else {
        return StoredBackendConfig::defaults();
    };
    let bridge = ui.global::<crate::Bridge>();
    let api_key = bridge.get_gstpop_api_key().to_string();
    StoredBackendConfig {
        kind: from_slint(bridge.get_media_backend()),
        gstpop_url: bridge.get_gstpop_url().to_string(),
        gstpop_api_key: (!api_key.is_empty()).then_some(api_key),
        gstpop_pipeline_id: bridge.get_gstpop_pipeline_id().to_string(),
    }
}

fn push_state(weak: &Weak<MainWindow>, state: crate::MediaBackendState) {
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.global::<crate::Bridge>().set_media_backend_state(state);
    });
}

fn push_saved(weak: &Weak<MainWindow>) {
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        bridge.set_media_backend_error_text("".into());
        bridge.set_media_backend_status_text("Saved backend settings".into());
    });
}

fn push_status(weak: &Weak<MainWindow>, status: BackendStatus) {
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        let state = if status.is_connected {
            crate::MediaBackendState::Ready
        } else {
            crate::MediaBackendState::Disconnected
        };
        bridge.set_media_backend_state(state);
        bridge.set_media_backend_status_text(status.status_text.into());
        bridge.set_media_backend_error_text(status.error_text.into());
    });
}

fn push_error(weak: &Weak<MainWindow>, message: &str) {
    let message = message.to_owned();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        let bridge = ui.global::<crate::Bridge>();
        bridge.set_media_backend_state(crate::MediaBackendState::Error);
        bridge.set_media_backend_status_text("".into());
        bridge.set_media_backend_error_text(message.into());
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn defaults_round_trip_through_save_load() {
        let dir = tempdir().unwrap();
        let original = StoredBackendConfig::defaults();
        original.save(dir.path()).unwrap();
        let restored = StoredBackendConfig::load(dir.path()).unwrap();

        assert_eq!(restored.kind, BackendKind::Migration);
        assert_eq!(restored.gstpop_url, "ws://127.0.0.1:9000");
    }

    #[test]
    fn load_falls_back_to_defaults_when_file_missing() {
        let dir = tempdir().unwrap();
        let loaded = StoredBackendConfig::load(dir.path()).unwrap();
        assert_eq!(loaded.kind, BackendKind::Migration);
    }

    #[test]
    fn test_switch_media_backend_to_gstpop_integration() {
        use std::sync::Mutex as StdMutex;
        use tokio::net::TcpListener;
        use tokio_tungstenite::accept_hdr_async;
        use tokio_tungstenite::tungstenite::Message;
        use futures_util::{SinkExt, StreamExt};
        use serde_json::{json, Value};

        // Create a multi-threaded tokio runtime for background tasks
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();

        // 1. Initialize Slint test backend with mock time
        i_slint_backend_testing::init_integration_test_with_mock_time();

        // 2. Start mock WebSocket server on an ephemeral port
        let listener = rt.block_on(async { TcpListener::bind("127.0.0.1:0").await }).unwrap();
        let port = listener.local_addr().unwrap().port();
        let mock_url = format!("ws://127.0.0.1:{port}");

        let auth_ok = Arc::new(StdMutex::new(false));
        let auth_ok_cb = Arc::clone(&auth_ok);

        rt.spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let callback = move |req: &tokio_tungstenite::tungstenite::handshake::server::Request, response: tokio_tungstenite::tungstenite::handshake::server::Response| {
                if let Some(auth) = req.headers().get("Authorization") {
                    if auth.to_str().unwrap() == "mock-key" {
                        *auth_ok_cb.lock().unwrap() = true;
                    }
                }
                Ok(response)
            };
            let mut ws = accept_hdr_async(stream, callback).await.unwrap();
            while let Some(msg) = ws.next().await {
                if let Ok(Message::Text(text)) = msg {
                    let req: Value = serde_json::from_str(&text).unwrap();
                    let id = req["id"].as_str().unwrap().to_owned();
                    let method = req["method"].as_str().unwrap();
                    let reply = match method {
                        "get_version" => json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "version": "v1.2.3" }
                        }),
                        "get_pipeline_count" => json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": { "count": 2 }
                        }),
                        _ => json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "error": { "code": -32601, "message": "Method not found" }
                        }),
                    };
                    let _ = ws.send(Message::Text(reply.to_string().into())).await;
                }
            }
        });

        // 3. Create BackendLifecycle with temp files dir
        let dir = tempdir().unwrap();
        let lifecycle = Arc::new(BackendLifecycle::new(dir.path().to_path_buf()));

        // 4. Instantiate UI window
        let ui = crate::MainWindow::new().unwrap();

        // 5. Register the lifecycle
        lifecycle.register(&ui);

        let ui_weak = ui.as_weak();
        let auth_ok_check = Arc::clone(&auth_ok);

        slint::spawn_local(async move {
            let ui = ui_weak.upgrade().unwrap();
            let bridge = ui.global::<crate::Bridge>();

            // 6. Process initial autostart logic (Migration backend)
            for _ in 0..20 {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }

            // 7. Verify the bridge is initially connected/ready as Migration
            assert_eq!(bridge.get_media_backend(), crate::MediaBackendKind::Migration);

            // 8. Simulate UI page switching to gst-pop and entering configuration
            bridge.set_media_backend(crate::MediaBackendKind::GstPop);
            bridge.set_gstpop_url(mock_url.into());
            bridge.set_gstpop_api_key("mock-key".into());
            bridge.set_gstpop_pipeline_id("0".into());

            // 9. Invoke apply callback (simulate clicking the Apply button in media_backend_page.slint)
            bridge.invoke_apply_media_backend();

            // 10. Spin the event loop to let GstPop connection process
            let mut success = false;
            for _ in 0..100 {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;

                if bridge.get_media_backend_state() == crate::MediaBackendState::Ready {
                    success = true;
                    break;
                }
            }

            assert!(success, "Backend state failed to transition to Ready");
            assert_eq!(
                bridge.get_media_backend_status_text().as_str(),
                "gst-pop v1.2.3 - 2 pipeline(s)"
            );
            assert!(*auth_ok_check.lock().unwrap(), "Authorization header was missing or incorrect");

            // 11. Clean up
            let _ = current().shutdown().await;

            slint::quit_event_loop().unwrap();
        }).unwrap();

        slint::run_event_loop().unwrap();
    }
}
