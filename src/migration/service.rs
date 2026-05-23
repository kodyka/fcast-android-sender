//! Migration runtime service wrapper implementing the `ServiceManager` trait.

use anyhow::Result;

use crate::migration::runtime;
use crate::service::{ServiceManager, ServiceOptions, ServiceStatus};

pub struct MigrationServiceManager {
    options: parking_lot::RwLock<ServiceOptions>,
}

impl MigrationServiceManager {
    pub fn new(options: ServiceOptions) -> Self {
        Self {
            options: parking_lot::RwLock::new(options),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for MigrationServiceManager {
    fn name(&self) -> &str {
        "migration"
    }

    fn options(&self) -> ServiceOptions {
        self.options.read().clone()
    }

    fn set_options(&mut self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> Result<ServiceStatus> {
        let opts = self.options.read().clone();
        if !opts.enabled {
            return Ok(ServiceStatus {
                running: false,
                healthy: true,
                status_text: "migration runtime disabled".into(),
                error_text: String::new(),
            });
        }

        tokio::task::spawn_blocking(runtime::start_graph_runtime)
            .await
            .map_err(|e| anyhow::anyhow!("join error: {e}"))??;

        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: "migration runtime started".into(),
            error_text: String::new(),
        })
    }

    async fn stop(&self) -> Result<ServiceStatus> {
        tokio::task::spawn_blocking(runtime::shutdown_graph_runtime)
            .await
            .map_err(|e| anyhow::anyhow!("join error: {e}"))??;

        Ok(ServiceStatus {
            running: false,
            healthy: true,
            status_text: "migration runtime stopped".into(),
            error_text: String::new(),
        })
    }

    async fn status(&self) -> Result<ServiceStatus> {
        let payload = r#"{"getinfo":{}}"#;
        let response = runtime::try_handle_command_json(payload);
        let healthy = response.contains("\"result\"");

        Ok(ServiceStatus {
            running: healthy,
            healthy,
            status_text: if healthy {
                "migration runtime responsive".into()
            } else {
                "migration runtime not responding".into()
            },
            error_text: String::new(),
        })
    }
}
