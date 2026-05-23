# STEP 07 — Assemble the Page

**New file:** `ui/pages/test_functionality_page.slint`

---

## Goal

Assemble all internal components (STEPS 04-06) into a complete page
component that follows the existing page structure: `PanelHeader` +
`ScrollView` + sections + bottom action bar.

---

## Page structure

```text
┌────────────────────────────────────┐
│ PanelHeader  "Test Functionality"  │  ← 56px, Theme.surface-card
├────────────────────────────────────┤
│                                    │
│  ScrollView                        │
│  ┌──────────────────────────────┐  │
│  │ [error banner if present]    │  │
│  │                              │  │
│  │ ── CAMERA SOURCE ──────────  │  │  ← STEP 04 section
│  │  Camera:     Back  ›         │  │
│  │  Resolution: 1080p ›         │  │
│  │  Framerate:  30 fps ›        │  │
│  │  □ Mirror front camera       │  │
│  │  □ Video stabilization       │  │
│  │  Zoom  ━━━━━━●━━━ 1.0×      │  │
│  │                              │  │
│  │ ── SRT SOURCE ─────────────  │  │  ← STEP 05 section
│  │  ┌─ SRT Source  idle ☐ ──┐  │  │
│  │  │ URL: [_____________]  │  │  │
│  │  │ Latency ━━●━━━ 2000ms │  │  │
│  │  │ Stream ID: [________] │  │  │
│  │  │ Alpha ━━━━━━━━●━ 1.0  │  │  │
│  │  │ Volume ━━━━━━━●━ 1.0  │  │  │
│  │  └───────────────────────┘  │  │
│  │                              │  │
│  │ ── IMAGE OVERLAY ──────────  │  │  ← STEP 06 section
│  │  ┌─ Image Overlay ☐ ────┐  │  │
│  │  │ Path: [____] [Browse] │  │  │
│  │  │ ┌── preview ───────┐  │  │  │
│  │  │ │  No image sel.   │  │  │  │
│  │  │ └──────────────────┘  │  │  │
│  │  │ X pos  ━━●━━ 0px     │  │  │
│  │  │ Y pos  ━━●━━ 0px     │  │  │
│  │  │ Width  ━━━●━ 320px   │  │  │
│  │  │ Height ━━●━━ 180px   │  │  │
│  │  │ Alpha  ━━━━━●━ 1.0   │  │  │
│  │  │ Z-order ━━●━ 10      │  │  │
│  │  └───────────────────────┘  │  │
│  │                              │  │
│  │  [96px spacer]              │  │
│  └──────────────────────────────┘  │
│                                    │
├────────────────────────────────────┤
│  idle    [ Start Test ][ Stop ]    │  ← 96px action bar
└────────────────────────────────────┘
```

---

## Complete file

