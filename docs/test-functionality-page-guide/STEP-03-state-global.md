# STEP 03 — Feature-Scoped State Global

**File:** `ui/state/test_functionality.slint` (new)

---

## Goal

Create an optional feature-scoped global that mirrors the `Bridge.test-*`
properties, following the pattern established by `ui/state/camera.slint`,
`ui/state/mixer.slint`, and other state globals.

> **This step is optional.**  The page component in STEP 07 can use
> `Bridge.test-*` properties directly.  Create the global only if you
> want the cleaner encapsulation seen in other state files.

---

## 1. Create the global

```slint
// ui/state/test_functionality.slint — Test functionality state.
//
// Mirrors Bridge.test-* properties into a feature-scoped global,
// following the camera.slint / mixer.slint pattern.

import { MixerState, SrtSource } from "../bridge.slint";

export global TestFunctionality {
    // ── Camera source ─────────────────────────────────────────────────
    in-out property <int>   camera-idx:           1;
    in-out property <int>   resolution-idx:       2;
    in-out property <int>   framerate-idx:        1;
    in-out property <bool>  camera-mirror:        false;
    in-out property <bool>  camera-stabilization: true;
    in-out property <float> camera-zoom:          1.0;

    // ── SRT source ────────────────────────────────────────────────────
    in-out property <SrtSource> srt-source: {
        slot-id:    "test-srt",
        enabled:    true,
        uri:        "",
        latency-ms: 2000,
        stream-id:  "",
        mix-alpha:  1.0,
        mix-zorder: 0,
        mix-volume: 1.0,
        state:      MixerState.idle,
        last-error: "",
    };

    // ── Image overlay ─────────────────────────────────────────────────
    in-out property <string> overlay-image-path: "";
    in-out property <bool>   overlay-enabled:    false;
    in-out property <float>  overlay-x:          0;
    in-out property <float>  overlay-y:          0;
    in-out property <float>  overlay-width:      320;
    in-out property <float>  overlay-height:     180;
    in-out property <float>  overlay-alpha:      1.0;
    in-out property <int>    overlay-z-order:    10;

    // ── Test lifecycle ────────────────────────────────────────────────
    in property <MixerState> state:      MixerState.idle;
    in property <string>     error-text: "";

    callback start-test();
    callback stop-test();
    callback pick-overlay-image();
}
```

### Comparison with existing state globals

| Existing global | Properties | Follows same pattern? |
|-----------------|------------|----------------------|
| `Camera` (`ui/state/camera.slint`) | `idx`, `resolution-idx`, `framerate-idx`, `mirror-front`, `stabilization`, `tap-to-focus`, `zoom-level` | Yes — camera section modelled after this |
| `Mixer` (`ui/state/mixer.slint`) | `srt-source-a`, `srt-source-b`, `rtmp-destination`, `canvas`, `state`, `error-text`, callbacks | Yes — SRT section and lifecycle modelled after this |

---

## 2. Register in `ui/state/index.slint`

```slint
// ui/state/index.slint  (add import + re-export)

import { TestFunctionality } from "test_functionality.slint";

// Update the export block (line 23-25):
export { SafeArea, AppBridge, PanelBridge, BannerBridge, MediaBackend, Recording,
         Casting, Receivers, History, Macros, Quickbar, BitratePresets,
         Network, Mixer, DebugLog, Camera, Audio, TestFunctionality }
```

---

## 3. Register in `ui/main.slint`

```slint
// ui/main.slint  (update the import/export blocks around line 57-62)

import { SafeArea, AppBridge, PanelBridge, BannerBridge, MediaBackend, Recording,
         Casting, Receivers, History, Macros, Quickbar, BitratePresets,
         Network, Mixer, DebugLog, Camera, Audio, TestFunctionality } from "state/index.slint";
export { SafeArea, AppBridge, PanelBridge, BannerBridge, MediaBackend, Recording,
         Casting, Receivers, History, Macros, Quickbar, BitratePresets,
         Network, Mixer, DebugLog, Camera, Audio, TestFunctionality }
```

This makes `TestFunctionality` available in Rust via:

```rust
ui.global::<TestFunctionality>().on_start_test(|| { ... });
ui.global::<TestFunctionality>().set_state(MixerState::Running);
```

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Create `ui/state/test_functionality.slint` | new file |
| 2 | Add import + re-export in `ui/state/index.slint` | existing file |
| 3 | Add import + re-export in `ui/main.slint` | existing file |
| 4 | Verify `slint-viewer ui/main.slint` still compiles | terminal |

---

## Notes

* If you skip this step and use `Bridge.test-*` directly, you still need
  the Bridge properties from STEP 02.  The page snippets in STEPs 04-07
  show both approaches with comments.
* The state global and Bridge properties can coexist — Rust wiring can
  sync them via `on_start_test` forwarding to `Bridge.start_test()` if
  needed.
