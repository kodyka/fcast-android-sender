//! The process-wide App context. One instance per process; constructed by
//! the JNI bootstrap entry and accessed via [`app`].
//!
//! Replaces the process-global `BACKEND: Lazy<RwLock<…>>`; see
//! docs/refactor-implementation-guide/05-composition-root-and-interfaces/.

use std::sync::Arc;

use once_cell::sync::OnceCell;

use crate::backend::registry::{BackendKind, BackendRegistry, InMemoryRegistry};
use crate::backend::{MediaBackend, MigrationBackend};
use crate::secret::{InMemorySecretStore, SecretStore};

/// The composition root for the Rust crate. Constructed once during
/// android_main / JNI bootstrap.
pub struct App {
    registry: Box<dyn BackendRegistry>,
    secrets: Box<dyn SecretStore>,
}

impl App {
    #[cfg(target_os = "android")]
    pub fn production(vm: jni::JavaVM) -> Self {
        let registry = InMemoryRegistry::new();
        registry.install(BackendKind::Migration, Arc::new(MigrationBackend::new()));
        Self {
            registry: Box::new(registry),
            secrets: Box::new(crate::secret::jni::JniSecretStore::new(vm)),
        }
    }

    #[cfg(not(target_os = "android"))]
    pub fn production() -> Self {
        let registry = InMemoryRegistry::new();
        registry.install(BackendKind::Migration, Arc::new(MigrationBackend::new()));
        Self {
            registry: Box::new(registry),
            secrets: Box::new(InMemorySecretStore::new()),
        }
    }

    pub fn with_secrets(mut self, secrets: Box<dyn SecretStore>) -> Self {
        self.secrets = secrets;
        self
    }

    pub fn registry(&self) -> &dyn BackendRegistry { self.registry.as_ref() }
    pub fn secrets(&self) -> &dyn SecretStore { self.secrets.as_ref() }
}

/// Process-global accessor — returns &'static App after bootstrap.
static APP: OnceCell<App> = OnceCell::new();

/// Bootstrap entry called from android_main (or test harness).
pub fn init(app: App) {
    if APP.set(app).is_err() {
        // Re-init is a bug — but don't crash the process; the existing
        // instance wins.
        tracing::warn!("App::init called twice; ignoring");
    }
}

/// Access the App. Panics if init() was not called first.
pub fn app() -> &'static App {
    APP.get().expect("App not initialised; call init() during JNI bootstrap")
}

/// Access the App safely without panicking.
pub fn try_app() -> Option<&'static App> {
    APP.get()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_has_migration_backend() {
        let a = App::production();
        assert!(a.registry().get(BackendKind::Migration).is_some());
        assert!(a.registry().get(BackendKind::Gstpop).is_none());
    }
}
