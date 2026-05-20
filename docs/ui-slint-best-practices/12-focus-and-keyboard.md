# 12 — Focus & keyboard: `FocusScope`, `forward-focus`, and the back-key contract

## Goal

Every overlay panel hosts a local `FocusScope` so the back-key /
keyboard navigation works within the panel and doesn't fight the
global scope. Form pages declare a `forward-focus` so the right input
gets focus when the panel opens. Custom buttons participate in tab
navigation.

## Findings

### F15 — single `FocusScope` in the whole tree

`grep -rn 'FocusScope' ui/` (excluding `std/`):

```text
ui/main.slint:103:    back-key-scope := FocusScope {
```

That's it. All 22 overlay panels, all dialogs, all the lock/snapshot
overlays piggy-back on the top-level scope. Consequences:

- A panel that wants to intercept the back-key (e.g. "Save dialog
  open — back closes the dialog, not the panel") cannot. The top-level
  scope sees the event first and pops the panel.
- Keyboard tab navigation through form inputs (rare on Android, real
  on the desktop debug build) isn't scoped — `Tab` from a `LineEdit`
  inside `MediaBackendPage` can land outside the panel.
- AT users on hardware-keyboard-equipped devices cannot trap focus.

### `forward-focus` is unused

`grep -rn 'forward-focus' ui/` → no hits. When `MediaBackendPage`
opens, the user has to tap the `LineEdit` to focus it — keyboard users
can't start typing immediately.

### Custom buttons don't declare `focus-policy`

`PrimaryButton`, `TextButton`, `DestructiveButton`, `QuickActionButton`
are all `Rectangle` with a `TouchArea` — they have no notion of focus.
On Android touch this isn't visible, but on Switch Access /
hardware-keyboard configurations they are unreachable.

## Slint docs reference

- [`focus.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/focus.mdx)
  — `FocusScope { … }`, `forward-focus`, `focus-policy`, the
  `key-pressed` / `key-released` callbacks.
- [`best-practices.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx#accessibility)
  — accessibility includes keyboard reachability.

## Before — `main.slint:103–118`

```slint
back-key-scope := FocusScope {
    key-pressed(event) => {
        if (event.text == Key.Escape) {
            if (Bridge.active-panel != Panel.none) {
                Bridge.active-panel = Panel.none;
                return accept;
            }
            return reject;
        }
        return reject;
    }
}
```

## After — per-panel scopes with a fallback handler at the top

```slint
// ui/main.slint  (target)
back-key-scope := FocusScope {
    key-pressed(event) => {
        // Last-resort handler. Per-panel scopes get the event first;
        // if none of them consume it, fall through to the global pop.
        if event.text == Key.Escape {
            if PanelBridge.active != Panel.none {
                PanelBridge.pop();
                return accept;
            }
            return reject;
        }
        return reject;
    }
}
```

(The body changes by replacing `Bridge.active-panel = Panel.none` with
`PanelBridge.pop()` — see step 11.)

### Per-panel `FocusScope`

```slint
// ui/pages/media_backend_page.slint  (target)
export component MediaBackendPage inherits Rectangle {
    in-out property <bool> any-edits-pending: false;

    // Take focus the moment we mount, and route back-key locally.
    forward-focus: panel-scope;

    panel-scope := FocusScope {
        key-pressed(event) => {
            if event.text == Key.Escape {
                if root.any-edits-pending {
                    // Open the unsaved-changes confirm dialog instead
                    // of closing the panel.
                    root.show-discard-confirm = true;
                    return accept;
                }
                PanelBridge.pop();
                return accept;
            }
            return reject;
        }

        VerticalLayout {
            // … the existing page body, now inside the FocusScope …
        }
    }
}
```

> **Why nest the layout inside the FocusScope rather than alongside it?**
> A `FocusScope` only receives key events when **focused** (or one of
> its children is). Wrapping the visible content guarantees the scope
> is the focus target whenever the user interacts with the panel.

### Pages with form input get `forward-focus`

```slint
// ui/pages/macro_edit_page.slint  (target)
export component MacroEditPage inherits Rectangle {
    forward-focus: name-edit;     // focus the macro-name LineEdit on open

    panel-scope := FocusScope {
        // …
        VerticalLayout {
            // …
            name-edit := LineEdit { /* … */ }
        }
    }
}
```

```slint
// ui/pages/receiver_rename_page.slint
export component ReceiverRenamePage inherits Rectangle {
    forward-focus: name-edit;
    panel-scope := FocusScope { /* … */ name-edit := LineEdit { /* … */ } }
}
```

```slint
// ui/pages/bitrate_preset_edit_page.slint
export component BitratePresetEditPage inherits Rectangle {
    forward-focus: bitrate-edit;
    // …
}
```

### Buttons participate in focus

```slint
// ui/components/buttons.slint
export component PrimaryButton inherits Rectangle {
    in property <string> label;
    in property <bool>   enabled: true;
    callback clicked();

    height: Theme.row-height;
    border-radius: Theme.radius-card;
    background:
        scope.has-focus
            ? Theme.accent-active
        : ta.pressed
            ? Theme.accent-pressed
        : Theme.accent;

    accessible-role:           button;
    accessible-label:          root.label;
    accessible-enabled:        root.enabled;
    accessible-action-default => { if root.enabled { root.clicked(); } }

    // FocusScope makes the button keyboard-focusable. The space/enter
    // key fires the click handler.
    scope := FocusScope {
        enabled: root.enabled;
        key-pressed(event) => {
            if event.text == "\n" || event.text == " " {
                root.clicked();
                return accept;
            }
            return reject;
        }
    }

    ta := TouchArea {
        enabled: root.enabled;
        clicked => { root.clicked(); }
    }

    Text {
        text: root.label;
        color: Theme.text-on-accent;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: Theme.font-size-body;
        font-weight: 600;
    }
}
```

Apply the same FocusScope wrap to `TextButton`, `DestructiveButton`,
`QuickActionButton`, `MenuRow`, `SettingsToggleRow`. Visually, draw a
focus ring (a 2 px border in `Theme.accent-active`) when `has-focus`:

```slint
border-width:  scope.has-focus ? 2px : 0px;
border-color:  Theme.accent-active;
animate border-width { duration: 80ms; }
```

### Tab order via `tab-stop: true` (Slint 1.16+)

If you're on 1.16 or later, declare `tab-stop: true` on each
keyboard-reachable element. On 1.15.1, the order follows source
order of `FocusScope` declarations — keep that in mind when laying out
form pages.

## Modal trap inside `ConfirmDialog`

`ConfirmDialog` should trap focus so the dialog is not back-keyed away
without user input:

```slint
// ui/components/confirm_dialog.slint  (target)
export component ConfirmDialog inherits Rectangle {
    in property <string> title: @tr("Confirm");
    in property <string> body:  @tr("Are you sure?");
    in property <string> confirm-label: @tr("OK");
    in property <bool>   destructive: false;
    callback confirmed();
    callback dismissed();

    background: Theme.scrim-strong;

    // Trap focus while the dialog is up.
    forward-focus: dialog-scope;
    dialog-scope := FocusScope {
        key-pressed(event) => {
            if event.text == Key.Escape {
                root.dismissed();
                return accept;
            }
            if event.text == "\n" {
                root.confirmed();
                return accept;
            }
            return reject;
        }
        // … card and buttons …
    }

    // Scrim tap dismisses
    TouchArea {
        clicked => { root.dismissed(); }
    }
}
```

The same pattern applies to `ReceiverContextMenu` and `SnapshotCountdown`.

## What `forward-focus` actually does

`forward-focus` on a component delegates focus to a named child. When
the parent receives `focus()`, it routes to that child instead of
keeping it itself. The Slint runtime auto-calls `focus()` on a
newly-mounted element if it's reachable via the focus chain — which is
how "panel opens, LineEdit is focused" works without explicit Rust
help.

## Slint docs reference

- `forward-focus` is in
  [`focus.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/focus.mdx).
- `accessible-role` interaction with focus: per the AccessibleProperties
  reference, role-`button` elements with no `TouchArea` are still
  focus-reachable if they declare a `FocusScope`.

## Migration

1. Decide on the focus contract for each panel:
   - **Form panels** (MediaBackend, MacroEdit, ReceiverRename,
     BitratePresetEdit, AudioPage, CameraPage, NetworkPage,
     MixerPage settings) — `forward-focus` to the first input.
   - **List panels** (CastHistory, Macros, QuickActions,
     BitratePresets) — `forward-focus` to the ScrollView or its first
     row's TouchArea.
   - **Modal overlays** (ConfirmDialog, ReceiverContextMenu,
     SnapshotCountdown, LockOverlay) — trap focus until dismissed.
2. Wrap each panel's `VerticalLayout` in a local `FocusScope`.
3. Move "intercept back" logic from the global scope into the panel
   scope where the panel has unsaved-changes semantics.
4. Add the focus-ring styling to the four custom buttons in
   `ui/components/buttons.slint`.
5. Run with a USB keyboard attached on a debug device — verify Tab
   walks every actionable element, Enter activates, Esc backs.

### Per-file checklist

| File                                       | Add                                              |
| ------------------------------------------ | ------------------------------------------------ |
| `ui/main.slint`                            | Body of `back-key-scope` calls `PanelBridge.pop()` |
| `ui/pages/media_backend_page.slint`        | `forward-focus`, `FocusScope { … }`, unsaved-changes back-key intercept |
| `ui/pages/macro_edit_page.slint`           | `forward-focus: name-edit`, `FocusScope`, same intercept |
| `ui/pages/receiver_rename_page.slint`      | `forward-focus: name-edit`, `FocusScope`         |
| `ui/pages/bitrate_preset_edit_page.slint`  | `forward-focus: bitrate-edit`, `FocusScope`       |
| `ui/pages/audio_page.slint`, `camera_page.slint`, `network_page.slint`, `mixer_page.slint` | `FocusScope`, focus-policy on each ScrollView |
| `ui/components/buttons.slint`              | `FocusScope` wrap on each button + focus ring     |
| `ui/components/confirm_dialog.slint`       | `FocusScope` trapping back-key                    |
| `ui/components/receiver_context_menu.slint`| `FocusScope` trapping back-key                    |
| `ui/components/lock_overlay.slint`         | `FocusScope` so back-key doesn't exit lock        |
| `ui/components/snapshot_countdown.slint`   | `FocusScope` so back-key cancels countdown        |

## Out of scope

- Roving-tab-index for the QuickActions for-loop. The simpler
  approach is to just let each `QuickActionButton` be its own focus
  scope; revisit only if focus performance is a problem.
- A bespoke "first focusable in this subtree" auto-finder. Slint's
  `forward-focus` is per-element, and that's sufficient for the
  current panel set.

## Acceptance

- [ ] `grep -rn 'FocusScope' ui/components ui/pages | wc -l` ≥ 12 (the
      modal overlays + the form pages + each custom button + the
      global scope).
- [ ] Plugging a USB keyboard into a debug build, Tab cycles through
      every actionable element on every panel in source order; Enter
      activates; Esc backs.
- [ ] TalkBack focus order is identical to keyboard tab order — verify
      on the Connect, Settings, Media-backend, and Macro-edit pages.
