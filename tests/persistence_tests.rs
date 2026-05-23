use fcastsender::backend::persistence::StoredBackendConfig;

#[test]
fn config_round_trip_with_new_fields() {
    let config = StoredBackendConfig::defaults();
    let json = serde_json::to_string_pretty(&config).unwrap();
    let loaded: StoredBackendConfig = serde_json::from_str(&json).unwrap();

    assert_eq!(format!("{:?}", loaded.kind), format!("{:?}", config.kind));
    assert_eq!(loaded.gstpop_url, config.gstpop_url);
    assert!(loaded.gstpop_service.is_some());
    assert!(loaded.migration_service.is_some());
}

#[test]
fn old_config_without_service_fields_loads() {
    let old_json = r#"{
        "kind": "migration",
        "gstpop_url": "ws://127.0.0.1:9000",
        "gstpop_api_key": null,
        "gstpop_pipeline_id": "0"
    }"#;
    let config: StoredBackendConfig = serde_json::from_str(old_json).unwrap();
    assert!(config.gstpop_service.is_none());
    assert!(config.migration_service.is_none());
    assert!(!config.auto_start_services);
}
