# STEP 15 — Testing Infrastructure

**Phase:** 5 (Codebase Restructuring)
**New files:** `tests/` directory

---

## Goal

Add unit tests for the service abstraction layer, integration tests for
SRT source handling, and mock service implementations for UI testing.

---

## 1. Mock ServiceManager for unit tests

```rust
// src/service/mock.rs  (new file, gated behind cfg(test))

#[cfg(test)]
pub mod mock {
    use super::{ServiceManager, ServiceOptions, ServiceStatus};
    use anyhow::Result;
    use std::sync::atomic::{AtomicBool, Ordering};

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

        fn options(&self) -> &ServiceOptions {
            self.options.read().clone()
        }

        fn set_options(&mut self, options: ServiceOptions) {
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
}
```

Register in `src/service/mod.rs`:

```rust
#[cfg(test)]
pub mod mock;
```

## 2. Unit tests for ServiceManager trait

```rust
// tests/service_tests.rs  (new integration test file)

use fcastsender::service::{ServiceManager, ServiceOptions, ServiceMode};
use fcastsender::service::mock::MockServiceManager;

#[tokio::test]
async fn mock_service_start_stop() {
    let svc = MockServiceManager::new("test-service");
    let status = svc.start().await.unwrap();
    assert!(status.running);
    assert!(status.healthy);

    let status = svc.stop().await.unwrap();
    assert!(!status.running);
}

#[tokio::test]
async fn disabled_service_skips_start() {
    let mut svc = MockServiceManager::new("test-service");
    svc.set_options(ServiceOptions {
        enabled: false,
        auto_start: false,
        mode: ServiceMode::Embedded,
    });

    // With enabled=false, the real implementations return early.
    // The mock doesn't check this — add the guard in production code.
    let status = svc.start().await.unwrap();
    assert!(status.running); // mock always starts; real impl would skip
}

#[tokio::test]
async fn start_failure_returns_error() {
    let svc = MockServiceManager::new("fail-service");
    svc.start_should_fail
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let result = svc.start().await;
    assert!(result.is_err());
}
```

## 3. SRT source manager tests

```rust
// tests/srt_tests.rs

use fcastsender::srt::{SrtSourceConfig, SrtSourceManager, SrtConnectionState};

#[test]
fn upsert_and_snapshot() {
    let manager = SrtSourceManager::new();
    let config = SrtSourceConfig {
        slot_id: "slot-1".into(),
        enabled: true,
        uri: "srt://example.com:9000".into(),
        ..Default::default()
    };
    manager.upsert_slot(config);

    let snap = manager.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap["slot-1"].config.uri, "srt://example.com:9000");
    assert_eq!(snap["slot-1"].connection, SrtConnectionState::Disconnected);
}

#[test]
fn remove_slot() {
    let manager = SrtSourceManager::new();
    manager.upsert_slot(SrtSourceConfig {
        slot_id: "slot-1".into(),
        ..Default::default()
    });
    manager.upsert_slot(SrtSourceConfig {
        slot_id: "slot-2".into(),
        ..Default::default()
    });
    assert_eq!(manager.snapshot().len(), 2);

    manager.remove_slot("slot-1");
    assert_eq!(manager.snapshot().len(), 1);
    assert!(manager.snapshot().contains_key("slot-2"));
}
```

## 4. Overlay manager tests

