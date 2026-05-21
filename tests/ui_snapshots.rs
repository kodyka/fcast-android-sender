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

use i_slint_backend_testing::{AccessibleRole, ElementHandle};
use slint::ComponentHandle;

slint::include_modules!();

fn init_headless() {
    i_slint_backend_testing::init_integration_test_with_mock_time();
}

fn wire_panel_bridge(ui: &MainWindow) {
    let panel_stack = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

    ui.global::<PanelBridge>().on_push({
        let stack = panel_stack.clone();
        let ui_weak = ui.as_weak();
        move |p: Panel| {
            let Some(ui) = ui_weak.upgrade() else { return };
            let pb = ui.global::<PanelBridge>();
            let current = pb.get_active();
            if current == p { return; }
            if current != Panel::None {
                stack.borrow_mut().insert(0, current);
            }
            pb.set_active(p);
            pb.set_stack(std::rc::Rc::new(slint::VecModel::from(stack.borrow().clone())).into());
        }
    });

    ui.global::<PanelBridge>().on_pop({
        let stack = panel_stack.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            let pb = ui.global::<PanelBridge>();
            let next = if stack.borrow().is_empty() {
                Panel::None
            } else {
                stack.borrow_mut().remove(0)
            };
            pb.set_active(next);
            pb.set_stack(std::rc::Rc::new(slint::VecModel::from(stack.borrow().clone())).into());
        }
    });

    ui.global::<PanelBridge>().on_replace({
        let ui_weak = ui.as_weak();
        move |p: Panel| {
            if let Some(ui) = ui_weak.upgrade() {
                ui.global::<PanelBridge>().set_active(p);
            }
        }
    });

    ui.global::<PanelBridge>().on_close_all({
        let stack = panel_stack.clone();
        let ui_weak = ui.as_weak();
        move || {
            let Some(ui) = ui_weak.upgrade() else { return };
            stack.borrow_mut().clear();
            let pb = ui.global::<PanelBridge>();
            pb.set_active(Panel::None);
            pb.set_stack(std::rc::Rc::new(slint::VecModel::from(stack.borrow().clone())).into());
        }
    });
}

#[test]
fn ui_snapshots_all() {
    init_headless();

    // ── Navigation / back-stack ───────────────────────────────────────────────────

    // 1. panel_bridge_push_pop_roundtrip
    {
        let ui = MainWindow::new().expect("MainWindow::new");
        wire_panel_bridge(&ui);
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

    // 2. panel_bridge_push_same_panel_is_noop
    {
        let ui = MainWindow::new().expect("MainWindow::new");
        wire_panel_bridge(&ui);
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

    // 3. panel_bridge_close_all_clears_stack
    {
        let ui = MainWindow::new().expect("MainWindow::new");
        wire_panel_bridge(&ui);
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

    // 4. panel_bridge_replace_swaps_without_growing_stack
    {
        let ui = MainWindow::new().expect("MainWindow::new");
        wire_panel_bridge(&ui);
        let pb = ui.global::<PanelBridge>();

        pb.invoke_push(Panel::Settings);
        pb.invoke_replace(Panel::Audio); // replaces Settings in-place

        pb.invoke_pop();
        // replace must not push Settings onto the back-stack; pop goes to None.
        assert_eq!(pb.get_active(), Panel::None);
    }

    // ── SafeArea clamping ─────────────────────────────────────────────────────────

    // 5. safe_area_top_uses_min_when_raw_is_small
    {
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

    // 6. safe_area_bottom_zero_min_no_clamping
    {
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

    // ── Home → Settings navigation via accessibility action ───────────────────────

    // 7. home_screen_settings_button_opens_settings_panel
    //
    // Behavioural smoke test of the start-screen → Settings flow:
    //   1. Seed Bridge.quick-actions with the OpenSettings entry that
    //      `default_quick_actions()` ships in production (src/lib.rs).
    //   2. Locate the rendered QuickActionButton by its accessible label.
    //   3. Invoke its default accessibility action (equivalent to a click /
    //      Space / Enter on a focused button).
    //   4. Assert PanelBridge transitioned to Panel::Settings.
    //
    // This exercises the full CastControlBar wiring — the for-loop repeater,
    // the QuickActionButton's accessible-action-default handler, and the
    // PanelBridge.push dispatch in control_bar.slint — without touching Rust
    // callbacks or the real Android event loop.
    {
        let ui = MainWindow::new().expect("MainWindow::new");
        wire_panel_bridge(&ui);
        let pb = ui.global::<PanelBridge>();
        let bridge = ui.global::<Bridge>();

        let actions = std::rc::Rc::new(slint::VecModel::from(vec![QuickAction {
            kind: QuickActionKind::OpenSettings,
            title: "Settings".into(),
            macro_id: "".into(),
            custom_id: "".into(),
            enabled: true,
            active: false,
        }]));
        bridge.set_quick_actions(actions.into());

        assert_eq!(
            pb.get_active(),
            Panel::None,
            "home screen starts with no panel open"
        );

        // The testing backend's element walker visits conditional branches
        // too, so the PanelHeader inside the (currently-hidden) FullSettingsPage
        // also publishes label "Settings". Filter by AccessibleRole::Button
        // to pick the QuickActionButton on the home screen.
        let buttons: Vec<_> = ElementHandle::find_by_accessible_label(&ui, "Settings")
            .filter(|el| el.accessible_role() == Some(AccessibleRole::Button))
            .collect();
        assert_eq!(
            buttons.len(),
            1,
            "exactly one Settings quick-action button is rendered on the home screen"
        );

        buttons[0].invoke_accessible_default_action();

        assert_eq!(
            pb.get_active(),
            Panel::Settings,
            "tapping the Settings quick-action opens Panel::Settings"
        );
    }
}
