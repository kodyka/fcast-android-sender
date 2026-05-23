# STEP 12 — SRT & Overlay UI

**Phase:** 4 (Enhanced SRT Source Handling)
**New file:** `ui/pages/srt_config_page.slint`

---

## Goal

Create a comprehensive SRT source configuration page with support for
dynamic source lists, per-source image overlays, and a visual composition
layout preview.

---

## 1. Add the Slint structs

Add the `OverlayItem` struct to `bridge.slint` (if not done in STEP 11)
and a new `SrtSlotEntry` for the dynamic list:

```slint
// bridge.slint — additions

export struct OverlayItem {
    id:        string,
    slot-id:   string,
    visible:   bool,
    source:    string,
    x:         int,
    y:         int,
    width:     int,
    height:    int,
    alpha:     float,
    z-order:   int,
}
```

## 2. Create the SRT config global

```slint
// ui/state/srt_config.slint

import { SrtSource, MixerState, OverlayItem } from "../bridge.slint";

export global SrtConfig {
    // Dynamic list of SRT sources (extends beyond A/B).
    in property <[SrtSource]> sources: [];

    // Overlays attached to all sources.
    in property <[OverlayItem]> overlays: [];

    // ── Commands ──────────────────────────────────────────────────────
    callback add-source();
    callback remove-source(slot-id: string);
    callback update-source(SrtSource);

    callback add-overlay(OverlayItem);
    callback remove-overlay(overlay-id: string);
    callback update-overlay(OverlayItem);
}
```

Register in `ui/state/index.slint` and re-export from `main.slint`.

## 3. Create the SRT config page

```slint
// ui/pages/srt_config_page.slint

import { LineEdit, Switch, ScrollView } from "std-widgets.slint";
import { SrtSource, MixerState, OverlayItem, Panel } from "../bridge.slint";
import { SrtConfig } from "../state/srt_config.slint";
import { PanelBridge } from "../state/panel_bridge.slint";
import { Theme } from "../theme.slint";
import { PrimaryButton, DestructiveButton, TextButton } from "../components/buttons.slint";
import { SettingsSection, SettingsSliderRow } from "../components/settings_rows.slint";
import { PanelHeader, Card, FormRow } from "../components/panel_chrome.slint";

// ── Per-source overlay list ───────────────────────────────────────────
component OverlayRow inherits Rectangle {
    in-out property <OverlayItem> data;
    callback edited();
    callback remove-clicked();

    background: Theme.surface-primary;
    border-radius: Theme.radius-card;
    min-height: 120px;

    VerticalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;

        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: @tr("Overlay");
                color: Theme.text-primary;
                font-size: Theme.font-size-body;
                horizontal-stretch: 1;
                vertical-alignment: center;
            }
            Switch {
                checked <=> root.data.visible;
                toggled => { root.edited(); }
            }
            DestructiveButton {
                label: @tr("Remove");
                clicked => { root.remove-clicked(); }
            }
        }

        FormRow {
            label: @tr("Image path / URL");
            LineEdit {
                text <=> root.data.source;
                edited(t) => { root.edited(); }
            }
        }

        HorizontalLayout {
            spacing: Theme.spacing-default;

            SettingsSliderRow {
                title: @tr("X");
                minimum: -1920;
                maximum: 3840;
                show-fractional: false;
                value: root.data.x;
                changed(v) => { root.data.x = v; root.edited(); }
            }
            SettingsSliderRow {
                title: @tr("Y");
                minimum: -1080;
                maximum: 2160;
                show-fractional: false;
                value: root.data.y;
                changed(v) => { root.data.y = v; root.edited(); }
            }
        }

        HorizontalLayout {
            spacing: Theme.spacing-default;

            SettingsSliderRow {
                title: @tr("Width");
                minimum: 0;
                maximum: 1920;
                show-fractional: false;
                value: root.data.width;
                changed(v) => { root.data.width = v; root.edited(); }
            }
            SettingsSliderRow {
                title: @tr("Height");
                minimum: 0;
                maximum: 1080;
                show-fractional: false;
                value: root.data.height;
                changed(v) => { root.data.height = v; root.edited(); }
            }
        }

        SettingsSliderRow {
            title: @tr("Alpha");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value: root.data.alpha;
            changed(v) => { root.data.alpha = v; root.edited(); }
        }

        SettingsSliderRow {
            title: @tr("Z-order");
            minimum: 0;
            maximum: 99;
            show-fractional: false;
            value: root.data.z-order;
            changed(v) => { root.data.z-order = v; root.edited(); }
        }
    }
}

// ── Per-source card with embedded overlays ────────────────────────────
component SrtSourceCard inherits Rectangle {
    in-out property <SrtSource> data;
    in property <[OverlayItem]> overlays;
    in property <string> title;
    callback source-edited();

    background: Theme.surface-card;
    border-radius: Theme.radius-card;

    VerticalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;

        // Source header
        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: root.title;
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            Switch {
                checked <=> root.data.enabled;
                toggled => { root.source-edited(); }
            }
        }

        FormRow {
            label: @tr("SRT URL");
            LineEdit {
                placeholder-text: @tr("srt://relay.example:9710?mode=caller");
                text <=> root.data.uri;
                edited(t) => { root.source-edited(); }
            }
        }

        SettingsSliderRow {
            title: @tr("Latency (ms)");
            minimum: 0;
            maximum: 8000;
            show-fractional: false;
            value: root.data.latency-ms;
            changed(v) => { root.data.latency-ms = v; root.source-edited(); }
        }

        // Overlays section
        Text {
            text: @tr("IMAGE OVERLAYS");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }

        for overlay in root.overlays: OverlayRow {
            data: overlay;
            edited => {
                SrtConfig.update-overlay(self.data);
            }
            remove-clicked => {
                SrtConfig.remove-overlay(overlay.id);
            }
        }

        TextButton {
            label: @tr("+ Add Overlay");
            clicked => {
                SrtConfig.add-overlay({
                    id: "",
                    slot-id: root.data.slot-id,
                    visible: true,
                    source: "",
                    x: 0, y: 0,
                    width: 0, height: 0,
                    alpha: 1.0,
                    z-order: 10,
                });
            }
        }
    }
}

// ── Composition preview ───────────────────────────────────────────────
component CompositionPreview inherits Rectangle {
    in property <int> canvas-width:  1280;
    in property <int> canvas-height: 720;
    in property <[OverlayItem]> overlays;

    // Scaled-down representation of the mixer canvas.
    background: Theme.surface-primary;
    border-radius: Theme.radius-card;
    min-height: 180px;

    // The actual preview would use Slint's Canvas or a series of
    // coloured Rectangle elements positioned proportionally.
    VerticalLayout {
        padding: Theme.padding-screen;
        Text {
            text: @tr("Composition Preview") + " (" +
                  root.canvas-width + "x" + root.canvas-height + ")";
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
            horizontal-alignment: center;
        }
    }

    // Overlay position indicators (simplified rectangles)
    for overlay in root.overlays: Rectangle {
        x:      overlay.x      * root.width  / root.canvas-width;
        y:      overlay.y      * root.height / root.canvas-height;
        width:  (overlay.width  > 0 ? overlay.width  : 100) * root.width  / root.canvas-width;
        height: (overlay.height > 0 ? overlay.height : 100) * root.height / root.canvas-height;
        background: Theme.accent.transparentize(1.0 - overlay.alpha);
        border-radius: 4px;
        border-width: 1px;
        border-color: Theme.accent;
    }
}

// ── Main page ─────────────────────────────────────────────────────────
export component SrtConfigPage inherits Rectangle {
    width: 100%;
    height: 100%;
    background: Theme.surface-primary;

    VerticalLayout {
        PanelHeader {
            title: @tr("SRT Sources & Overlays");
            close-clicked => { PanelBridge.pop(); }
        }

        ScrollView {
            VerticalLayout {
                alignment: start;
                spacing: Theme.spacing-default;
                padding: Theme.padding-screen;

                // Dynamic SRT source cards
                for src[idx] in SrtConfig.sources: SrtSourceCard {
                    title: @tr("Source") + " " + (idx + 1);
                    data: src;
                    overlays: SrtConfig.overlays;
                    // NOTE: in production, filter overlays by slot-id.
                    source-edited => {
                        SrtConfig.update-source(self.data);
                    }
                }

                PrimaryButton {
                    label: @tr("+ Add SRT Source");
                    clicked => { SrtConfig.add-source(); }
                }

                // Composition preview
                SettingsSection {
                    title: @tr("PREVIEW");
                    CompositionPreview {
                        overlays: SrtConfig.overlays;
                    }
                }
            }
        }
    }
}
```

