# 04 — `Card` & `PanelHeader`: reusable wrappers for the page chrome

## Goal

Extract three primitives that the page layer is currently re-deriving
by hand on every screen:

1. `PanelHeader` — the **56 px header row** with a title and a "Done"
   `TextButton`. Used by 17 panel pages today.
2. `Card` — the **surface-card rounded rectangle** with screen padding,
   used as the visual container for every settings group. Repeated
   ≥60 times across the page layer.
3. `FormRow` — a "label + control" 2-column row used inside every
   settings card.

Together they cut the page files by ~30–40 % and put the design-system
contract in one place.

## Findings

### F3 — duplicated panel header (17 pages)

The same Rectangle + Text + TextButton block recurs verbatim in:

- `media_backend_page.slint:15–32`
- `settings_page.slint:82–99`
- `recording_page.slint:75–93`
- `macro_edit_page.slint:40–67`
- `backup_reset_page.slint`
- `audio_page.slint`, `camera_page.slint`, `bitrate_*_page.slint`
- `cast_history_page.slint`, `cast_history_detail_page.slint`
- `debug_log_page.slint`, `debug_video_page.slint`, `debug_page.slint`
- `network_page.slint`, `mixer_page.slint`, `pairing_page.slint`,
  `receiver_rename_page.slint`, `quick_actions_page.slint`,
  `macros_page.slint`

Every instance follows the same Schema:

```slint
Rectangle {
    height: 56px;
    background: Theme.surface-card;
    HorizontalLayout {
        padding: Theme.padding-screen;
        Text {
            text: @tr("…title…");
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;
        }
        TextButton {
            label: @tr("close-panel-button" => "Done");
            clicked => { Bridge.active-panel = Panel.none; }
        }
    }
}
```

Some headers (`macro_edit_page.slint`) add a "Save" button on the right;
others (`receiver_rename_page.slint`, `pairing_page.slint`) use
"Cancel" + a custom button. All of them follow the title-flex-right-cluster
pattern.

### F4 — duplicated card wrapping (60+ sites)

`grep -nE "background: Theme\\.surface-card" ui/pages/*.slint | wc -l`
→ **62**. Almost all are the same body shape:

```slint
Rectangle {
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 88px;          // or 56 / 64 / 200 — varies
    VerticalLayout {
        padding-left:  Theme.padding-screen;
        padding-right: Theme.padding-screen;
        padding-top:   12px;
        padding-bottom: 12px;
        spacing: 4px;
        // … content …
    }
}
```

### FormRow

Inside `media_backend_page.slint` and `mixer_page.slint`, every input
row reads as "Text label + LineEdit / ComboBox":

```slint
Text {
    text: @tr("WebSocket URL");
    color: Theme.text-secondary;
    font-size: Theme.font-size-label;
}
LineEdit {
    placeholder-text: "ws://127.0.0.1:9000";
    text <=> Bridge.gstpop-url;
    edited(text) => { root.any-edits-pending = true; }
}
```

This pairing repeats 3× in `media_backend_page.slint` alone.

## Slint docs reference

- [`custom-controls.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/custom-controls.mdx)
  — the recommended pattern for building reusable controls + `@children`.
- [`best-practices.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx)
  — accessibility on custom components (see step
  [05](./05-button-accessibility.md) for the a11y angle).
- [`positioning-and-layouts.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
  — `horizontal-stretch`, `padding-*`, alignment.

## Before — three different page headers, three near-identical bodies

```slint
// ui/pages/media_backend_page.slint:15-32  (current)
Rectangle {
    height: 56px;
    background: Theme.surface-card;
    HorizontalLayout {
        padding: Theme.padding-screen;
        Text {
            text: @tr("Media backend");
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;
        }
        TextButton {
            label: @tr("close-panel-button" => "Done");
            clicked => { Bridge.active-panel = Panel.none; }
        }
    }
}
```

```slint
// ui/pages/macro_edit_page.slint:40-67  (current)
Rectangle {
    height: 56px;
    background: Theme.surface-card;
    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;
        Text {
            text: Bridge.macro-edit-id == "" ? @tr("New macro") : @tr("Edit macro");
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;
        }
        TextButton {
            label: @tr("dismiss-dialog-button" => "Cancel");
            clicked => { Bridge.active-panel = Panel.macros; }
        }
        PrimaryButton {
            label: @tr("save-button" => "Save");
            clicked => {
                Bridge.save-macro(...);
                Bridge.active-panel = Panel.macros;
            }
        }
    }
}
```

## After — `ui/components/panel_chrome.slint`

```slint
// ui/components/panel_chrome.slint
// Panel header, card surface, and form row primitives.
//
// Slint docs ref:
//   draft/slint-ui/docs/astro/src/content/docs/guide/development/custom-controls.mdx

