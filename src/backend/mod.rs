pub mod gstpop;
mod kind;
pub mod lifecycle;
mod migration_backend;
pub mod persistence;

pub use kind::BackendKind;
pub use migration_backend::MigrationBackend;

use anyhow::Result;
use parking_lot::RwLock;
use serde_json::Value;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct BackendStatus {
    pub status_text: String,
    pub error_text: String,
    /// True when the backend is reachable and operational (show Ready).
    /// False when it is present but not yet running (show Disconnected).
    pub is_connected: bool,
}

impl Default for BackendStatus {
    fn default() -> Self {
        Self { status_text: String::new(), error_text: String::new(), is_connected: true }
    }
}

#[async_trait::async_trait]
pub trait MediaBackend: Send + Sync {
    fn kind(&self) -> BackendKind;
    async fn probe(&self) -> Result<BackendStatus>;
    async fn dispatch(&self, action: &str, params: Value) -> Result<Value>;
    async fn list(&self) -> Result<Value>;
    async fn shutdown(&self) -> Result<()>;
}

static BACKEND: once_cell::sync::Lazy<RwLock<Arc<dyn MediaBackend>>> =
    once_cell::sync::Lazy::new(|| RwLock::new(Arc::new(MigrationBackend::new())));

pub fn current() -> Arc<dyn MediaBackend> {
    BACKEND.read().clone()
}

pub fn install(new_backend: Arc<dyn MediaBackend>) {
    *BACKEND.write() = new_backend;
}

pub fn from_slint(kind: crate::MediaBackendKind) -> BackendKind {
    match kind {
        crate::MediaBackendKind::Migration => BackendKind::Migration,
        crate::MediaBackendKind::GstPop => BackendKind::GstPop,
    }
}

pub fn into_slint(kind: BackendKind) -> crate::MediaBackendKind {
    match kind {
        BackendKind::Migration => crate::MediaBackendKind::Migration,
        BackendKind::GstPop => crate::MediaBackendKind::GstPop,
    }
}
