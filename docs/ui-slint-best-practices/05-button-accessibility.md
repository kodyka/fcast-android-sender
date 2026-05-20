# 05 — Buttons: accessibility & focus semantics

## Goal

Every custom button (`PrimaryButton`, `TextButton`, `DestructiveButton`,
`QuickActionButton`, `MenuRow`, `Badge`) declares `accessible-role:
button`, an `accessible-label`, an `accessible-action-default`, and
participates in keyboard / TalkBack focus. The `TouchArea` driving them
forwards `enabled` correctly so disabled buttons don't accept gestures.

This is the single highest-leverage hardening change the UI can do —
Slint already supports it, the cost is one or two lines per button, and
the result is TalkBack/AccessibilityInsights compatibility on day one.

## Findings

`grep -rn 'accessible-role\|accessible-label' ui/components/*.slint ui/pages/*.slint`
→ **0 hits**. The vendored `ui/components/std/*.slint` widgets do
declare a11y (e.g. `ui/components/std/button.slint`), but **none** of
the FCast-authored buttons or rows do.

Affected components:

- `ui/components/buttons.slint` — `PrimaryButton`, `TextButton`,
  `DestructiveButton`, `LoadingView`.
- `ui/components/control_bar.slint` — `QuickActionButton`.
- `ui/components/receiver_context_menu.slint` — `MenuRow`.
- `ui/components/status_badges.slint` — `Badge` (read-only, but should
  still expose `accessible-role: text`).
- `ui/components/settings_rows.slint` — `SettingsValueRow`,
  `SettingsToggleRow`.

## Slint docs reference

