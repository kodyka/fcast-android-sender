//! Service lifecycle abstraction layer.
//!
//! Provides [`ServiceManager`] — a uniform interface to start, stop,
//! and health-check any managed background service (gst-pop, migration
//! runtime, future services).

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// How the service process is hosted.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceMode {
    /// Run the engine inside the app process (current Migration default).
    #[default]
    Embedded,
    /// Managed Android foreground Service (current GstPop default on Android).
    AndroidService,
    /// Connect to a user-supplied external daemon (CI / dev machine).
    External,
}

/// Per-service toggles persisted alongside `StoredBackendConfig`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ServiceOptions {
    pub enabled: bool,
    pub auto_start: bool,
    pub mode: ServiceMode,
}

impl Default for ServiceOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_start: true,
            mode: ServiceMode::Embedded,
        }
    }
}

/// Lifecycle status reported by a managed service.
#[derive(Clone, Debug, Default)]
pub struct ServiceStatus {
    pub running: bool,
    pub healthy: bool,
    pub status_text: String,
    pub error_text: String,
}

/// Uniform abstraction over any managed background service.
#[async_trait::async_trait]
pub trait ServiceManager: Send + Sync {
    /// Human-readable service name (e.g. "gst-pop", "migration").
    fn name(&self) -> &str;

    /// Current options (enabled, auto-start, mode).
    fn options(&self) -> ServiceOptions;

    /// Mutate options at runtime.
    fn set_options(&mut self, options: ServiceOptions);

    /// Bring the service up. Idempotent — calling twice is safe.
    async fn start(&self) -> Result<ServiceStatus>;

    /// Tear the service down. Idempotent.
    async fn stop(&self) -> Result<ServiceStatus>;

    /// Cheap health-check (no heavy I/O).
    async fn status(&self) -> Result<ServiceStatus>;
}

pub mod mock;
