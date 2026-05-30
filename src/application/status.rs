//! Status pipeline helpers.

// Phase 8 (deferred): producer of Bridge.status-items. Currently unused —
// CastingView renders mock-status-items inline. Keep this helper so the
// Rust side of Phase 8 is a one-line wire-up.
#[allow(dead_code)]
pub(crate) fn build_status_items(
    receiver_name: &str,
    encoder: &str,
    network: &str,
) -> Vec<crate::StatusItem> {
    vec![
        crate::StatusItem {
            label: "Receiver".into(),
            value: receiver_name.into(),
            severity: crate::StatusSeverity::Info,
            icon_glyph: "📺".into(),
        },
        crate::StatusItem {
            label: "Encoder".into(),
            value: encoder.into(),
            severity: crate::StatusSeverity::Info,
            icon_glyph: "⚙️".into(),
        },
        crate::StatusItem {
            label: "Network".into(),
            value: network.into(),
            severity: crate::StatusSeverity::Info,
            icon_glyph: "📶".into(),
        },
    ]
}
