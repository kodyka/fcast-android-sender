use fcastsender::overlay::{OverlayConfig, OverlayManager, OverlayRect, OverlaySource};
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
    assert_eq!(slot1[0].id, "o1"); // z_order=5 comes first
    assert_eq!(slot1[1].id, "o2"); // z_order=10 comes second

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
