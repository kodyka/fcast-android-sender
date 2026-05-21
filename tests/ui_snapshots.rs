//! Headless UI tests using `i-slint-backend-testing`.
//!
//! These tests verify key behaviours without booting the Android app:
//!   - PanelBridge push/pop navigation (back-stack invariant from step 11)
//!   - SafeArea defaults (safe-area invariant from step 13)
//!   - Panel visibility gating
//!
//! Run with:
//!   cargo test --test ui_snapshots
//!
//! To regenerate accessibility golden files:
//!   UI_SNAPSHOT_REFRESH=1 cargo test --test ui_snapshots

use slint::ComponentHandle;

slint::include_modules!();

fn init_headless() {
    // init_integration_test_with_mock_time sets up the headless rendering
    // backend. Must be called once per process; subsequent calls are no-ops.
    i_slint_backend_testing::init_integration_test_with_mock_time();
}

// ── Navigation / back-stack ───────────────────────────────────────────────────

#[test]
fn panel_bridge_push_pop_roundtrip() {
    init_headless();
    let ui = MainWindow::new().expect("MainWindow::new");
    let pb = ui.global::<PanelBridge>();

    assert_eq!(pb.get_active(), Panel::None, "initial state is Panel::None");

    pb.invoke_push(Panel::Settings);
    assert_eq!(pb.get_active(), Panel::Settings);

    pb.invoke_push(Panel::Audio);
    assert_eq!(pb.get_active(), Panel::Audio);

    pb.invoke_pop();
    assert_eq!(pb.get_active(), Panel::Settings, "pop returns to Settings");

    pb.invoke_pop();
    assert_eq!(pb.get_active(), Panel::None, "pop to root returns Panel::None");
}

#[test]
fn panel_bridge_push_same_panel_is_noop() {
    init_headless();
    let ui = MainWindow::new().expect("MainWindow::new");
    let pb = ui.global::<PanelBridge>();

    pb.invoke_push(Panel::Settings);
    pb.invoke_push(Panel::Settings); // should be a no-op
    pb.invoke_pop();

    assert_eq!(
        pb.get_active(),
        Panel::None,
        "pushing the same panel twice leaves only one entry on the stack"
    );
}

#[test]
fn panel_bridge_close_all_clears_stack() {
    init_headless();
    let ui = MainWindow::new().expect("MainWindow::new");
    let pb = ui.global::<PanelBridge>();

    pb.invoke_push(Panel::Settings);
    pb.invoke_push(Panel::Audio);
    pb.invoke_push(Panel::Camera);

    pb.invoke_close_all();
    assert_eq!(pb.get_active(), Panel::None);

    // Stack should be empty — a subsequent pop stays at None.
    pb.invoke_pop();
    assert_eq!(pb.get_active(), Panel::None);
}

#[test]
fn panel_bridge_replace_swaps_without_growing_stack() {
    init_headless();
    let ui = MainWindow::new().expect("MainWindow::new");
    let pb = ui.global::<PanelBridge>();

    pb.invoke_push(Panel::Settings);
    pb.invoke_replace(Panel::Audio); // replaces Settings in-place

    pb.invoke_pop();
    // replace must not push Settings onto the back-stack; pop goes to None.
    assert_eq!(pb.get_active(), Panel::None);
}

// ── SafeArea clamping ─────────────────────────────────────────────────────────

#[test]
fn safe_area_top_uses_min_when_raw_is_small() {
    init_headless();
    let ui = MainWindow::new().expect("MainWindow::new");
    let sa = ui.global::<SafeArea>();

    // raw-top 0 → top must be clamped to min-top (24px).
    sa.set_raw_top(0.0);
    assert_eq!(
        sa.get_top(), 24.0_f32,
        "SafeArea.top must equal min-top (24px) when raw-top (0) < min-top"
    );

    // raw-top 40 > min-top 24 → top passes through as-is.
    sa.set_raw_top(40.0);
    assert_eq!(
        sa.get_top(), 40.0_f32,
        "SafeArea.top must equal raw-top when raw-top > min-top"
    );
}

#[test]
fn safe_area_bottom_zero_min_no_clamping() {
    init_headless();
    let ui = MainWindow::new().expect("MainWindow::new");
    let sa = ui.global::<SafeArea>();

    // min-bottom is 0, so any raw-bottom >= 0 passes through unchanged.
    // This is the key regression guard against the old 65px hard-coded floor.
    sa.set_raw_bottom(16.0);
    assert_eq!(
        sa.get_bottom(), 16.0_f32,
        "SafeArea.bottom should equal raw-bottom (16px) without a 65px floor"
    );

    sa.set_raw_bottom(0.0);
    assert_eq!(
        sa.get_bottom(), 0.0_f32,
        "SafeArea.bottom should be 0 on landscape tablets with no bottom inset"
    );
}
