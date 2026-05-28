use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::BackendKind;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StoredBackendConfig {
    pub kind: BackendKind,
    pub gstpop_url: String,
    #[serde(default)]
    pub gstpop_api_key_alias: Option<String>,
    pub gstpop_pipeline_id: String,
}

impl StoredBackendConfig {
    pub fn defaults() -> Self {
        Self {
            kind: BackendKind::Migration,
            gstpop_url: "ws://127.0.0.1:9000".into(),
            gstpop_api_key_alias: None,
            gstpop_pipeline_id: "0".into(),
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
}
