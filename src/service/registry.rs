//! Global service manager registry.
//!
//! Holds long-lived `Arc` handles to the concrete service managers so
//! both `BackendLifecycle` (autostart/apply paths) and the UI callback
//! wiring (`ServiceBridge`, `ServiceConfigBridge`) operate on the same
//! instances.

use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::backend::gstpop::service::GstPopServiceManager;
use crate::backend::persistence::StoredBackendConfig;
use crate::migration::service::MigrationServiceManager;
use crate::overlay::OverlayManager;
use crate::service::ServiceManager;
use crate::srt::SrtSourceManager;

/// Concrete manager handles shared across the app.
pub struct ServiceManagers {
    pub gstpop: Arc<GstPopServiceManager>,
    pub migration: Arc<MigrationServiceManager>,
    pub srt: Arc<SrtSourceManager>,
    pub overlay: Arc<OverlayManager>,
}

static MANAGERS: OnceCell<ServiceManagers> = OnceCell::new();

/// Initialise the global registry from a `StoredBackendConfig`. Idempotent:
/// subsequent calls return the originally-initialised handles.
pub fn init(config: &StoredBackendConfig) -> &'static ServiceManagers {
    MANAGERS.get_or_init(|| ServiceManagers {
        gstpop: Arc::new(GstPopServiceManager::new(config.gstpop_opts())),
        migration: Arc::new(MigrationServiceManager::new(config.migration_opts())),
        srt: Arc::new(SrtSourceManager::new()),
        overlay: Arc::new(OverlayManager::new()),
    })
}

/// Borrow the initialised registry, if any.
pub fn get() -> Option<&'static ServiceManagers> {
    MANAGERS.get()
}

/// Look up a manager by id (`"gstpop"` or `"migration"`) as a trait object
/// suitable for generic `start/stop/status` dispatch.
pub fn lookup(id: &str) -> Option<Arc<dyn ServiceManager>> {
    let mgrs = get()?;
    match id {
        "gstpop" => Some(Arc::clone(&mgrs.gstpop) as Arc<dyn ServiceManager>),
        "migration" => Some(Arc::clone(&mgrs.migration) as Arc<dyn ServiceManager>),
        _ => None,
    }
}