import { Theme } from "../theme.slint";
import { TextButton } from "buttons.slint";

// ── Header ──────────────────────────────────────────────────────────
// Title left, optional trailing actions right via the `actions` slot.
// Default trailing slot renders a single "Done" button that fires
// `close-clicked`; consumer can override via @children for Save+Cancel.
export component PanelHeader inherits Rectangle {
    in property <string> title;
    callback close-clicked();

    height: Theme.header-height;
    background: Theme.surface-card;

    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;

        Text {
            text: root.title;
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;

            accessible-role: text;
            accessible-label: root.title;
        }

        // Default trailing action — "Done" closes the panel.
        // Replaced by consumer-provided @children when present.
        if false-or-only-if-no-children: TextButton {
            label: @tr("close-panel-button" => "Done");
            clicked => { root.close-clicked(); }
        }

        @children
    }
}
```

> **Pitfall:** Slint as of 1.15 does **not** let you conditionally render
> a *default* slot only when `@children` is empty (no "has-children"
> predicate). Two acceptable workarounds:

**Option A** — the consumer always supplies trailing actions; the
component does not render a default.

```slint
// ui/components/panel_chrome.slint
export component PanelHeader inherits Rectangle {
    in property <string> title;
    height: Theme.header-height;
    background: Theme.surface-card;

    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;

        Text {
            text: root.title;
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;
            accessible-role: text;
            accessible-label: root.title;
        }

        @children
    }
}
```

…and consumers spell out the action:

```slint
PanelHeader {
    title: @tr("Media backend");
    TextButton {
        label: @tr("close-panel-button" => "Done");
        clicked => { PanelBridge.pop(); }
    }
}
```

**Option B** — split the header into `PanelHeader` (title + default
"Done") and `PanelHeaderActions` (title + free-form actions) — two
components. Less typing at call-site, slightly more in the library.

```slint
export component PanelHeader inherits Rectangle {
    in property <string> title;
    callback close-clicked();

    height: Theme.header-height;
    background: Theme.surface-card;

    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;
        Text {
            text: root.title;
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;
        }
        TextButton {
            label: @tr("close-panel-button" => "Done");
            clicked => { root.close-clicked(); }
        }
    }
}

export component PanelHeaderActions inherits Rectangle {
    in property <string> title;

    height: Theme.header-height;
    background: Theme.surface-card;

    HorizontalLayout {
        padding: Theme.padding-screen;
        spacing: Theme.spacing-default;
        Text {
            text: root.title;
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
            vertical-alignment: center;
            horizontal-stretch: 1;
        }
        @children
    }
}
```

**Recommendation: Option B**, because 80 % of panels are
title-only-with-Done. Pick A if you want a single component and
explicit call-sites.

## After — `Card` primitive

```slint
// ui/components/panel_chrome.slint (continued)
export component Card inherits Rectangle {
    in property <length> min-card-height: 56px;
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: root.min-card-height;
    clip: true;

    VerticalLayout {
        padding-left:  Theme.padding-screen;
        padding-right: Theme.padding-screen;
        padding-top:    12px;
        padding-bottom: 12px;
        spacing: Theme.spacing-tight;
        @children
    }
}
```

## After — `FormRow` primitive

```slint
// ui/components/panel_chrome.slint (continued)
// Label + control in a vertical pair, separated by 4px.
// Consumer supplies one or more controls as @children.
export component FormRow inherits VerticalLayout {
    in property <string> label;
    spacing: Theme.spacing-tight;

    Text {
        text: root.label;
        color: Theme.text-secondary;
        font-size: Theme.font-size-label;
    }
    @children
}
```

## After — `media_backend_page.slint` rewritten with the primitives

```slint
// ui/pages/media_backend_page.slint (target, abbreviated)
import { ScrollView, LineEdit, ComboBox } from "std-widgets.slint";
import { MediaBackend, PanelBridge, MediaBackendKind, MediaBackendState }
    from "../state/index.slint";
import { Theme } from "../theme.slint";
import { PrimaryButton, DestructiveButton, TextButton } from "../components/buttons.slint";
import { SettingsSection } from "../components/settings_rows.slint";
import { PanelHeader, Card, FormRow } from "../components/panel_chrome.slint";

