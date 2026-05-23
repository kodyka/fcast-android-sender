# STEP 10 — Add Navigation Entry

**File:** `ui/pages/settings_page.slint` (or other entry point)

---

## Goal

Give the user a way to reach the test functionality page from the UI.
Two options are shown: a settings row (recommended) and an optional
quick action.

---

## Option A: Settings page row (recommended)

Add a `SettingsValueRow` in the developer / debug section of
`settings_page.slint`:

```slint
// ui/pages/settings_page.slint — inside the developer / debug section
// (near the existing "Debug", "Codec test", "Debug video" entries)

SettingsValueRow {
    title: @tr("Test Functionality");
    value: "";
    clicked => { PanelBridge.push(Panel.test-functionality); }
}
```

### Context — where to insert

Locate the section that contains links to debug/test panels (look for
entries like `Panel.codec-test`, `Panel.debug-video`):

```slint
// Existing pattern in settings_page.slint:
SettingsValueRow {
    title: @tr("Codec test");
    value: "";
    clicked => { PanelBridge.push(Panel.codec-test); }
}
// ← Insert the Test Functionality row here
```

---

## Option B: Quick action (optional)

If you also want the test page accessible from the control bar:

### B.1 — Add to `QuickActionKind` enum

```slint
// ui/bridge.slint — inside QuickActionKind enum
export enum QuickActionKind {
    // ... existing kinds ...
    open-test-functionality,   // ← NEW
}
```

### B.2 — Handle in Rust

```rust
// src/lib.rs — inside the quick action handler

QuickActionKind::OpenTestFunctionality => {
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.global::<PanelBridge>().invoke_push(Panel::TestFunctionality);
    });
}
```

### B.3 — Add a default quick action entry

```rust
// In the default quick actions list (likely in src/backend/persistence.rs
// or wherever quick actions are initialized):

QuickAction {
    kind: QuickActionKind::OpenTestFunctionality,
    label: "Test Functionality".into(),
    enabled: true,
    ..Default::default()
}
```

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Add `SettingsValueRow` in the debug/test section | `ui/pages/settings_page.slint` |
| 2 | Verify tapping the row opens the test page | manual test |
| 3 | (Optional) Add `open-test-functionality` to `QuickActionKind` | `ui/bridge.slint` |
| 4 | (Optional) Handle the new kind in Rust | `src/lib.rs` |
| 5 | (Optional) Add default quick action entry | `src/backend/persistence.rs` |

---

## Notes

* `PanelBridge.push(Panel.test-functionality)` adds the panel to the
  navigation stack.  The user can return to the previous screen via the
  "Done" button or the Android back gesture/key (handled by the
  `FocusScope` in `TestFunctionalityPage`).
* The settings row follows the existing pattern where `value: ""` means
  no detail text — the row acts as a simple navigation link.
* Option B (quick action) is fully optional and can be added later.
  Most developer/test features only need a settings entry.
