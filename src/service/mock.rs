use std::sync::atomic::{AtomicBool, Ordering};

use super::{ServiceManager, ServiceOptions, ServiceStatus};
use anyhow::Result;

/// A mock ServiceManager that tracks start/stop calls.
pub struct MockServiceManager {
    pub name: String,
    pub options: parking_lot::RwLock<ServiceOptions>,
    pub started: AtomicBool,
    pub start_should_fail: AtomicBool,
}

impl MockServiceManager {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            options: parking_lot::RwLock::new(ServiceOptions::default()),
            started: AtomicBool::new(false),
            start_should_fail: AtomicBool::new(false),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for MockServiceManager {
    fn name(&self) -> &str {
        &self.name
    }

    fn options(&self) -> ServiceOptions {
        self.options.read().clone()
    }

    fn set_options(&self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> Result<ServiceStatus> {
        if self.start_should_fail.load(Ordering::Relaxed) {
            anyhow::bail!("mock start failure");
        }
        self.started.store(true, Ordering::Relaxed);
        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: format!("{} mock started", self.name),
            error_text: String::new(),
        })
    }

    async fn stop(&self) -> Result<ServiceStatus> {
        self.started.store(false, Ordering::Relaxed);
        Ok(ServiceStatus {
            running: false,
            healthy: true,
            status_text: format!("{} mock stopped", self.name),
            error_text: String::new(),
        })
    }

    async fn status(&self) -> Result<ServiceStatus> {
        Ok(ServiceStatus {
            running: self.started.load(Ordering::Relaxed),
            healthy: true,
            status_text: "mock status".into(),
            error_text: String::new(),
        })
    }
}