- [Best Practices — Accessibility](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx#accessibility):

  > *When designing custom components, consider early on to declare
  > accessibility properties. At least a role, possibly a label, as well
  > as actions.*

- `ui/components/std/button.slint:160–180` — the vendored Slint stdlib
  button — shows the canonical wiring (paraphrased below) used by
  upstream:

  ```slint
  accessible-role: button;
  accessible-label: root.text;
  accessible-action-default => { root.clicked(); }
  ```

- AccessibleProperties reference page on the Slint docs site (the
  vendored docs index lists `Link type="AccessibleProperties"` in
  `best-practices.mdx`).

## Before — `ui/components/buttons.slint:9–31` (PrimaryButton)

```slint
export component PrimaryButton inherits Rectangle {
    in property <string>  label;
    in property <bool>    enabled: true;
    callback clicked();

    height: Theme.row-height;
    border-radius: Theme.radius-card;
    background: ta.pressed ? Theme.accent-pressed : Theme.accent;
    opacity: root.enabled ? 1.0 : 0.45;

    ta := TouchArea {
        enabled: root.enabled;
        clicked => { root.clicked(); }
    }
    Text {
        text: root.label;
        color: Theme.text-primary;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: Theme.font-size-body;
        font-weight: 600;
    }
}
```

Problems:

1. No `accessible-role` → screen readers see it as a plain
   `Rectangle`.
2. No `accessible-label` → screen readers announce nothing.
3. No `accessible-action-default` → keyboard/AT can't activate the
   button. AT users on Android (TalkBack double-tap, Switch Access) get
   nothing.
4. Visual disabled state is `opacity: 0.45` — but no
   `accessible-enabled` is forwarded, so AT continues to read it as
   "available".

## After — `ui/components/buttons.slint` (target shape)

```slint
import { Theme } from "../theme.slint";
import { Spinner } from "std-widgets.slint";

export component PrimaryButton inherits Rectangle {
    in property <string>  label;
    in property <bool>    enabled: true;
    callback clicked();

    height: Theme.row-height;
    border-radius: Theme.radius-card;
    background: ta.pressed ? Theme.accent-pressed : Theme.accent;
    opacity: root.enabled ? 1.0 : 0.45;

    // ── Accessibility ───────────────────────────────────────────
    accessible-role:          button;
    accessible-label:         root.label;
    accessible-enabled:       root.enabled;
    accessible-action-default => {
        if root.enabled { root.clicked(); }
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

export component TextButton inherits Rectangle {
    in property <string> label;
    in property <bool>   enabled: true;
    callback clicked();

    height: Theme.row-height;
    background: transparent;
    opacity: root.enabled ? 1.0 : 0.45;

    accessible-role:          button;
    accessible-label:         root.label;
    accessible-enabled:       root.enabled;
    accessible-action-default => {
        if root.enabled { root.clicked(); }
    }

    ta := TouchArea {
        enabled: root.enabled;
        clicked => { root.clicked(); }
    }
    Text {
        text: root.label;
        color: Theme.accent;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: Theme.font-size-body;
    }
}

export component DestructiveButton inherits Rectangle {
    in property <string> label;
    in property <bool>   enabled: true;
    callback clicked();

    height: Theme.row-height;
    border-radius: Theme.radius-card;
    background: ta.pressed ? Theme.error.darker(20%) : Theme.error;
    opacity: root.enabled ? 1.0 : 0.45;

    accessible-role:          button;
    // Convey *intent* to AT users, not just the label.
    accessible-label:         @tr("destructive-button-a11y-label", "{} (destructive)", root.label);
    accessible-enabled:       root.enabled;
    accessible-action-default => {
        if root.enabled { root.clicked(); }
    }

    ta := TouchArea {
        enabled: root.enabled;
        clicked => { root.clicked(); }
    }
    Text {
        text: root.label;
        color: Theme.text-on-error;
        horizontal-alignment: center;
        vertical-alignment: center;
    }
}
```

> The `accessible-action-default` body guards on `root.enabled` to
> match the visual `opacity` cue — AT users hitting a disabled button
> should not trigger the callback even though the role is exposed.

## After — `MenuRow` in `receiver_context_menu.slint`

```slint
component MenuRow inherits Rectangle {
    in property <string> label;
    in property <color>  label-color: Theme.text-primary;
    callback clicked;

    height: 44px;
    background: transparent;

    accessible-role:           button;
    accessible-label:          root.label;
    accessible-action-default => { root.clicked(); }

    HorizontalLayout {
        padding-left:  16px;
        padding-right: 16px;
        alignment: start;
        Text {
            text: root.label;
            color: root.label-color;
            font-size: Theme.font-size-body;
            vertical-alignment: center;
        }
    }

    TouchArea {
        clicked => { root.clicked(); }
    }
}
```

## After — `QuickActionButton` in `control_bar.slint`

```slint
export component QuickActionButton inherits Rectangle {
    in property <QuickAction> action;
    callback invoked(string);

    height: 48px;
    width: 80px;
    border-radius: Theme.radius-card;
    background: root.action.active
        ? Theme.accent-active
        : (ta.pressed ? Theme.surface-card.brighter(20%) : Theme.surface-card);
    opacity: root.action.enabled ? 1.0 : 0.45;

    accessible-role:    button;
    accessible-label:   root.action.is-macro
        ? @tr("macro-quick-action-a11y", "Run macro: {}", root.action.title)
        : root.action.title;
    accessible-enabled: root.action.enabled;
    accessible-checked: root.action.active;    // toggle-style buttons
    accessible-action-default => {
        if root.action.enabled { root.invoked(root.action.id); }
    }

    ta := TouchArea {
        enabled: root.action.enabled;
        clicked => { root.invoked(root.action.id); }
    }
    Text {
        text: root.action.is-macro ? "▶ " + root.action.title : root.action.title;
        color: Theme.text-primary;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: Theme.font-size-label;
        wrap: word-wrap;
    }
}
```

`accessible-checked` is meaningful for the `active` quick actions
(record-while-recording lights up). Slint exposes it to the platform AT
layer as `selected`/`checked` depending on the role.

## After — `SettingsToggleRow` in `settings_rows.slint`

```slint
export component SettingsToggleRow inherits Rectangle {
    in property <string> title;
    in-out property <bool> checked: false;
    in property <bool> enabled: true;
    callback toggled(bool);

    height: Theme.row-height;
    opacity: root.enabled ? 1.0 : 0.45;

    accessible-role:    switch;
    accessible-label:   root.title;
    accessible-enabled: root.enabled;
    accessible-checked: root.checked;
    accessible-action-default => {
        if root.enabled {
            root.checked = !root.checked;
            root.toggled(root.checked);
        }
    }

    HorizontalLayout {
        padding-left:  Theme.padding-screen;
        padding-right: Theme.padding-screen;
        Text {
            text: root.title;
            color: Theme.text-primary;
            vertical-alignment: center;
            horizontal-stretch: 1;
        }
        CheckBox {
            checked <=> root.checked;
            enabled: root.enabled;
            toggled() => { root.toggled(self.checked); }
        }
    }
}
```

> The `accessible-role: switch` is preferred over `button` when the
> control carries on/off state; AT users hear "switch, on" / "switch,
> off" instead of "button". Slint maps `switch` to
> `AccessibleRole::Switch` for AccessKit, which Android's TalkBack
> understands.

## Static text & decorative elements

For purely informational elements that are still important to AT (badges
in `status_badges.slint`, status pill in `media_backend_page.slint`),
add `accessible-role: text;` + `accessible-label`. This lets users swipe
through them with focus navigation without making them activatable.

```slint
component Badge inherits Rectangle {
    in property <string> icon-glyph;
    in property <string> value;
    in property <color>  fg: Theme.text-secondary;

    accessible-role:  text;
    accessible-label: root.icon-glyph + " " + root.value;

    // … rest unchanged …
}
```

## Migration

1. Add `accessible-*` properties to **every** custom button in
   `ui/components/buttons.slint`.
2. Repeat for `MenuRow`, `QuickActionButton`, `SettingsToggleRow`,
   `SettingsValueRow`, `Badge`, `IconAndText` (role: text).
3. For controls with a `toggled` callback, choose `accessible-role:
   switch` (`SettingsToggleRow`) or `checkbox` (`CheckBox`-wrapping
   rows) per the AccessibleProperties table.
4. Verify by running the app under TalkBack and swiping through the
   pages — every actionable element should be focusable, announce a
   useful label, and activate via double-tap.
5. On desktop, run Accessibility Insights (Windows) or Accessibility
   Inspector (macOS) against `slint-viewer ui/main.slint` to lint.

### Per-file checklist

| File                                       | Components changed                                  | New a11y properties added                  |
| ------------------------------------------ | --------------------------------------------------- | ------------------------------------------ |
| `ui/components/buttons.slint`              | `PrimaryButton`, `TextButton`, `DestructiveButton`  | `role`, `label`, `enabled`, `action-default` |
| `ui/components/control_bar.slint`          | `QuickActionButton`                                  | `role`, `label`, `enabled`, `checked`, `action-default` |
| `ui/components/receiver_context_menu.slint`| `MenuRow`                                            | `role`, `label`, `action-default`          |
| `ui/components/settings_rows.slint`        | `SettingsValueRow`, `SettingsToggleRow`              | `role` (button / switch), `label`, `checked`, `enabled`, `action-default` |
| `ui/components/status_badges.slint`        | `Badge`                                              | `role: text`, `label`                       |
| `ui/components/icon_and_text.slint`        | `IconAndText`                                        | `role: text`, `label` (= label prop)        |
| `ui/components/info_banner.slint`          | `InfoBanner`                                         | `role: alert`, `label`                      |
| `ui/components/lock_overlay.slint`         | `LockOverlay`                                        | `role: dialog`, `label`                     |
| `ui/components/confirm_dialog.slint`       | `ConfirmDialog`                                      | `role: dialog`, `label`                     |
| `ui/components/snapshot_countdown.slint`   | `SnapshotCountdown`                                  | `role: dialog`, `label`                     |
| `ui/components/std/*`                      | **no change** (vendored)                            | n/a                                         |

## Out of scope

- Custom focus rings / outlines. The vendored stdlib buttons already
  draw a focus ring; the FCast buttons can adopt the same pattern in a
  follow-up.
- AccessKit-specific announcements (custom `accessible-description`,
  `accessible-placeholder-text`). Add ad-hoc if needed.
- Localised AT-only strings beyond the destructive-warning override
  above. Treat each new `accessible-label` as a normal `@tr(...)`.

## Acceptance

- [ ] `git grep -c 'accessible-role:' ui/components/*.slint ui/pages/*.slint`
      ≥ 12 (one per actionable component above + ConfirmDialog/LockOverlay).
- [ ] TalkBack swipe walks every action in `ConnectView`,
      `SettingsPageView`, `MediaBackendPage` end-to-end.
- [ ] AccessibilityInsights run on `slint-viewer ui/main.slint` reports
      no missing-name / missing-role errors on FCast components.
- [ ] No visual regressions (a11y properties are non-rendering, so
      verify by snapshot diff).
