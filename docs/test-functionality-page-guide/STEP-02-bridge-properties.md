# STEP 02 — Bridge Properties & Callbacks

**File:** `ui/bridge.slint`

---

## Goal

Add all the properties and callbacks the test page needs.  They live in
the central `Bridge` global so the Rust backend can read/write them via
`ui.global::<Bridge>()`.

---

## 1. Camera source properties

Insert after the existing camera properties (line ~282):

```slint
// ui/bridge.slint  —  inside `global Bridge { }`
// ── Test functionality: camera source ─────────────────────────────────
in-out property <int>    test-camera-idx:           1;   // 0=Front 1=Back 2=External
in-out property <int>    test-resolution-idx:       2;   // 0=480p  1=720p 2=1080p 3=4K
in-out property <int>    test-framerate-idx:        1;   // 0=24fps 1=30fps 2=60fps
in-out property <bool>   test-camera-mirror:        false;
in-out property <bool>   test-camera-stabilization: true;
in-out property <float>  test-camera-zoom:          1.0;
```

### Design rationale

These are separate from the existing `Bridge.camera-idx` etc. because the
test screen is an independent pipeline — changing a test setting should not
affect the main casting camera.

The index-to-label mapping is done in the Slint page using inline arrays,
identical to `camera_page.slint`:

```slint
value: ["Front", "Back", "External"][Math.clamp(Bridge.test-camera-idx, 0, 2)];
```

---

## 2. SRT source properties

```slint
// ── Test functionality: SRT source ────────────────────────────────────
in-out property <SrtSource> test-srt-source: {
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
```

### Design rationale

Reuses the existing `SrtSource` struct (defined at line 197) so the
`TestSrtSourceCard` component can use the same `<SrtSource>` type as the
mixer page's `SrtSourceRow`.

Default values match `Bridge.srt-source-a` for familiarity.

---

## 3. Image overlay properties

```slint
// ── Test functionality: image overlay ─────────────────────────────────
in-out property <string>  test-overlay-image-path: "";
in-out property <bool>    test-overlay-enabled:    false;
in-out property <float>   test-overlay-x:          0;
in-out property <float>   test-overlay-y:          0;
in-out property <float>   test-overlay-width:      320;
in-out property <float>   test-overlay-height:     180;
in-out property <float>   test-overlay-alpha:      1.0;
in-out property <int>     test-overlay-z-order:    10;
```

### Mapping from `WidgetImageSettingsView.swift`

| Moblin property | Bridge property | Notes |
|-----------------|-----------------|-------|
| `widget.image` (via `PhotosPicker`) | `test-overlay-image-path` | Slint has no picker; Rust opens intent |
| Widget position (scene editor) | `test-overlay-x`, `test-overlay-y` | Slider controls |
| Widget size | `test-overlay-width`, `test-overlay-height` | 0 = use original |
| Opacity (WidgetEffectsView) | `test-overlay-alpha` | 0.0–1.0 range |
| Z-order | `test-overlay-z-order` | GStreamer compositor `zorder` pad property |

---

## 4. Test lifecycle state + callbacks

```slint
// ── Test functionality: lifecycle ─────────────────────────────────────
in property <MixerState> test-state:      MixerState.idle;
in property <string>     test-error-text: "";

callback start-test();
callback stop-test();
callback pick-test-overlay-image();
```

### Callback signatures

| Callback | Direction | Purpose |
|----------|-----------|---------|
| `start-test()` | Slint → Rust | Build + start the GStreamer test pipeline |
| `stop-test()` | Slint → Rust | Tear down the pipeline |
| `pick-test-overlay-image()` | Slint → Rust | Open Android intent / desktop file dialog; write result back to `test-overlay-image-path` |

`test-state` is `in` (Rust-owned) because the backend drives state
transitions: `idle → starting → running → stopping → idle`.

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Add camera properties (6 lines) | `ui/bridge.slint` |
| 2 | Add SRT source property (1 struct init) | `ui/bridge.slint` |
| 3 | Add overlay properties (8 lines) | `ui/bridge.slint` |
| 4 | Add `test-state`, `test-error-text`, 3 callbacks | `ui/bridge.slint` |
| 5 | Verify `slint-viewer ui/main.slint` still compiles | terminal |

---

## Notes

* All new properties are `in-out` (UI and Rust can write) except
  `test-state` and `test-error-text` which are `in` (Rust-only).
* The `SrtSource` struct is already exported by `bridge.slint` — no new
  type definitions needed.