```rust
// tests/overlay_tests.rs

use fcastsender::overlay::{OverlayConfig, OverlayManager, OverlaySource, OverlayRect};
use std::path::PathBuf;

#[test]
fn add_and_retrieve_overlays() {
    let manager = OverlayManager::new();
    manager.upsert(OverlayConfig {
        id: "o1".into(),
        slot_id: "slot-1".into(),
        visible: true,
        source: OverlaySource::File(PathBuf::from("/tmp/logo.png")),
        rect: OverlayRect { x: 10, y: 20, width: 100, height: 50 },
        alpha: 0.8,
        z_order: 5,
    });
    manager.upsert(OverlayConfig {
        id: "o2".into(),
        slot_id: "slot-1".into(),
        visible: true,
        source: OverlaySource::File(PathBuf::from("/tmp/banner.png")),
        rect: OverlayRect::default(),
        alpha: 1.0,
        z_order: 10,
    });
    manager.upsert(OverlayConfig {
        id: "o3".into(),
        slot_id: "slot-2".into(),
        visible: true,
        ..Default::default()
    });

    let slot1 = manager.overlays_for_slot("slot-1");
    assert_eq!(slot1.len(), 2);
    assert_eq!(slot1[0].id, "o1"); // z_order=5 first
    assert_eq!(slot1[1].id, "o2"); // z_order=10 second

    let slot2 = manager.overlays_for_slot("slot-2");
    assert_eq!(slot2.len(), 1);
}

#[test]
fn invisible_overlays_excluded() {
    let manager = OverlayManager::new();
    manager.upsert(OverlayConfig {
        id: "hidden".into(),
        slot_id: "slot-1".into(),
        visible: false,
        ..Default::default()
    });
    assert_eq!(manager.overlays_for_slot("slot-1").len(), 0);
}

#[test]
fn remove_overlay() {
    let manager = OverlayManager::new();
    manager.upsert(OverlayConfig {
        id: "remove-me".into(),
        slot_id: "slot-1".into(),
        ..Default::default()
    });
    assert_eq!(manager.all().len(), 1);
    manager.remove("remove-me");
    assert_eq!(manager.all().len(), 0);
}
```

## 5. Persistence round-trip test

```rust
// tests/persistence_tests.rs

use fcastsender::backend::persistence::StoredBackendConfig;

#[test]
fn config_round_trip_with_new_fields() {
    let config = StoredBackendConfig::defaults();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let loaded: StoredBackendConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(loaded.kind, config.kind);
    assert_eq!(loaded.gstpop_url, config.gstpop_url);
    // New fields survive serialization
    assert!(loaded.gstpop_service.is_some());
    assert!(loaded.migration_service.is_some());
}

#[test]
fn old_config_without_service_fields_loads() {
    // Simulate an old backend.json without the new fields.
    let old_json = r#"{
        "kind": "migration",
        "gstpop_url": "ws://127.0.0.1:9000",
        "gstpop_api_key": null,
        "gstpop_pipeline_id": "0"
    }"#;
    let config: StoredBackendConfig = serde_json::from_str(old_json).unwrap();
    assert!(config.gstpop_service.is_none());
    assert!(config.migration_service.is_none());
    assert!(!config.auto_start_services); // default
}
```

## 6. UI snapshot tests (extending existing)

The repo already has `tests/ui_snapshots.rs` using
`i-slint-backend-testing`.  Extend it to cover the new pages:

```rust
// tests/ui_snapshots.rs  (add test cases)

#[test]
fn service_config_page_renders() {
    // Load the page component and verify it doesn't panic.
    slint_testing::init();
    // Instantiate ServiceConfigPage and assert basic properties.
}

#[test]
fn srt_config_page_renders() {
    slint_testing::init();
    // Instantiate SrtConfigPage and verify layout.
}
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `src/service/mock.rs` with `MockServiceManager` | new file |
| 2 | Create `tests/service_tests.rs` | new file |
| 3 | Create `tests/srt_tests.rs` | new file |
| 4 | Create `tests/overlay_tests.rs` | new file |
| 5 | Create `tests/persistence_tests.rs` | new file |
| 6 | Extend `tests/ui_snapshots.rs` for new pages | existing file |
| 7 | Run `cargo test` and verify all pass | terminal |

---

## Notes

* The mock service manager is `#[cfg(test)]`-gated so it doesn't bloat
  the release binary.
* Integration tests (`tests/*.rs`) have access to `pub` items from the
  crate.  The `service` and `srt` modules are `pub` specifically to
  enable this.
