# 01 — Theme tokens: remove inline literals

## Goal

Every color, length, and radius referenced from FCast Slint code should
come from `Theme.*`. No `#000`, no `48px`, no `#00000080` — exactly the
"design tokens are a global singleton" pattern that the Slint docs spell
out for `Palette`.

## Findings

`grep -rn 'background: #[a-fA-F0-9]\{3,8\}' ui/components/*.slint ui/pages/*.slint`:

```text
ui/components/confirm_dialog.slint:37:    background: #00000080;   // 50% opaque scrim — TODO Theme.scrim later
ui/components/qr_placeholder.slint:19:    background: #000;
ui/components/qr_placeholder.slint:26:        background: #fff;
ui/components/qr_placeholder.slint:33:        background: #000;
ui/components/qr_placeholder.slint:44:    background: #fff;
ui/components/qr_placeholder.slint:64:    Rectangle { x: 70px; … background: #000; }
ui/components/qr_placeholder.slint:65–68: …
ui/pages/recording_page.slint:118:    background: Bridge.recording-state == RecordingState.idle ? #cc0000 : …
ui/components/receiver_context_menu.slint:53:    drop-shadow-color: #00000040;
```

`grep -rn 'font-size: [0-9]' ui/components/*.slint ui/pages/*.slint`:

```text
ui/components/lock_overlay.slint:48:    font-size: 48px;        // 🔒 glyph
ui/components/snapshot_countdown.slint:48: font-size: 72px;     // countdown number
ui/pages/cast_history_page.slint:156:    font-size: 20px;       // big duration cell
```

## Slint docs reference

- Globals: [`globals.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/globals.mdx)
- Best practices — "Separate Code, UI, and Assets" (assets is what
  tokens are): [`best-practices.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx#separate-code-ui-and-assets)

## Before — `ui/theme.slint` today

```slint
// ui/theme.slint (current)
export global Theme {
    out property <color> surface-primary:  #0b1020;
    out property <color> surface-card:     #222633;
    out property <color> surface-bar:      #111827;
    out property <color> surface-overlay:  #1f2937cc;
    // …
    out property <length> font-size-heading:  20px;
    out property <length> font-size-body:     16px;
    out property <length> font-size-label:    12px;
    out property <length> padding-screen:   12px;
    out property <length> padding-card:      8px;
    out property <length> spacing-default:   8px;
    out property <length> radius-card:       8px;
    out property <length> radius-pill:        6px;
    out property <length> row-height:        48px;
    out property <length> control-bar-height: 72px;
}
```

## After — extend `Theme` with scrim, elevation, type-scale, recording red

```slint
// ui/theme.slint (target)
export global Theme {
    // ── Surfaces ────────────────────────────────────────────────────
    out property <color> surface-primary:  #0b1020;
    out property <color> surface-card:     #222633;
    out property <color> surface-bar:      #111827;
    out property <color> surface-overlay:  #1f2937cc;
    // NEW: scrim used by dialogs / popovers / context menus.
    out property <color> scrim-strong:     #00000080;  // 50 % black — modal blocking
    out property <color> scrim-light:      #00000040;  // 25 % black — drop-shadow / soft

    // ── Text ────────────────────────────────────────────────────────
    out property <color> text-primary:     #ffffff;
    out property <color> text-secondary:   #b8becd;
    out property <color> text-disabled:    #6b7280;
    // NEW: on-image / overlay text that must read on coloured surfaces.
    out property <color> text-on-accent:   #ffffff;
    out property <color> text-on-error:    #ffffff;

    // ── Accent / interactive ────────────────────────────────────────
    out property <color> accent:           #4682b4;
    out property <color> accent-muted:     #b0c4de;
    out property <color> accent-active:    #2563eb;
    out property <color> accent-pressed:   #1e40af;

    // ── Severity ────────────────────────────────────────────────────
    out property <color> error:            #c62828;
    out property <color> error-fg:         #ef4444;
    out property <color> warning:          #ed6c02;
    out property <color> warning-fg:       #ed6c02;
    out property <color> success:          #2e7d32;
    // NEW: paired token for the record button. Avoids `#cc0000` inline.
    out property <color> recording-dot:    #cc0000;

    // ── Typography — extended scale ─────────────────────────────────
    out property <length> font-size-label:    12px;
    out property <length> font-size-body:     16px;
    out property <length> font-size-heading:  20px;
    // NEW: bigger sizes used by overlays. Replace inline 48px / 72px.
    out property <length> font-size-display:  48px;  // lock icon
    out property <length> font-size-hero:     72px;  // countdown number
    out property <length> font-size-cell:     20px;  // history cell duration

    // ── Spacing ─────────────────────────────────────────────────────
    out property <length> padding-screen:   12px;
    out property <length> padding-card:      8px;
    out property <length> spacing-default:   8px;
    // NEW: tighter spacing for chips/pills and group headers.
    out property <length> spacing-tight:     4px;
    out property <length> spacing-loose:    16px;

    // ── Shape ───────────────────────────────────────────────────────
    out property <length> radius-card:       8px;
    out property <length> radius-pill:        6px;
    // NEW: full-pill helper. Use as `border-radius: self.height / 2;` if
    // you need it to track height; this constant is for *square* pills.
    out property <length> radius-circle:    9999px;
    out property <length> row-height:        48px;
    out property <length> control-bar-height: 72px;
    // NEW: standard panel header height — used by every overlay.
    out property <length> header-height:    56px;

    // ── Elevation ───────────────────────────────────────────────────
    // Drop-shadow tokens. Slint expresses elevation per-element with
    // `drop-shadow-blur` + `drop-shadow-color`; surface these so the
    // four sites that use them stop hand-rolling values.
    out property <length> elevation-1-blur:    4px;
    out property <length> elevation-2-blur:    8px;
    out property <length> elevation-3-blur:   16px;
}
```

## After — call-sites that now use tokens

`ui/components/confirm_dialog.slint:37`:

```slint
// Before
background: #00000080;   // 50% opaque scrim — TODO Theme.scrim later

