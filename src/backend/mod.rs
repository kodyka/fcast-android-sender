mod gstpop_backend;
mod kind;
pub mod lifecycle;
mod migration_backend;
pub mod persistence;

pub use gstpop_backend::GstPopBackend;
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

pub mod registry;

#[deprecated(note = "Use crate::app::app().registry() — see refactor step 05.")]
#[doc(hidden)]
static BACKEND_LEGACY_GUARD: () = ();

/// Deprecated. Returns the *migration* backend from the new registry. Existing
/// callers expected the migration backend by default — that contract is
/// preserved during the soak window.
#[deprecated(note = "Use crate::app::app().registry().require(BackendKind::…) — see refactor step 05.")]
pub fn current() -> std::sync::Arc<dyn MediaBackend> {
    crate::app::app()
        .registry()
        .require(crate::backend::registry::BackendKind::Migration)
        .expect("legacy current() called before any backend installed")
}

#[deprecated(note = "Use crate::app::app().registry().install(BackendKind::…, backend) — see refactor step 05.")]
pub fn install(new_backend: std::sync::Arc<dyn MediaBackend>) {
    crate::app::app()
        .registry()
        .install(crate::backend::registry::BackendKind::Migration, new_backend);
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
