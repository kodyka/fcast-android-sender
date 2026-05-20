# 13 — Responsive layout & safe-area handling

## Goal

Pull the magic-number safe-area math out of `main.slint`, codify the
sizing conventions Slint expects (logical px, layout sizing,
`horizontal-stretch`, `min-width` / `min-height`), and document where
content can flex (ScrollView pages) vs. where it can't (control bar,
header rows).

## Findings

### F16 — magic-number safe-area in `main.slint:90–110`

```slint
in property <length> sa-top:    insets.top    > 0px ? insets.top    : 24px;
in property <length> sa-bottom: insets.bottom > 65px ? insets.bottom : 65px;
in property <length> sa-left:   insets.left;
in property <length> sa-right:  insets.right;
```

Two issues:

1. `24px` and `65px` are floor-values for the status bar / gesture
   strip — observed from Pixel 7. They don't generalise to tablets
   (where `insets.bottom` may legitimately be 0 px on a docked
   landscape device) and they don't shrink for devices without a
   gesture strip.
2. The 65 px floor is **bigger** than common edge-to-edge values
   (~16 px on devices that report any inset), forcing those devices
   to render a wider-than-necessary safe area. Visible as black bands
   on landscape tablets.

### Inline absolute sizing scattered through pages

`grep -nE '\b(width|height|min-width|min-height):\s*[0-9]+px' ui/pages/*.slint`:

About 60 sites. Most are reasonable (control-cluster widths, list-row
heights), but a few are visibly arbitrary:

- `connect_page.slint` — `height: 56px` on rows (should use
  `Theme.row-height`).
- `cast_history_detail_page.slint` — `width: 320px` on the card column
  (should be percentage-driven on landscape).
- `mixer_page.slint` — `width: 200px` thumbnails.
- `pairing_page.slint` — `width: 280px; height: 280px` for the QR
  square (should track parent width).

### `width: 100%` is unnecessary — Slint sizes children to layout

`grep -nE 'width: 100%' ui/pages/*.slint` → 23 hits. Inside a
`VerticalLayout`, children stretch horizontally by default; explicit
`width: 100%` is a no-op. Inside a `Rectangle` parent it's correct, but
in most cases the parent is a layout.

### `ScrollView`'s child should size to the view

```slint
ScrollView {
    VerticalLayout {
        // contents
    }
}
```

By default `ScrollView` fits the parent and the child gets the inner
viewport's width. No explicit `width: parent.width` needed on the
inner `VerticalLayout`. A handful of pages add it redundantly.

## Slint docs reference

