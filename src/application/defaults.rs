//! Default UI presets and quick actions.

pub(crate) fn default_presets() -> Vec<crate::BitratePreset> {
    vec![
        crate::BitratePreset {
            id: "low".into(),
            name: "Low".into(),
            bitrate_kbps: 1500,
            active: false,
        },
        crate::BitratePreset {
            id: "med".into(),
            name: "Medium".into(),
            bitrate_kbps: 4000,
            active: true,
        },
        crate::BitratePreset {
            id: "high".into(),
            name: "High".into(),
            bitrate_kbps: 8000,
            active: false,
        },
        crate::BitratePreset {
            id: "max".into(),
            name: "Maximum".into(),
            bitrate_kbps: 15000,
            active: false,
        },
    ]
}

pub(crate) fn default_quick_actions() -> Vec<crate::QuickAction> {
    let mut actions = vec![
        crate::QuickAction {
            kind: crate::QuickActionKind::OpenSettings,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Settings".into(),
            enabled: true,
            active: false,
        },
        crate::QuickAction {
            kind: crate::QuickActionKind::OpenDebug,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Debug".into(),
            enabled: true,
            active: false,
        },
        crate::QuickAction {
            kind: crate::QuickActionKind::OpenCodecTest,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Codec test".into(),
            enabled: true,
            active: false,
        },
        crate::QuickAction {
            kind: crate::QuickActionKind::ScanQr,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Scan QR".into(),
            enabled: true,
            active: false,
        },
        crate::QuickAction {
            kind: crate::QuickActionKind::OpenRecording,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Record".into(),
            enabled: true,
            active: false,
        },
        crate::QuickAction {
            kind: crate::QuickActionKind::OpenPairing,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Pair".into(),
            enabled: true,
            active: false,
        },
        crate::QuickAction {
            kind: crate::QuickActionKind::OpenBitrate,
            macro_id: "".into(),
            custom_id: "".into(),
            title: "Bitrate".into(),
            enabled: true,
            active: false,
        },
    ];
    if cfg!(debug_assertions) {
        actions.extend([
            crate::QuickAction {
                kind: crate::QuickActionKind::Custom,
                macro_id: "".into(),
                custom_id: "migrated-server".into(),
                title: "Migrated srv".into(),
                enabled: true,
                active: false,
            },
            crate::QuickAction {
                kind: crate::QuickActionKind::Custom,
                macro_id: "".into(),
                custom_id: "test-getinfo".into(),
                title: "GetInfo".into(),
                enabled: true,
                active: false,
            },
            crate::QuickAction {
                kind: crate::QuickActionKind::Custom,
                macro_id: "".into(),
                custom_id: "test-crossfade".into(),
                title: "Crossfade".into(),
                enabled: true,
                active: false,
            },
            crate::QuickAction {
                kind: crate::QuickActionKind::Custom,
                macro_id: "".into(),
                custom_id: "test-smoke".into(),
                title: "Smoke Graph".into(),
                enabled: true,
                active: false,
            },
        ]);
    }
    actions
}