## 4. Register the panel and navigation

Add `srt-config` to the `Panel` enum in `bridge.slint`:

```slint
export enum Panel {
    // ... existing ...
    srt-config,
}
```

In `main.slint` PanelHost:

```slint
import { SrtConfigPage } from "pages/srt_config_page.slint";
if PanelBridge.active == Panel.srt-config: SrtConfigPage { }
```

Add a navigation entry from the Mixer page or Settings page:

```slint
TextButton {
    label: @tr("SRT Sources & Overlays\u{2026}");
    clicked => { PanelBridge.push(Panel.srt-config); }
}
```

---

## Wire-up checklist

| # | Action | Where |
|---|--------|-------|
| 1 | Create `ui/state/srt_config.slint` + register in index | new file |
| 2 | Create `ui/pages/srt_config_page.slint` | new file |
| 3 | Add `srt-config` to `Panel` enum | `bridge.slint` |
| 4 | Add conditional in PanelHost | `main.slint` |
| 5 | Add navigation from Mixer page or Settings | `mixer_page.slint` |
| 6 | Wire `SrtConfig` callbacks to `SrtSourceManager` + `OverlayManager` in Rust | new Rust file |
| 7 | Verify with `slint-viewer ui/pages/srt_config_page.slint --component SrtConfigPage` | terminal |

---

## Notes

* The `CompositionPreview` is a simplified schematic view.  A true video
  preview would require a GStreamer pipeline rendering to a Slint
  `NativeImage` — that is a Phase 6 stretch goal.
* The overlay list filtering by `slot-id` is noted in the code comment.
  In production, use Slint's `pure function` to filter the model or
  push per-slot sub-models from Rust.
