//! One-shot migration of legacy `gstpop_api_key` plaintext config blobs into
//! the SecretStore. Idempotent: a config that has already been migrated is
//! left alone.

use crate::backend::persistence::StoredBackendConfig;
use std::fs;
use std::path::Path;

const DEFAULT_ALIAS: &str = "gstpop.api_key.v1";

pub fn migrate_config_file(files_dir: &Path) -> Result<(), String> {
    let path = files_dir.join("backend.json");
    if !path.exists() {
        return Ok(());
    }

    let raw = match fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => return Err(format!("read_to_string: {e}")),
    };

    let mut config: StoredBackendConfig = match serde_json::from_str(&raw) {
        Ok(c) => c,
        Err(e) => return Err(format!("parse: {e}")),
    };

    let legacy_key = config.gstpop_api_key.clone().unwrap_or_default();
    if legacy_key.is_empty() {
        // Already migrated or no legacy key.
        return Ok(());
    }

    // Store key bytes in SecretStore
    if let Some(app) = crate::app::try_app() {
        app.secrets()
            .put(DEFAULT_ALIAS, legacy_key.as_bytes())
            .map_err(|e| format!("put: {e}"))?;
    } else {
        return Err("App context not initialized".to_owned());
    }

    // Update config to use the alias and clear plaintext
    config.gstpop_api_key_alias = Some(DEFAULT_ALIAS.to_owned());
    config.gstpop_api_key = None;

    // Rewrite atomically
    let s = serde_json::to_string_pretty(&config).map_err(|e| format!("serialize: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, s).map_err(|e| format!("write tmp: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("rename: {e}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app;
    use crate::secret::InMemorySecretStore;

    fn fresh_app() {
        let app = app::App::production().with_secrets(Box::new(InMemorySecretStore::new()));
        let _ = app::init(app);
    }

    #[test]
    fn extracts_plaintext() {
        fresh_app();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("backend.json");
        std::fs::write(
            &path,
            r#"{"kind":"gst-pop","gstpop_url":"ws://127.0.0.1:9000","gstpop_api_key":"secret-123","gstpop_pipeline_id":"0"}"#,
        ).unwrap();
        migrate_config_file(tmp.path()).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("secret-123"));
        assert!(raw.contains("gstpop.api_key.v1"));
        let bytes = crate::app::app().secrets().get(DEFAULT_ALIAS).unwrap();
        assert_eq!(bytes.as_str().unwrap(), "secret-123");
    }

    #[test]
    fn idempotent() {
        fresh_app();
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("backend.json");
        std::fs::write(
            &path,
            r#"{"kind":"gst-pop","gstpop_url":"ws://127.0.0.1:9000","gstpop_api_key_alias":"gstpop.api_key.v1","gstpop_pipeline_id":"0"}"#,
        ).unwrap();
        migrate_config_file(tmp.path()).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(raw.contains("gstpop.api_key.v1"));
    }
}
