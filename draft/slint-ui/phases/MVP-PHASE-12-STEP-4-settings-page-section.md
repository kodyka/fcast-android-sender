# MVP-PHASE-12 — Step 4: Settings section + `MediaBackendPage` + routing

> Part 4 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-3](./MVP-PHASE-12-STEP-3-bridge-callbacks.md).
> Next: [STEP-5](./MVP-PHASE-12-STEP-5-rust-trait-and-migration-adapter.md).

---

## 0. Goal of this step

Build the actual UI: one open-row in `FullSettingsPage`, a new
`MediaBackendPage` component in its own file, and the
`Panel.media-backend` route in `ui/main.slint` that opens it.

After this step the user can navigate **Settings → Media backend**,
flip the backend toggle, edit the gst-pop URL / API key / pipeline-id,
and press **Probe** / **Apply** / **Save**. The callbacks fire but
Rust hasn't wired handlers yet — STEP-8 closes that loop.

---

## 1. Open-row in `FullSettingsPage`

Insert one row at the bottom of the **CODEC & DEBUG** section in
`ui/pages/settings_page.slint` (right before the closing `}` of the
section, around `ui/pages/settings_page.slint:210`):

```slint
                // ── Section: CODEC & DEBUG ────────────────────────────────
                SettingsSection {
                    title: @tr("CODEC & DEBUG");
                    // … existing rows …

                    // ⇩ MVP-PHASE-12 — new row
                    SettingsValueRow {
                        title: @tr("Media backend");
                        value: Bridge.media-backend == MediaBackendKind.migration
                            ? @tr("Migration (in-process)")
                            : @tr("gst-pop ({})", Bridge.gstpop-url);
                        clicked => { Bridge.active-panel = Panel.media-backend; }
                    }
                }
```

> **Slint-doc reference for ternary expressions:**
> [`expressions-and-statements.mdx`](../docs/astro/src/content/docs/guide/language/coding/expressions-and-statements.mdx)
> §"Ternary if". The pattern matches the existing
> `["480p", "720p", "1080p", "1440p"][root.resolution-idx]` shortcut
> at `settings_page.slint:168`.

Don't forget to import `MediaBackendKind` at the top of the file:

```slint
import { Bridge, Panel, LifecycleMode, MediaBackendKind } from "../bridge.slint";
```

---

## 2. `MediaBackendPage` (new file)

Create **`ui/pages/media_backend_page.slint`**:

