use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::BackendKind;
use crate::service::{ServiceMode, ServiceOptions};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoredBackendConfig {
    pub kind: BackendKind,
    pub gstpop_url: String,
    pub gstpop_api_key: Option<String>,
    pub gstpop_pipeline_id: String,

    #[serde(default)]
    pub gstpop_service: Option<ServiceOptions>,
    #[serde(default)]
    pub migration_service: Option<ServiceOptions>,
    #[serde(default)]
    pub auto_start_services: bool,
    #[serde(default)]
    pub service_mode: ServiceMode,
}

impl StoredBackendConfig {
    pub fn defaults() -> Self {
        Self {
            kind: BackendKind::Migration,
            gstpop_url: "ws://127.0.0.1:9000".into(),
            gstpop_api_key: None,
            gstpop_pipeline_id: "0".into(),
            gstpop_service: Some(ServiceOptions {
                enabled: true,
                auto_start: true,
                mode: ServiceMode::AndroidService,
            }),
            migration_service: Some(ServiceOptions {
                enabled: true,
                auto_start: true,
                mode: ServiceMode::Embedded,
            }),
            auto_start_services: true,
            service_mode: ServiceMode::Embedded,
        }
    }

    pub fn load(files_dir: &Path) -> Result<Self> {
        let path = files_dir.join("backend.json");
        if !path.exists() {
            return Ok(Self::defaults());
        }
        let bytes = fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }

    pub fn save(&self, files_dir: &Path) -> Result<()> {
        let path = files_dir.join("backend.json");
        let json = serde_json::to_string_pretty(self).context("serialize backend.json")?;
        fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
        Ok(())
    }

    /// Resolved gst-pop service options (with defaults).
    pub fn gstpop_opts(&self) -> ServiceOptions {
        self.gstpop_service.clone().unwrap_or(ServiceOptions {
            enabled: true,
            auto_start: true,
            mode: ServiceMode::AndroidService,
        })
    }

    /// Resolved migration service options (with defaults).
    pub fn migration_opts(&self) -> ServiceOptions {
        self.migration_service.clone().unwrap_or_default()
    }
}