- [`positioning-and-layouts.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
  — `min-width`, `max-width`, `horizontal-stretch`, `min-height`,
  `max-height`, `vertical-stretch`, alignment.
- [Window inset API](../../draft/slint-ui/docs/astro/src/content/docs/reference/window.mdx)
  — `Window.safe-area-insets` (1.16+) is the platform-native source
  for the values that today come through the FFI as `Bridge.insets`.

## Before — `main.slint:90–101`

```slint
in property <Insets> insets;
// Stable safe-area insets, never zero on Android.
in property <length> sa-top:    insets.top    > 0px ? insets.top    : 24px;
in property <length> sa-bottom: insets.bottom > 65px ? insets.bottom : 65px;
in property <length> sa-left:   insets.left;
in property <length> sa-right:  insets.right;
```

## After — single source-of-truth `SafeArea` global, no magic numbers

```slint
// ui/state/safe_area.slint  (new global)
export struct Insets {
    top:    length,
    right:  length,
    bottom: length,
    left:   length,
}

export global SafeArea {
    in property <Insets> raw: { top: 0px, right: 0px, bottom: 0px, left: 0px };

    // Minimum top/bottom guards — used only when the platform reports
    // 0 px (some emulators, legacy devices). The constants are
    // intentionally exposed so they can be tuned (or zeroed for
    // tablets) without touching window code.
    in property <length> min-top:    24px;
    in property <length> min-bottom: 0px;     // <-- WAS 65px; defer to platform.

    out property <length> top:    SafeArea.raw.top    > SafeArea.min-top    ? SafeArea.raw.top    : SafeArea.min-top;
    out property <length> bottom: SafeArea.raw.bottom > SafeArea.min-bottom ? SafeArea.raw.bottom : SafeArea.min-bottom;
    out property <length> left:   SafeArea.raw.left;
    out property <length> right:  SafeArea.raw.right;
}
```

```slint
// ui/main.slint  (target excerpt)
import { SafeArea } from "state/safe_area.slint";

export component MainWindow inherits Window {
    in property <Insets> insets <=> SafeArea.raw;
    // … now sa-top / sa-bottom usages become SafeArea.top / .bottom
}
```

Rust sets `MainWindow::set_insets(...)` once per inset change (already
plumbed). The fcc-magical `65px` floor is gone. If a specific device
truly needs a non-zero floor, set it on the **device** side (Rust
publishes a clamped value) — not in the UI shell.

> If "min-bottom 0 makes the bottom look weird on Pixel 7" remains a
> concern, *measure* it on the device and feed the value back through
> `SafeArea.raw.bottom` (the platform inset API is the truth). Hard-
> coding `65px` papers over a real platform query bug.

## After — pages use `SafeArea` directly when they need it (otherwise `PanelHost` does)

After step 03, only `PanelHost` reads `SafeArea.top`/`SafeArea.bottom`:

```slint
PanelHost {
    safe-top:    SafeArea.top;
    safe-bottom: SafeArea.bottom;
    // …
}
```

The non-panel screens (Connect / Casting / Connecting) read it inside
`main.slint`:

```slint
ConnectView {
    y:       SafeArea.top;
    height:  parent.height - SafeArea.top - SafeArea.bottom;
}
```

…or, even better, wrap them in their own `SafeArea`-aware container —
but the current direct-bind is fine.

## Inline absolute sizing — replace with `Theme.*` tokens

```slint
// Before
ListItem {
    height: 56px;
}
// After
ListItem {
    height: Theme.row-height-comfortable;     // new token
}

// Before
Image {
    width: 280px;
    height: 280px;
    source: @image-url("…/qr.png");
}
// After — QR square tracks parent width up to 80% of the shorter side.
Image {
    width: min(parent.width * 0.8, parent.height * 0.8);
    height: self.width;
    source: @image-url("…/qr.png");
}
```

Add the missing tokens to `Theme` (extends step 01):

```slint
// theme.slint
out property <length> row-height-comfortable: 56px;
out property <length> row-height-compact:     40px;
out property <length> thumbnail-width:       200px;
out property <length> qr-square-min:         240px;
out property <length> qr-square-max:         360px;
```

## Drop redundant `width: 100%`

```slint
// Before
ScrollView {
    VerticalLayout {
        width: 100%;     // <-- no-op
        // …
    }
}

// After
ScrollView {
    VerticalLayout {
        // children stretch by default inside the ScrollView's viewport
    }
}
```

`grep -nE 'width: 100%' ui/pages/*.slint` → 23 hits. Drop only the ones
where the parent is a layout (`VerticalLayout`, `HorizontalLayout`,
`ScrollView`). Inside a `Rectangle` parent, `width: 100%` is
**not** redundant.

## `horizontal-stretch` audit

`grep -nE 'horizontal-stretch:' ui/pages/*.slint | wc -l` → ~80 hits.
Most are correct (`horizontal-stretch: 1` on the flex child of a
header row, e.g. the title between "back" and "Done" buttons). But:

- `bitrate_preset_edit_page.slint:97` adds `horizontal-stretch: 1` to
  a control that's already the only child of a `HorizontalLayout` →
  redundant.
- A few pages set both `width: 100%` *and* `horizontal-stretch: 1`,
  which is contradictory. Pick one (`horizontal-stretch` if layout
  parent, `width: 100%` if `Rectangle` parent).

## Landscape behaviour

The app is portrait-only on the Manifest, but tablets that don't
respect orientation locks ship the layout in landscape anyway. Two
guidelines:

1. Use `min(parent.width, parent.height) * 0.8` for square assets
   (QR codes, status overlays) so the square fits both orientations.
2. Lay out two-column screens via `if root.width > 600px:
   HorizontalLayout { … }` / `if root.width <= 600px: VerticalLayout
   { … }`. None of the current pages do this; consider for the
   "Settings ▸ Media backend" details view as a future enhancement.

## Migration

1. Add `SafeArea` global with `raw` ↔ MainWindow `insets`.
2. Remove the `sa-top`/`sa-bottom` magic-number floors from
   `main.slint`. The two `min-*` tokens on `SafeArea` are now the only
   place that knob lives.
3. Drop redundant `width: 100%` from inside layouts (use
   `git grep -E 'width: 100%' ui/pages` and inspect each hit's parent).
4. Replace hard-coded sizes with `Theme.*` tokens per the table above
   (extends step 01).
5. Audit `horizontal-stretch` for redundancy / contradiction with
   sibling `width:` props.
6. Snapshot-test on a tablet / landscape emulator and a non-gesture
   device (no bottom inset) to verify no black-bar regressions.

### Per-file checklist

| File                                       | Change                                        |
| ------------------------------------------ | --------------------------------------------- |
| `ui/state/safe_area.slint`                 | NEW global                                    |
| `ui/main.slint`                            | Drop `sa-*` props; bind `insets <=> SafeArea.raw` |
| `ui/components/panel_host.slint`           | Read `SafeArea.top`/`SafeArea.bottom`         |
| `ui/theme.slint`                           | Add `row-height-*`, `qr-square-*`, `thumbnail-width` |
| `ui/pages/connect_page.slint`              | Replace `height: 56px` rows → `Theme.row-height-comfortable` |
| `ui/pages/pairing_page.slint`              | Replace fixed 280 px QR square → `min(...)`   |
| `ui/pages/mixer_page.slint`                | Replace fixed thumbnail size → `Theme.thumbnail-width` |
| All FCast pages                            | Audit `width: 100%` inside layouts; drop      |

## Out of scope

- Truly responsive layouts (two-column on tablets, fluid font sizes).
  Add later as a "responsive enhancements" follow-up.
- DPI scaling. Slint already does this; no action needed.
- `LayoutInfo`-driven custom layouts. The std layouts cover every
  current use-case.

## Acceptance

- [ ] `grep -n '65px' ui/main.slint` returns no hits.
- [ ] `grep -n 'width: 100%' ui/pages` returns ≤5 hits (each
      justified by a `Rectangle` parent).
- [ ] On a Pixel 7 dev build, the bottom safe-area equals the
      `insets.bottom` reported by Rust (verify via overlay logging),
      not a hard-coded floor.
- [ ] On a landscape tablet, panels render edge-to-edge without
      unnecessary banding.
