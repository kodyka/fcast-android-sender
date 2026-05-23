# STEP 01 — Add Panel Enum Variant

**File:** `ui/bridge.slint`

---

## Goal

Register a new `test-functionality` panel so the navigation stack can
route to the test page.

---

## 1. Add the variant

Locate the `Panel` enum (currently at line 113) and append the new
variant:

```slint
// ui/bridge.slint  (line ~113)

export enum Panel {
    none,
    settings,
    debug,
    codec-test,
    backup-reset,
    audio,
    camera,
    quick-actions,
    cast-history,
    cast-history-detail,
    recording,
    pairing,
    receiver-rename,
    bitrate-presets,
    bitrate-preset-edit,
    macros,
    macro-edit,
    debug-log,
    debug-video,
    network,
    mixer,
    media-backend,
    test-functionality,   // ← NEW
}
```

---

## How it works

* `slint_build::compile("ui/main.slint")` generates a Rust enum
  `Panel` with a matching `TestFunctionality` variant.
* `PanelBridge.push(Panel.test-functionality)` adds the new panel to
  the navigation stack — the same call pattern used by every existing
  page.
* No changes to `ui/state/panel_bridge.slint` are needed; the
  `PanelBridge` global is generic over all `Panel` variants.

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Add `test-functionality,` to the `Panel` enum | `ui/bridge.slint` |
| 2 | Verify `slint-viewer ui/main.slint` still compiles | terminal |

---

## Notes

* The variant name uses kebab-case (`test-functionality`) to match the
  existing convention (`cast-history-detail`, `bitrate-preset-edit`).
* The Rust binding auto-generates `Panel::TestFunctionality` — no manual
  Rust code needed for this step.