export component MediaBackendPage inherits Rectangle {
    in-out property <bool> any-edits-pending: false;

    width: 100%; height: 100%;
    background: Theme.surface-primary;

    VerticalLayout {
        PanelHeader {
            title: @tr("Media backend");
            close-clicked => { PanelBridge.pop(); }
        }

        ScrollView {
            mouse-drag-pan-enabled: true;
            VerticalLayout {
                alignment: start;
                spacing: Theme.spacing-default;
                padding: Theme.padding-screen;

                StatusPill { /* extracted in step 06 */ }

                SettingsSection {
                    title: @tr("BACKEND");
                    Card {
                        FormRow {
                            label: @tr("Engine");
                            ComboBox {
                                model: [@tr("Migration (in-process)"),
                                         @tr("gst-pop (WebSocket)")];
                                current-index: MediaBackend.kind == MediaBackendKind.migration ? 0 : 1;
                                selected(value) => { /* see step 09 */ }
                            }
                        }
                    }
                }

                if MediaBackend.kind == MediaBackendKind.gst-pop: SettingsSection {
                    title: @tr("GST-POP DAEMON");

                    Card {
                        FormRow {
                            label: @tr("WebSocket URL");
                            LineEdit {
                                placeholder-text: "ws://127.0.0.1:9000";
                                text <=> MediaBackend.gstpop-url;
                                edited(text) => { root.any-edits-pending = true; }
                            }
                        }
                    }
                    Card {
                        FormRow {
                            label: @tr("API key (optional)");
                            LineEdit {
                                input-type: password;
                                text <=> MediaBackend.gstpop-api-key;
                                edited(text) => { root.any-edits-pending = true; }
                            }
                        }
                    }
                    Card {
                        FormRow {
                            label: @tr("Pipeline id (gst-pop assigns \"0\" first)");
                            LineEdit {
                                placeholder-text: "0";
                                text <=> MediaBackend.gstpop-pipeline-id;
                                edited(text) => { root.any-edits-pending = true; }
                            }
                        }
                    }
                }
            }
        }
        // Footer button row stays — see steps 05/06/11 for cleanups.
    }
}
```

The page drops from **227** lines (current) to roughly **140**, and the
diff hits the conceptual primitive (`Card`, `FormRow`) instead of the
repeated wrapper boilerplate.

## Migration

1. Create `ui/components/panel_chrome.slint` with `PanelHeader`,
   `Card`, `FormRow` exports.
2. Replace each panel's header `Rectangle { height: 56px; … }` with
   `PanelHeader { title: …; close-clicked => { PanelBridge.pop(); } }`.
3. Replace each `Rectangle { background: surface-card; … VerticalLayout { padding-…; … } }`
   with `Card { … }`. Where a non-default min-height was used,
   pass `min-card-height: 200px;` (or whatever) as a prop.
4. Replace each "Text label + control" pair inside a `Card` with
   `FormRow { label: …; <control> }`.
5. Run `slint-viewer` on each migrated page to visually verify no
   regression.

### Per-file checklist

| Page                                  | PanelHeader | Card sites | FormRow sites |
| ------------------------------------- | ----------- | ---------- | ------------- |
| `media_backend_page.slint`            | yes         | 4          | 3             |
| `recording_page.slint`                | yes         | 6+         | 0             |
| `settings_page.slint`                 | yes (FullSettingsPage) | sectioned | n/a |
| `macro_edit_page.slint`               | yes (Actions variant) | 3 | 2 |
| `macros_page.slint`                   | yes         | per-row    | 0             |
| `audio_page.slint`                    | yes         | 3          | 2             |
| `camera_page.slint`                   | yes         | 4          | 2             |
| `network_page.slint`                  | yes         | per-row    | 0             |
| `mixer_page.slint`                    | yes         | 4–6        | 4–6           |
| `backup_reset_page.slint`             | yes         | 4          | 0             |
| `cast_history_*_page.slint`           | yes         | per-row    | 0             |
| `debug_*_page.slint`                  | yes         | 2          | 0             |
| `bitrate_*_page.slint`                | yes         | 2–3        | 1             |
| `quick_actions_page.slint`            | yes         | per-row    | 0             |
| `pairing_page.slint`                  | yes (Actions variant: Cancel) | 1 | 0 |
| `receiver_rename_page.slint`          | yes (Actions variant: Cancel+Save) | 1 | 1 |
| `codec_test_page.slint`               | yes         | 1          | 0             |

## Out of scope

- A `Sheet` / `BottomSheet` variant. Defer until a real use-case
  appears.
- A `Toolbar` / `BottomBar` primitive (the CastControlBar lives in its
  own file already).
- Migrating the connect / casting / connecting pages — they don't have
  a panel header by design.

## Acceptance

- [ ] `git grep -nE 'height: 56px' ui/pages/*.slint` returns at most one
      hit per page (the `PanelHeader` itself); preferably zero hits.
- [ ] `git grep -c 'background: Theme.surface-card' ui/pages/*.slint`
      drops by ≥50 % vs the current 62.
- [ ] Slint viewer renders every migrated page identical to baseline.
- [ ] No Rust changes (this is a pure restructure).