```slint
// MediaBackendPage — Settings → Media backend.
//
// Lets the user choose which media-pipeline engine drives the
// outbound cast: the in-process src/migration/ subsystem or the
// out-of-process gst-pop daemon over WebSocket.
//
// See draft/slint-ui/phases/MVP-PHASE-12-gstpop-backend-toggle.md.

import { ScrollView, LineEdit, ComboBox } from "std-widgets.slint";
import { Bridge, Panel, MediaBackendKind, MediaBackendState } from "../bridge.slint";
import { Theme } from "../theme.slint";
import { PrimaryButton, DestructiveButton, TextButton } from "../components/buttons.slint";
import { SettingsSection } from "../components/settings_rows.slint";

export component MediaBackendPage inherits Rectangle {
    // Did the user edit a field but not press Apply / Save yet?
    in-out property <bool> any-edits-pending: false;

    width:  100%;
    height: 100%;
    background: Theme.surface-primary;

    VerticalLayout {
        // ── Header ─────────────────────────────────────────────────────
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

        // ── Body (scrollable) ─────────────────────────────────────────
        ScrollView {
            mouse-drag-pan-enabled: true;
            VerticalLayout {
                spacing: Theme.spacing-default;
                padding: Theme.padding-screen;

                // ── Status banner ──────────────────────────────────────
                Rectangle {
                    background: Bridge.media-backend-state == MediaBackendState.error
                        ? #4a1414
                        : Theme.surface-card;
                    border-radius: Theme.radius-card;
                    min-height: 56px;
                    HorizontalLayout {
                        padding: Theme.padding-screen;
                        spacing: 8px;
                        // Status dot (12px circle, colour from state)
                        Rectangle {
                            width: 12px;
                            height: 12px;
                            border-radius: 6px;
                            background:
                                Bridge.media-backend-state == MediaBackendState.ready    ? #4ade80 :
                                Bridge.media-backend-state == MediaBackendState.probing  ? #fbbf24 :
                                Bridge.media-backend-state == MediaBackendState.error    ? #f87171 :
                                                                                            #6b7280;
                            y: (parent.height - self.height) / 2;
                        }
                        VerticalLayout {
                            spacing: 2px;
                            horizontal-stretch: 1;
                            Text {
                                text:
                                    Bridge.media-backend-state == MediaBackendState.ready    ? @tr("Ready")    :
                                    Bridge.media-backend-state == MediaBackendState.probing  ? @tr("Probing…") :
                                    Bridge.media-backend-state == MediaBackendState.error    ? @tr("Error")    :
                                                                                                @tr("Disconnected");
                                color: Theme.text-primary;
                                font-size: Theme.font-size-body;
                            }
                            if Bridge.media-backend-status-text != "": Text {
                                text: Bridge.media-backend-status-text;
                                color: Theme.text-secondary;
                                font-size: Theme.font-size-label;
                                wrap: word-wrap;
                            }
                            if Bridge.media-backend-error-text != "": Text {
                                text: Bridge.media-backend-error-text;
                                color: Theme.error-fg;
                                font-size: Theme.font-size-label;
                                wrap: word-wrap;
                            }
                        }
                    }
                }

                // ── Section: Backend choice ────────────────────────────
                SettingsSection {
                    title: @tr("BACKEND");
                    Rectangle {
                        background: Theme.surface-card;
                        border-radius: Theme.radius-card;
                        min-height: 64px;
                        HorizontalLayout {
                            padding: Theme.padding-screen;
                            spacing: 12px;
                            Text {
                                text: @tr("Engine");
                                color: Theme.text-primary;
                                font-size: Theme.font-size-body;
                                vertical-alignment: center;
                                horizontal-stretch: 1;
                            }
                            ComboBox {
                                model: [@tr("Migration (in-process)"),
                                        @tr("gst-pop (WebSocket)")];
                                // ComboBox.current-index is `int`,
                                // Bridge.media-backend is an enum.
                                // Translate by hand on both sides.
                                current-index: Bridge.media-backend == MediaBackendKind.migration ? 0 : 1;
                                selected(idx) => {
                                    Bridge.media-backend = idx == 0
                                        ? MediaBackendKind.migration
                                        : MediaBackendKind.gst-pop;
                                    root.any-edits-pending = true;
                                }
                            }
                        }
                    }
                }

                // ── Section: gst-pop daemon (only if gst-pop selected) ─
                if Bridge.media-backend == MediaBackendKind.gst-pop: SettingsSection {
                    title: @tr("GST-POP DAEMON");

                    // WebSocket URL
                    Rectangle {
                        background: Theme.surface-card;
                        border-radius: Theme.radius-card;
                        min-height: 88px;
                        VerticalLayout {
                            padding-left:   Theme.padding-screen;
                            padding-right:  Theme.padding-screen;
                            padding-top:    12px;
                            padding-bottom: 12px;
                            spacing: 4px;
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
                        }
                    }

                    // API key (masked)
                    Rectangle {
                        background: Theme.surface-card;
                        border-radius: Theme.radius-card;
                        min-height: 88px;
                        VerticalLayout {
                            padding-left:   Theme.padding-screen;
                            padding-right:  Theme.padding-screen;
                            padding-top:    12px;
                            padding-bottom: 12px;
                            spacing: 4px;
                            Text {
                                text: @tr("API key (optional)");
                                color: Theme.text-secondary;
                                font-size: Theme.font-size-label;
                            }
                            LineEdit {
                                placeholder-text: @tr("Leave empty if --api-key was not set on the daemon");
                                input-type: password;
                                text <=> Bridge.gstpop-api-key;
                                edited(text) => { root.any-edits-pending = true; }
                            }
                        }
                    }

                    // Pipeline id
                    Rectangle {
                        background: Theme.surface-card;
                        border-radius: Theme.radius-card;
                        min-height: 88px;
                        VerticalLayout {
                            padding-left:   Theme.padding-screen;
                            padding-right:  Theme.padding-screen;
                            padding-top:    12px;
                            padding-bottom: 12px;
                            spacing: 4px;
                            Text {
                                text: @tr("Pipeline id (gst-pop assigns \"0\" first)");
                                color: Theme.text-secondary;
                                font-size: Theme.font-size-label;
                            }
                            LineEdit {
                                placeholder-text: "0";
                                text <=> Bridge.gstpop-pipeline-id;
                                edited(text) => { root.any-edits-pending = true; }
                            }
                        }
                    }
                }
            }
        }

        // ── Footer (sticky action bar) ────────────────────────────────
        Rectangle {
            height: 96px;
            background: Theme.surface-card;
            HorizontalLayout {
                padding: Theme.padding-screen;
                spacing: 8px;

                DestructiveButton {
                    label: @tr("Probe");
                    clicked => {
                        Bridge.probe-media-backend();
                    }
                }

                Rectangle { horizontal-stretch: 1; }

                TextButton {
                    label: root.any-edits-pending ? @tr("Save") : @tr("Saved");
                    clicked => {
                        Bridge.save-media-backend-settings();
                        root.any-edits-pending = false;
                    }
                }

                PrimaryButton {
                    label: @tr("Apply");
                    clicked => {
                        Bridge.apply-media-backend();
                        root.any-edits-pending = false;
                    }
                }
            }
        }
    }
}
```