// After
background: Theme.scrim-strong;
```

`ui/components/receiver_context_menu.slint:52–53`:

```slint
// Before
drop-shadow-blur: 8px;
drop-shadow-color: #00000040;

// After
drop-shadow-blur:  Theme.elevation-2-blur;
drop-shadow-color: Theme.scrim-light;
```

`ui/components/lock_overlay.slint:46–49`:

```slint
// Before
Text {
    text: "🔒";
    font-size: 48px;
    horizontal-alignment: center;
}

// After
Text {
    text: "🔒";
    font-size: Theme.font-size-display;
    horizontal-alignment: center;
}
```

`ui/components/snapshot_countdown.slint:45–51`:

```slint
// Before
Text {
    text: root.remaining;
    color: Theme.text-primary;
    font-size: 72px;
    font-weight: 700;
    horizontal-alignment: center;
}

// After
Text {
    text: root.remaining;
    color: Theme.text-primary;
    font-size: Theme.font-size-hero;
    font-weight: 700;
    horizontal-alignment: center;
}
```

`ui/pages/recording_page.slint:117–122` — record button color:

```slint
// Before
background:
    Bridge.recording-state == RecordingState.idle      ? #cc0000
    : Bridge.recording-state == RecordingState.recording ? #cc0000
    : Bridge.recording-state == RecordingState.paused    ? Theme.warning
    : Bridge.recording-state == RecordingState.finalizing ? Theme.text-disabled
    : #cc0000;

// After
background:
    Bridge.recording-state == RecordingState.paused
        ? Theme.warning
    : Bridge.recording-state == RecordingState.finalizing
        ? Theme.text-disabled
    : Theme.recording-dot;
```

(Once step [06](./06-states-instead-of-ternaries.md) lands, this gets
rewritten as a `states [ … ]` block — but the `#cc0000` literal is gone
either way.)

## QR placeholder — black/white *is* the design

`ui/components/qr_placeholder.slint` deliberately renders **black on
white** because that's what a QR code looks like — those `#000` / `#fff`
are not theme overrides, they are payload colors. The right fix is to
*tag* them as such:

```slint
// ui/components/qr_placeholder.slint (target)
export global QrTokens {
    out property <color> module-dark:  #000;
    out property <color> module-light: #fff;
}
```

…and then `background: QrTokens.module-dark` etc. This makes future
inverted-QR / paper-cut variants trivial without polluting `Theme`.

## Migration

1. Add the new properties to `ui/theme.slint`. Keep the old names
   untouched — this step is purely additive.
2. Run a tree-wide regex replace for the **exact** literals listed in
   "Findings" above. Use `git grep -F` to keep it boring.
3. Spot-check the QR placeholder: it should still render b/w because
   `QrTokens.module-*` ships dark/light defaults.
4. Build with `cargo check -p android-sender`. Slint will fail
   compilation if any of the renamed colors are still referenced as raw
   literals inside a property typed `color` — that's the safety net.

### Per-file checklist

| File                                       | Replace                                 |
| ------------------------------------------ | --------------------------------------- |
| `ui/components/confirm_dialog.slint`       | `#00000080` → `Theme.scrim-strong`      |
| `ui/components/receiver_context_menu.slint`| `#00000040` → `Theme.scrim-light`; `8px` → `Theme.elevation-2-blur` |
| `ui/components/lock_overlay.slint`         | `48px` → `Theme.font-size-display`      |
| `ui/components/snapshot_countdown.slint`   | `72px` → `Theme.font-size-hero`         |
| `ui/pages/cast_history_page.slint`         | `20px` → `Theme.font-size-cell`         |
| `ui/pages/recording_page.slint`            | `#cc0000` → `Theme.recording-dot`       |
| `ui/components/qr_placeholder.slint`       | `#000` / `#fff` → `QrTokens.module-*`   |

## Out of scope

- Renaming existing tokens. `surface-primary` etc stay as-is — this
  step is **additive only** so the diff stays trivial.
- Light-mode / theme-switching scaffolding. The current Theme is
  dark-only and that's fine for this step.
- Introducing a `colours.slint` separate from `theme.slint`. Keep
  everything in `Theme`.

## Acceptance

- [ ] `git grep -E '#[0-9a-fA-F]{3,8}' -- 'ui/components/*.slint' 'ui/pages/*.slint' 'ui/main.slint' 'ui/bridge.slint'`
      returns **zero** hits outside `ui/components/std/` and
      `ui/components/qr_placeholder.slint`.
- [ ] `git grep -E 'font-size: [0-9]+px' -- 'ui/components/*.slint' 'ui/pages/*.slint'`
      returns **zero** hits.
- [ ] Visual diff: every page renders pixel-identical to the
      pre-refactor baseline (snapshot tests added in step
      [14](./14-testing-and-validation.md)).
