# Example: Adding a New Service Backend

This example shows how to add a hypothetical "WebRTC Service" backend.

## 1. Implement ServiceManager

```rust
// src/webrtc/service.rs

use crate::service::{ServiceManager, ServiceOptions, ServiceStatus};

pub struct WebRtcServiceManager {
    options: parking_lot::RwLock<ServiceOptions>,
}

impl WebRtcServiceManager {
    pub fn new(options: ServiceOptions) -> Self {
        Self {
            options: parking_lot::RwLock::new(options),
        }
    }
}

#[async_trait::async_trait]
impl ServiceManager for WebRtcServiceManager {
    fn name(&self) -> &str { "webrtc" }

    fn options(&self) -> ServiceOptions {
        self.options.read().clone()
    }

    fn set_options(&mut self, options: ServiceOptions) {
        *self.options.write() = options;
    }

    async fn start(&self) -> anyhow::Result<ServiceStatus> {
        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: "WebRTC service running".into(),
            error_text: String::new(),
        })
    }

    async fn stop(&self) -> anyhow::Result<ServiceStatus> {
        Ok(ServiceStatus {
            running: false,
            healthy: true,
            status_text: "WebRTC service stopped".into(),
            error_text: String::new(),
        })
    }

    async fn status(&self) -> anyhow::Result<ServiceStatus> {
        Ok(ServiceStatus {
            running: true,
            healthy: true,
            status_text: "ok".into(),
            error_text: String::new(),
        })
    }
}
```

## 2. Register in the service registry

```rust
// In your app init code:
service_registry.insert("webrtc", Box::new(WebRtcServiceManager::new(opts)));
```

## 3. Add to BackendKind (if it's a new media backend)

```rust
// src/backend/kind.rs
pub enum BackendKind {
    Migration,
    GstPop,
    WebRtc,  // new
}
```

## 4. Add UI toggle

The ServiceBridge automatically shows any registered service in the Service Configuration page — no Slint changes needed.