### 2.1 Why a `ComboBox`, not a `Switch`

The choice is intrinsically *two values*, which would be a perfect
`Switch` use case. But the readability cost of "ON means gst-pop, OFF
means migration" — and the future-proofing for a third backend (e.g.
"Native iOS pipeline" or "Browser MediaStream") — argues for a
`ComboBox` from the start. The std-widgets `ComboBox` model is a list
of strings (`ui/components/std/combobox.slint:11`), and the
`current-index` ↔ enum translation is two lines on each side (see
`ui/components/std/combobox.slint:10` for the `model: [string]`
declaration).

### 2.2 Why a `password` input-type for the API key

`LineEdit.input-type: password` is part of the standard widget API
(`ui/components/std/lineedit.slint:9`). It masks the field's contents
visually but does *not* prevent log lines from showing the value — the
backend lifecycle (STEP-8) must never log the API key.

### 2.3 Why three buttons (Probe, Save, Apply) — not one Apply

- **Probe** validates the URL + API key without changing the active
  backend. The user uses this to confirm "yes, the daemon at
  ws://my-relay:9000 is reachable" before they commit.
- **Save** persists the current values to disk so they survive an app
  restart, **without** swapping the live backend. The user uses this
  to stash a configuration for later.
- **Apply** does both: persists, swaps the live selector, then probes
  the new selection. The user uses this to commit.

The trade-off vs. PHASE-7's "edit-and-it's-live" model is intentional
— flipping the backend in the middle of a cast is destructive, so we
require an explicit Apply.

---

## 3. Route the panel in `ui/main.slint`

Add to `ui/main.slint` (next to the other panel routes around lines
107-135):

```slint
import { MediaBackendPage } from "pages/media_backend_page.slint";
// … other imports …

// inside MainWindow:
if Bridge.active-panel == Panel.media-backend: MediaBackendPage {
    width:  parent.width;
    height: parent.height;
}
```

> **Slint-doc reference for conditional rendering:**
> [`positioning-and-layouts.mdx §"Conditional elements"`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx).
> The pattern matches every other `if Bridge.active-panel == Panel.X`
> branch.

---

## 4. Expected diff size

- `ui/pages/settings_page.slint`: +8 lines (one new row + import).
- `ui/pages/media_backend_page.slint`: +260 lines (new file).
- `ui/main.slint`: +5 lines (one import, one route).

---

## 5. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build

# Open the app, tap Settings → Media backend, confirm:
#   - Toggle flips between Migration and gst-pop.
#   - When gst-pop selected, the daemon section appears.
#   - Pressing Apply / Probe / Save logs:
#       "warning: callback apply-media-backend has no handler"
#     (until STEP-8 lands).
```

---

## 6. Exit gate

- [ ] `FullSettingsPage` has the new "Media backend" row.
- [ ] `MediaBackendPage` renders the status banner, the engine
      combobox, and (when gst-pop selected) the three daemon fields.
- [ ] The three footer buttons fire the three callbacks declared in
      STEP-3.
- [ ] `Panel.media-backend` routes to `MediaBackendPage` in
      `ui/main.slint`.
- [ ] `cargo build` and `ci/ui-validate.sh` both pass.

Proceed to [STEP-5](./MVP-PHASE-12-STEP-5-rust-trait-and-migration-adapter.md).