```slint
// ui/pages/test_functionality_page.slint — Test Functionality screen.
//
// Combines three test sources: phone camera, SRT ingest, and image overlay.
// Adapted from:
//   camera_page.slint          — SOURCE section pattern  (STEP 04)
//   mixer_page.slint           — SrtSourceRow component  (STEP 05)
//   draft/moblin-ui/.../CameraSettingsView.swift         (camera design ref)
//   draft/moblin-ui/.../SrtlaServerSettingsView.swift     (SRT design ref)
//   draft/moblin-ui/.../WidgetImageSettingsView.swift     (overlay design ref)

import { LineEdit, Switch, ScrollView } from "std-widgets.slint";
import { Bridge, Panel, SrtSource, MixerState } from "../bridge.slint";
import { PanelBridge } from "../state/panel_bridge.slint";
import { Theme } from "../theme.slint";
import { PrimaryButton, TextButton, DestructiveButton } from "../components/buttons.slint";
import {
    SettingsSection,
    SettingsValueRow,
    SettingsToggleRow,
    SettingsSliderRow,
} from "../components/settings_rows.slint";
import { PanelHeader } from "../components/panel_chrome.slint";


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Internal: state chip (from STEP 05)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
component TestStateChip inherits Text {
    in property <MixerState> state: MixerState.idle;
    font-size: Theme.font-size-label;
    vertical-alignment: center;

    states [
        idle     when root.state == MixerState.idle     : { text: @tr("idle");     color: Theme.text-secondary; }
        starting when root.state == MixerState.starting : { text: @tr("starting"); color: Theme.text-secondary; }
        running  when root.state == MixerState.running  : { text: @tr("running");  color: Theme.success;        }
        stopping when root.state == MixerState.stopping : { text: @tr("stopping"); color: Theme.text-secondary; }
        error    when root.state == MixerState.error    : { text: @tr("error");    color: Theme.error-fg;       }
    ]
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Internal: SRT source card (from STEP 05)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
component TestSrtSourceCard inherits Rectangle {
    in-out property <SrtSource> data;
    callback edited();

    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 280px;

    VerticalLayout {
        padding-left: Theme.padding-screen;
        padding-right: Theme.padding-screen;
        padding-top: Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing: Theme.spacing-default;

        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: @tr("SRT Source");
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            TestStateChip { state: root.data.state; }
            Switch {
                checked <=> root.data.enabled;
                toggled() => { root.edited(); }
            }
        }

        Text {
            text: @tr("URL");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            placeholder-text: @tr("srt://relay.example:9710?mode=caller");
            text <=> root.data.uri;
            edited(text) => { root.edited(); }
        }

        SettingsSliderRow {
            title: @tr("Latency");
            unit: @tr(" ms");
            minimum: 0;
            maximum: 8000;
            show-fractional: false;
            value: root.data.latency-ms;
            changed(v) => {
                root.data.latency-ms = v;
                root.edited();
            }
        }

        Text {
            text: @tr("Stream ID (optional)");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            placeholder-text: @tr("publish:my-stream-key");
            text <=> root.data.stream-id;
            edited(text) => { root.edited(); }
        }

        SettingsSliderRow {
            title: @tr("Alpha");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> root.data.mix-alpha;
        }

        SettingsSliderRow {
            title: @tr("Volume");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> root.data.mix-volume;
        }

        if root.data.last-error != "": Text {
            text: root.data.last-error;
            color: Theme.error-fg;
            font-size: Theme.font-size-label;
            wrap: word-wrap;
        }
    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Internal: image overlay card (from STEP 06)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
component ImageOverlayCard inherits Rectangle {
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 460px;

    VerticalLayout {
        padding-left: Theme.padding-screen;
        padding-right: Theme.padding-screen;
        padding-top: Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing: Theme.spacing-default;

        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: @tr("Image Overlay");
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            Switch {
                checked <=> Bridge.test-overlay-enabled;
            }
        }

        Text {
            text: @tr("Image file path");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        HorizontalLayout {
            spacing: Theme.spacing-default;
            LineEdit {
                placeholder-text: @tr("/sdcard/DCIM/overlay.png");
                text <=> Bridge.test-overlay-image-path;
                horizontal-stretch: 1;
            }
            PrimaryButton {
                label: @tr("Browse");
                clicked => { Bridge.pick-test-overlay-image(); }
            }
        }

        Rectangle {
            height: 120px;
            border-radius: Theme.radius-card;
            background: Bridge.test-overlay-image-path != ""
                ? Theme.surface-bar
                : Theme.surface-primary;
            Text {
                text: Bridge.test-overlay-image-path != ""
                    ? @tr("Preview available when test is running")
                    : @tr("No image selected");
                color: Theme.text-secondary;
                font-size: Theme.font-size-label;
                horizontal-alignment: center;
                vertical-alignment: center;
            }
        }

        SettingsSliderRow {
            title: @tr("X position");
            unit: @tr(" px");
            minimum: -1920;
            maximum: 3840;
            show-fractional: false;
            value <=> Bridge.test-overlay-x;
        }
        SettingsSliderRow {
            title: @tr("Y position");
            unit: @tr(" px");
            minimum: -1080;
            maximum: 2160;
            show-fractional: false;
            value <=> Bridge.test-overlay-y;
        }
        SettingsSliderRow {
            title: @tr("Width");
            unit: @tr(" px");
            minimum: 0;
            maximum: 1920;
            show-fractional: false;
            value <=> Bridge.test-overlay-width;
        }
        SettingsSliderRow {
            title: @tr("Height");
            unit: @tr(" px");
            minimum: 0;
            maximum: 1080;
            show-fractional: false;
            value <=> Bridge.test-overlay-height;
        }
        SettingsSliderRow {
            title: @tr("Alpha");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> Bridge.test-overlay-alpha;
        }
        SettingsSliderRow {
            title: @tr("Z-order");
            minimum: 0;
            maximum: 99;
            show-fractional: false;
            value: Bridge.test-overlay-z-order;
            changed(v) => { Bridge.test-overlay-z-order = v; }
        }
    }
}


// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//  Exported: TestFunctionalityPage
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
export component TestFunctionalityPage inherits Rectangle {
    width: 100%;
    height: 100%;
    background: Theme.surface-primary;

    forward-focus: panel-scope;
    panel-scope := FocusScope {
        key-pressed(event) => {
            if event.text == Key.Escape { PanelBridge.pop(); return accept; }
            return reject;
        }

    VerticalLayout {
        // ── Header ─────────────────────────────────────────────────────
        PanelHeader {
            title: @tr("Test Functionality");
            close-clicked => { PanelBridge.pop(); }
        }

        // ── Body (scrollable) ──────────────────────────────────────────
        ScrollView {
            mouse-drag-pan-enabled: true;
            VerticalLayout {
                alignment: start;
                spacing: Theme.spacing-default;
                padding: Theme.padding-screen;

                // ── Error banner ───────────────────────────────────────
                if Bridge.test-error-text != "": Rectangle {
                    background: Theme.error;
                    border-radius: Theme.radius-card;
                    min-height: 40px;
                    Text {
                        text: Bridge.test-error-text;
                        color: Theme.text-primary;
                        font-size: Theme.font-size-label;
                        wrap: word-wrap;
                        x: Theme.padding-card;
                        width: parent.width - Theme.padding-card * 2;
                        vertical-alignment: center;
                    }
                }

                // ── Section 1: CAMERA SOURCE (STEP 04) ────────────────
                SettingsSection {
                    title: @tr("CAMERA SOURCE");

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

                    SettingsToggleRow {
                        title: @tr("Mirror front camera");
                        checked: Bridge.test-camera-mirror;
                        toggled(checked) => {
                            Bridge.test-camera-mirror = checked;
                        }
                    }

                    SettingsToggleRow {
                        title: @tr("Video stabilization");
                        checked: Bridge.test-camera-stabilization;
                        toggled(checked) => {
                            Bridge.test-camera-stabilization = checked;
                        }
                    }

                    SettingsSliderRow {
                        title: @tr("Zoom");
                        unit: "\u{00D7}";
                        minimum: 0.5;
                        maximum: 5.0;
                        show-fractional: true;
                        value <=> Bridge.test-camera-zoom;
                    }
                }

                // ── Section 2: SRT SOURCE (STEP 05) ───────────────────
                SettingsSection {
                    title: @tr("SRT SOURCE");

                    TestSrtSourceCard {
                        data <=> Bridge.test-srt-source;
                        edited => { }
                    }
                }

                // ── Section 3: IMAGE OVERLAY (STEP 06) ────────────────
                SettingsSection {
                    title: @tr("IMAGE OVERLAY");

                    ImageOverlayCard { }
                }

                // Spacer — matches footer height so last section scrolls
                // fully above the action bar.
                Rectangle { height: 96px; background: transparent; }
            }
        }

        // ── Bottom action bar ──────────────────────────────────────────
        // Same 96px bar pattern as mixer_page.slint (lines 432-454).
        Rectangle {
            height: 96px;
            background: Theme.surface-bar;

            HorizontalLayout {
                padding: Theme.padding-screen;
                spacing: Theme.spacing-default;

                TestStateChip {
                    state: Bridge.test-state;
                    horizontal-stretch: 0;
                }

                Rectangle { horizontal-stretch: 1; }

                PrimaryButton {
                    label: @tr("Start Test");
                    enabled: Bridge.test-state == MixerState.idle
                          || Bridge.test-state == MixerState.error;
                    clicked => { Bridge.start-test(); }
                    horizontal-stretch: 1;
                }
                DestructiveButton {
                    label: @tr("Stop Test");
                    enabled: Bridge.test-state == MixerState.running
                          || Bridge.test-state == MixerState.starting;
                    clicked => { Bridge.stop-test(); }
                    horizontal-stretch: 1;
                }
            }
        }
    }
    }   // end FocusScope
}
```

---

## Wire-up checklist

| # | Action | File |
|---|--------|------|
| 1 | Create `ui/pages/test_functionality_page.slint` with the full content above | new file |
| 2 | Verify `slint-viewer ui/pages/test_functionality_page.slint --component TestFunctionalityPage` renders | terminal |
| 3 | Proceed to STEP 08 to register in `main.slint` | next step |

---

## Notes

* The page is a single file containing three internal components
  (`TestStateChip`, `TestSrtSourceCard`, `ImageOverlayCard`) and one
  exported component (`TestFunctionalityPage`).  This matches the
  pattern of `mixer_page.slint` which has 5 internal components.
* The bottom action bar mirrors `mixer_page.slint` exactly: two buttons
  (Start/Stop) with state-based `enabled` guards.
* The error banner pattern is copied from `mixer_page.slint` line 370.
