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
            let lifecycle = Arc::clone(&apply_lifecycle);
            let weak = apply_weak.clone();
            tokio::spawn(async move {
                let config = read_config_from_bridge(&weak);
                if let Err(err) = lifecycle.apply(config, weak.clone()).await {
                    push_error(&weak, &err.to_string());
                }
            });
        });

        let save_lifecycle = Arc::clone(&self);
        let save_weak = ui.as_weak();
        bridge.on_save_media_backend_settings(move || {
            let lifecycle = Arc::clone(&save_lifecycle);
            let weak = save_weak.clone();
            tokio::spawn(async move {
                let config = read_config_from_bridge(&weak);
                match config.save(&lifecycle.files_dir) {
                    Ok(()) => push_saved(&weak),
                    Err(err) => push_error(&weak, &format!("save failed: {err}")),
                }
            });
        });

        let probe_weak = ui.as_weak();
        bridge.on_probe_media_backend(move || {
            let weak = probe_weak.clone();
            tokio::spawn(async move {
                let config = read_config_from_bridge(&weak);
                push_state(&weak, crate::MediaBackendState::Probing);
                let backend = build_backend(&config);
                match backend.probe().await {
                    Ok(status) => push_status(&weak, status),
                    Err(err) => push_error(&weak, &err.to_string()),
                }
            });
        });
    }

    async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
        config.save(&self.files_dir)?;
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
        bridge.set_media_backend_state(crate::MediaBackendState::Ready);
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
}
