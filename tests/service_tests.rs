use fcastsender::service::{ServiceMode, ServiceOptions};
use fcastsender::service::mock::MockServiceManager;
use fcastsender::service::ServiceManager;

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
async fn disabled_service_options_accessible() {
    let svc = MockServiceManager::new("test-service");
    svc.set_options(ServiceOptions {
        enabled: false,
        auto_start: false,
        mode: ServiceMode::Embedded,
    });

    let opts = svc.options();
    assert!(!opts.enabled);
    assert!(!opts.auto_start);
}

#[tokio::test]
async fn start_failure_returns_error() {
    let svc = MockServiceManager::new("fail-service");
    svc.start_should_fail
        .store(true, std::sync::atomic::Ordering::Relaxed);

    let result = svc.start().await;
    assert!(result.is_err());
}
