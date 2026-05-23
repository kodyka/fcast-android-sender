# STEP 04 — Camera Source Section

**Internal component for:** `ui/pages/test_functionality_page.slint`

---

## Goal

Build the Camera Source section of the test page, adapted from
`camera_page.slint` (SOURCE section, lines 83-106) and the Moblin
`CameraSettingsView.swift` (lines 190-284).

---

## Design reference

### From `camera_page.slint` SOURCE section

The existing camera page uses:

```slint
SettingsValueRow {
    title: @tr("Camera");
    value: [@tr("Front"), @tr("Back"), @tr("External")][Math.clamp(Bridge.camera-idx, 0, 2)];
    clicked => { Bridge.camera-idx = Math.mod(Bridge.camera-idx + 1, 3); }
}
```

This pattern cycles through options on tap — simple and mobile-friendly.

### From `CameraSettingsView.swift`

The Moblin camera settings has these sections:

| Section | Controls | Our Slint equivalent |
|---------|----------|---------------------|
| Video (NavigationLink) | Resolution, codec, bitrate | `SettingsValueRow` (resolution, framerate) |
| Camera controls | Zoom, stabilization, mirror, tap-to-focus | `SettingsSliderRow` (zoom), `SettingsToggleRow` (mirror, stabilization) |
| Color space | Apple Log, LUT picker | *Not included — future stretch goal* |

---

## Component snippet

```slint
// Inside test_functionality_page.slint — Camera source section.
//
// This is used directly inside the ScrollView VerticalLayout; it is not
// a standalone component but a SettingsSection block.

// ── Section: CAMERA SOURCE ────────────────────────────────────────────
// Adapted from camera_page.slint SOURCE + IMAGE + ZOOM sections.
// Design ref: draft/moblin-ui/.../CameraSettingsView.swift
SettingsSection {
    title: @tr("CAMERA SOURCE");

    // Camera selector — cycles Front → Back → External on tap.
    // Uses the same inline-array indexing pattern as camera_page.slint.
    SettingsValueRow {
        title: @tr("Camera");
        value: [
            @tr("Front"),
            @tr("Back"),
            @tr("External"),
        ][Math.clamp(Bridge.test-camera-idx, 0, 2)];
        clicked => {
            Bridge.test-camera-idx =
                Math.mod(Bridge.test-camera-idx + 1, 3);
        }
    }

    // Resolution selector — cycles 480p → 720p → 1080p → 4K.
    // Adapted from CameraSettingsView → StreamVideoSettingsView.
    SettingsValueRow {
        title: @tr("Resolution");
        value: [
            "480p", "720p", "1080p", "4K",
        ][Math.clamp(Bridge.test-resolution-idx, 0, 3)];
        clicked => {
            Bridge.test-resolution-idx =
                Math.mod(Bridge.test-resolution-idx + 1, 4);
        }
    }

    // Framerate selector — cycles 24 → 30 → 60 fps.
    SettingsValueRow {
        title: @tr("Framerate");
        value: [
            "24 fps", "30 fps", "60 fps",
        ][Math.clamp(Bridge.test-framerate-idx, 0, 2)];
        clicked => {
            Bridge.test-framerate-idx =
                Math.mod(Bridge.test-framerate-idx + 1, 3);
        }
    }

    // Mirror — maps to CameraSettingsView → MirrorFrontCameraOnStreamView.
    SettingsToggleRow {
        title: @tr("Mirror front camera");
        checked: Bridge.test-camera-mirror;
        toggled(checked) => {
            Bridge.test-camera-mirror = checked;
        }
    }

    // Stabilization — maps to CameraSettingsView → VideoStabilizationSettingsView.
    SettingsToggleRow {
        title: @tr("Video stabilization");
        checked: Bridge.test-camera-stabilization;
        toggled(checked) => {
            Bridge.test-camera-stabilization = checked;
        }
    }

    // Zoom slider — maps to CameraSettingsView → ZoomSettingsView.
    // Range 0.5×–5.0× with fractional display, same as camera_page.slint.
    SettingsSliderRow {
        title: @tr("Zoom");
        unit: "\u{00D7}";     // × symbol
        minimum: 0.5;
        maximum: 5.0;
        show-fractional: true;
        value <=> Bridge.test-camera-zoom;
    }
}
```

---

## Mapping from Swift → Slint (camera section)

| CameraSettingsView.swift element | Slint element | Property |
|----------------------------------|---------------|----------|
| `VideoStabilizationSettingsView(mode:)` | `SettingsToggleRow` | `test-camera-stabilization` |
| `MirrorFrontCameraOnStreamView(...)` | `SettingsToggleRow` | `test-camera-mirror` |
| `ZoomSettingsView(zoom:)` | `SettingsSliderRow` | `test-camera-zoom` |
| `TapScreenToFocusSettingsView(...)` | *Omitted* — Slint has no tap-to-focus gesture API | — |
| `CameraControlsView(...)` | *Omitted* — volume button hijack is Android-specific | — |
| `Picker("Color space", selection:)` | *Omitted* — stretch goal for future step | — |

---

## Wire-up checklist

| # | Action |
|---|--------|
| 1 | Copy the `SettingsSection { title: @tr("CAMERA SOURCE"); ... }` block into the page's ScrollView |
| 2 | Ensure `Bridge.test-camera-*` properties exist (STEP 02) |
| 3 | Verify the section renders correctly in `slint-viewer` |

---

## Notes

* The `SettingsValueRow` tap-to-cycle pattern is intentional for mobile —
  it avoids a dropdown/combo-box which requires precise tap targeting.
  This mirrors the existing camera page behavior.
* The zoom slider uses `<=>` two-way binding because `SettingsSliderRow.value`
  and `Bridge.test-camera-zoom` are both `float`.  No type conversion needed.
* If you used the `TestFunctionality` global from STEP 03, replace all
  `Bridge.test-camera-*` references with `TestFunctionality.camera-*`.
