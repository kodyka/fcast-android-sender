use fcastsender::srt::{SrtConnectionState, SrtSourceConfig, SrtSourceManager};

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

#[test]
fn upsert_updates_existing_slot() {
    let manager = SrtSourceManager::new();
    manager.upsert_slot(SrtSourceConfig {
        slot_id: "slot-1".into(),
        uri: "srt://old.example:9000".into(),
        ..Default::default()
    });
    manager.upsert_slot(SrtSourceConfig {
        slot_id: "slot-1".into(),
        uri: "srt://new.example:9000".into(),
        ..Default::default()
    });
    let snap = manager.snapshot();
    assert_eq!(snap.len(), 1);
    assert_eq!(snap["slot-1"].config.uri, "srt://new.example:9000");
}
