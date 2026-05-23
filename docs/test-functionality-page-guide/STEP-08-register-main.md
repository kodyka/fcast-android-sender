# STEP 08 — Register in `main.slint`

**File:** `ui/main.slint`

---

## Goal

Import the new page and add it to the `PanelHost` conditional routing
so `PanelBridge.push(Panel.test-functionality)` renders the page.

---

## 1. Add the page import

Insert after the existing `MixerPage` import (line 88):

```slint
// ui/main.slint — add after line 88:
import { TestFunctionalityPage }        from "pages/test_functionality_page.slint";
```

### Full context (surrounding lines)

```slint
import { MixerPage }                    from "pages/mixer_page.slint";
import { MediaBackendPage }             from "pages/media_backend_page.slint";
import { TestFunctionalityPage }        from "pages/test_functionality_page.slint";  // ← NEW
import { Theme } from "theme.slint";
```

---

## 2. Add the PanelHost conditional

Insert after the `media-backend` conditional (line 179):

```slint
// ui/main.slint — inside PanelHost { }, after line 179:
if PanelBridge.active == Panel.test-functionality: TestFunctionalityPage { }
```

### Full context (surrounding lines)

```slint
            if PanelBridge.active == Panel.mixer:               MixerPage               { }
            if PanelBridge.active == Panel.media-backend:       MediaBackendPage        { }
            if PanelBridge.active == Panel.test-functionality:  TestFunctionalityPage   { }  // ← NEW
        }
```

---

## How it works

* `PanelHost` (from `ui/components/panel_host.slint`) is a full-screen
  overlay that manages safe-area insets.  Each panel is conditionally
  instantiated based on `PanelBridge.active`.
* When the user navigates to `Panel.test-functionality`,
  `PanelBridge.push(Panel.test-functionality)` sets
  `PanelBridge.active`, which activates the `if` conditional and
  renders `TestFunctionalityPage`.
* When the user clicks "Done" or presses Escape, `PanelBridge.pop()`
  removes the panel from the stack and returns to the previous screen.

---

## 3. (If STEP 03 was used) Add TestFunctionality global re-export

If you created the `TestFunctionality` state global in STEP 03, also add
it to the main.slint import/export blocks:

```slint
// ui/main.slint — update the state import line (~57):
import { SafeArea, AppBridge, PanelBridge, BannerBridge, MediaBackend, Recording,
         Casting, Receivers, History, Macros, Quickbar, BitratePresets,
         Network, Mixer, DebugLog, Camera, Audio,
         TestFunctionality } from "state/index.slint";

// ui/main.slint — update the state export line (~60):
export { SafeArea, AppBridge, PanelBridge, BannerBridge, MediaBackend, Recording,
         Casting, Receivers, History, Macros, Quickbar, BitratePresets,
         Network, Mixer, DebugLog, Camera, Audio,
         TestFunctionality }
```

This is required for `slint_build` to generate the Rust binding
`ui.global::<TestFunctionality>()`.

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Add `import { TestFunctionalityPage }` after `MediaBackendPage` import | `ui/main.slint` |
| 2 | Add `if PanelBridge.active == Panel.test-functionality:` inside `PanelHost` | `ui/main.slint` |
| 3 | (If STEP 03 used) Add `TestFunctionality` to import + export blocks | `ui/main.slint` |
| 4 | Verify `slint-viewer ui/main.slint` compiles | terminal |

---

## Notes

* The order of `if` conditionals inside `PanelHost` does not affect
  behavior — only one is active at a time.  By convention, new panels
  are appended at the end.
* The `visible: PanelBridge.active == Panel.none` guard on the
  background page (line 137) automatically hides it when any panel is
  open, preventing touch-through issues.
