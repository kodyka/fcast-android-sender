//! Backend registry — owns the chosen MediaBackend for each kind.
//!
//! Replaces the previous process-global BACKEND: Lazy<RwLock<…>>; see
//! docs/refactor-implementation-guide/05-composition-root-and-interfaces/.

use std::sync::Arc;

use crate::backend::MediaBackend;
use parking_lot::RwLock;

/// Stable identifier for a backend implementation. Mirrors the Slint
/// `MediaBackendKind` enum.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum BackendKind {
    Gstpop,
    Migration,
}

pub trait BackendRegistry: Send + Sync {
    fn install(&self, kind: BackendKind, backend: Arc<dyn MediaBackend>);
    fn get(&self, kind: BackendKind) -> Option<Arc<dyn MediaBackend>>;

    /// Convenience: get-or-error-with-message.
    fn require(&self, kind: BackendKind) -> Result<Arc<dyn MediaBackend>, String> {
        self.get(kind)
            .ok_or_else(|| format!("no backend installed for kind={kind:?}"))
    }
}

/// Simple in-memory implementation backed by parking_lot::RwLock.
pub struct InMemoryRegistry {
    gstpop:    RwLock<Option<Arc<dyn MediaBackend>>>,
    migration: RwLock<Option<Arc<dyn MediaBackend>>>,
}

impl InMemoryRegistry {
    pub fn new() -> Self {
        Self {
            gstpop:    RwLock::new(None),
            migration: RwLock::new(None),
        }
    }
}

impl Default for InMemoryRegistry {
    fn default() -> Self { Self::new() }
}

impl BackendRegistry for InMemoryRegistry {
    fn install(&self, kind: BackendKind, backend: Arc<dyn MediaBackend>) {
        match kind {
            BackendKind::Gstpop    => *self.gstpop.write()    = Some(backend),
            BackendKind::Migration => *self.migration.write() = Some(backend),
        }
    }

    fn get(&self, kind: BackendKind) -> Option<Arc<dyn MediaBackend>> {
        match kind {
            BackendKind::Gstpop    => self.gstpop.read().clone(),
            BackendKind::Migration => self.migration.read().clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::MigrationBackend;

    #[test]
    fn install_and_get() {
        let r = InMemoryRegistry::new();
        assert!(r.get(BackendKind::Migration).is_none());
        r.install(BackendKind::Migration, Arc::new(MigrationBackend::new()));
        assert!(r.get(BackendKind::Migration).is_some());
    }

    #[test]
    fn unset_kinds_remain_none() {
        let r = InMemoryRegistry::new();
        r.install(BackendKind::Migration, Arc::new(MigrationBackend::new()));
        assert!(r.get(BackendKind::Gstpop).is_none());
    }
}
