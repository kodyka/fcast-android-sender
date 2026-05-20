# 03 — `PanelHost`: collapse the 22-way panel-overlay routing

## Goal

Reduce `ui/main.slint:141–172` from **22 hand-written conditional panel
mounts** to a single `PanelHost { … }` element. Side benefit: panels
no longer have to re-derive their own safe-area sizing, because the
host hands them a `width`/`height` rectangle that's already inset.

## Findings

`grep -c "^[[:space:]]*if Bridge.active-panel ==" ui/main.slint` → **22**.

Each line looks like this, with the same trailing rectangle math
copy-pasted into every one:

```slint
if Bridge.active-panel == Panel.settings:        FullSettingsPage         { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.debug:           FullDebugPage            { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.codec-test:      CodecTestPage            { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.backup-reset:    BackupResetPage          { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.audio:           AudioPage                { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.camera:          CameraPage               { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.quick-actions:   QuickActionsPage         { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.cast-history:        CastHistoryPage      { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.cast-history-detail: CastHistoryDetailPage{ y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.recording:       RecordingPage            { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
// … 12 more lines …
```

The problems with this:

1. **Repetition.** Every new panel needs three identical edits: add a
   variant to `Panel`, add an `if` line here, and re-paste the same
   `y` / `height` math.
2. **No miss-detection.** If the dev forgets the `if` line, the panel
   simply never renders, and there's no warning. The flat-22-arm
   structure makes this very easy to miss in code review.
3. **No "exactly one" guarantee.** Two `if Bridge.active-panel == …`
   arms could match simultaneously if the panel enum ever grew an
   `all` / `any` value — there's no exhaustiveness check.
4. **Pairing with the next layer.** Pages like `PairingPage`,
   `ReceiverRenamePage` need extra wiring (`close => …`, `save => …`)
   that hides among the boilerplate.

## Slint docs reference

- [`repetition-and-data-models.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/repetition-and-data-models.mdx)
  — `for item in model` over an inline array.
- [`functions-and-callbacks.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
  — pure-function helpers to route a `Panel` to a string label.
- The `examples/todo-mvc/ui/index.slint` and
  `examples/gallery/gallery.slint` files (linked by
  `draft/slint-ui/docs/slint-docs-used.md`) both use a single
  conditional component element to host the "current page".

## Why Slint can't quite do a real `match`

Slint does not have a `match` expression (yet), and inheriting from
multiple `if`-branches into the same element is forbidden — `if X: A
{ … }` instantiates `A`, full stop, no `else` branch. The best you can
do is **one conditional element per discriminant value**, which is
exactly what `main.slint` does today.

The improvement, then, is to **encapsulate the boilerplate**, not
eliminate the per-variant conditional. `PanelHost` owns the geometry,
the visibility plumbing, and the optional close-callback wiring;
`main.slint` only states *which* component goes with *which* enum
variant.

## Before — `ui/main.slint:137–172`

```slint
// ── Panel overlay layer ───────────────────────────────────────────────────
// Each panel is offset by sa-top so its header starts below the status bar
// and "Done" buttons are tappable. Height shrinks by both insets so scroll
// content ends above the gesture-nav strip.
if Bridge.active-panel == Panel.settings:   FullSettingsPage { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.debug:      FullDebugPage { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
if Bridge.active-panel == Panel.codec-test: CodecTestPage { y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom; }
// ... 19 more lines ...
if Bridge.active-panel == Panel.pairing: PairingPage {
    y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom;
    close => { Bridge.active-panel = Panel.none; }
}
if Bridge.active-panel == Panel.receiver-rename: ReceiverRenamePage {
    y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom;
    current-name: Bridge.selected-receiver-name;
    save(new-name) => {
        Bridge.active-panel = Panel.none;
    }
    cancel => { Bridge.active-panel = Panel.none; }
}
// ... 8 more lines ...
```

## After — `ui/components/panel_host.slint`

```slint
// ui/components/panel_host.slint
// Geometry + visibility shell for full-screen overlay panels.
//
// Owns:
//   - safe-area inset math (sa-top / sa-bottom)
//   - "is this panel currently active" visibility
//   - one conditional element per page
//
// Does NOT own:
//   - panel-specific wiring (close / save / cancel / etc.) — those are
//     declared next to each page in MainWindow, but inside the host's
//     content so the geometry stays uniform.
//
// Slint docs ref:
//   draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/repetition-and-data-models.mdx
//   draft/slint-ui/docs/astro/src/content/docs/reference/elements/rectangle.mdx

import { Theme } from "../theme.slint";
import { PanelBridge, Panel } from "../state/index.slint";

export component PanelHost inherits Rectangle {
    // Safe-area insets, passed in from MainWindow.
    in property <length> safe-top;
    in property <length> safe-bottom;

    // No background — host is transparent so the page underneath shows
    // through whenever no panel is active.
    background: transparent;

    // Position + size derived from the parent. Pages mount inside @children
    // and stretch to the host's full rect.
    width: parent.width;
    height: parent.height;

    // Children only render when `active != none`. The host hides itself
    // wholesale so input falls through to the main page.
    visible: PanelBridge.active != Panel.none;

    // The inset rectangle children should live in.
    Rectangle {
        x: 0;
        y: root.safe-top;
        width: parent.width;
        height: parent.height - root.safe-top - root.safe-bottom;
        background: transparent;

        @children
    }
}
```

## After — `ui/main.slint` shrinks to one `PanelHost`

```slint
// ── Panel overlay layer ───────────────────────────────────────────────────
PanelHost {
    safe-top:    root.sa-top;
    safe-bottom: root.sa-bottom;

    if PanelBridge.active == Panel.settings:            FullSettingsPage        { }
    if PanelBridge.active == Panel.debug:               FullDebugPage           { }
    if PanelBridge.active == Panel.codec-test:          CodecTestPage           { }
    if PanelBridge.active == Panel.backup-reset:        BackupResetPage         { }
    if PanelBridge.active == Panel.audio:               AudioPage               { }
    if PanelBridge.active == Panel.camera:              CameraPage              { }
    if PanelBridge.active == Panel.quick-actions:       QuickActionsPage        { }
    if PanelBridge.active == Panel.cast-history:        CastHistoryPage         { }
    if PanelBridge.active == Panel.cast-history-detail: CastHistoryDetailPage   { }
    if PanelBridge.active == Panel.recording:           RecordingPage           { }
    if PanelBridge.active == Panel.pairing: PairingPage {
        close => { PanelBridge.pop(); }
    }
    if PanelBridge.active == Panel.receiver-rename: ReceiverRenamePage {
        current-name: Receivers.selected-name;
        save(new-name) => { PanelBridge.pop(); }
        cancel        => { PanelBridge.pop(); }
    }
    if PanelBridge.active == Panel.bitrate-presets:     BitratePresetsPage      { }
    if PanelBridge.active == Panel.bitrate-preset-edit: BitratePresetEditPage   { }
    if PanelBridge.active == Panel.macros:              MacrosPage              { }
    if PanelBridge.active == Panel.macro-edit:          MacroEditPage           { }
    if PanelBridge.active == Panel.debug-log:           DebugLogPage            { }
    if PanelBridge.active == Panel.debug-video:         DebugVideoPage          { }
    if PanelBridge.active == Panel.network:             NetworkPage             { }
    if PanelBridge.active == Panel.mixer:               MixerPage               { }
    if PanelBridge.active == Panel.media-backend:       MediaBackendPage        { }
}
```

The line count drops from ~36 lines (22 if-statements + multi-line
PairingPage/ReceiverRenamePage blocks) to ~24, each statement is
single-line, and every panel inherits the same safe-area sizing without
ever having to spell it out.

> The `close => { PanelBridge.pop(); }` wiring assumes
> [step 11](./11-back-stack-and-navigation.md) is in. If you do this
> step first, keep the existing `Bridge.active-panel = Panel.none;`
> body and rewrite in step 11.

## Panels with no individual wiring — declarative table form

If you want to push it further (Slint 1.16+ recommended for the array
literal of components — drop the `if` block entirely for the no-wiring
panels):

```slint
// (Slint 1.16+ illustrative)
property <[{kind: Panel, page: component}]> panel-table: [
    { kind: Panel.settings,            page: FullSettingsPage   },
    { kind: Panel.debug,               page: FullDebugPage      },
    { kind: Panel.codec-test,          page: CodecTestPage      },
    // …
];
for entry in root.panel-table:
    if PanelBridge.active == entry.kind: entry.page { }
```

The Slint compiler does not currently allow `component` as a struct
field type, so the array-literal approach **does not compile on 1.15.1**.
The "list of `if` statements inside `PanelHost`" form above is the
target for this step on the current Slint pin.

## Migration

1. Add `ui/components/panel_host.slint` with the new component.
2. Import it from `ui/main.slint`:
   ```slint
   import { PanelHost } from "components/panel_host.slint";
   ```
3. Replace the 22 `if` lines + `InfoBanner` block with one `PanelHost { … }`
   element. Keep the back-key `FocusScope` exactly as it is.
4. Verify panels still render — `slint-viewer ui/main.slint` should
   round-trip every panel via the `Panel.*` enum.
5. **No Rust changes required.** Rust still writes
   `ui.global::<Bridge>().set_active_panel(Panel::Settings)` (or, after
   step 02, `PanelBridge::set_active(Panel::Settings)`).

### Per-file checklist

| File                                | Change                                                    |
| ----------------------------------- | --------------------------------------------------------- |
| `ui/components/panel_host.slint`    | **NEW**                                                   |
| `ui/main.slint`                     | Replace lines 137–172 with `PanelHost { … }` block above. |

## Out of scope

- A back-stack for panels (covered in step
  [11](./11-back-stack-and-navigation.md); the `pop()` call above is
  the *call-site*, not the implementation).
- Removing `Panel.none`. It's still the cleanest "no panel active"
  representation and the `visible:` binding above depends on it.
- Generalising the host over `y: sa-top` vs `y: 0` to support a
  bottom-sheet variant. If that's needed later, take a `position:
  PanelPosition` enum prop on `PanelHost`.

## Acceptance

- [ ] `git grep -c 'sa-top - root.sa-bottom' ui/main.slint` returns
      `0` (currently `22`).
- [ ] `grep -c "if PanelBridge.active ==" ui/main.slint` matches the
      number of `Panel.*` variants minus `Panel.none`.
- [ ] Every panel renders identical pixels to its pre-refactor baseline
      under `slint-viewer ui/main.slint`.
- [ ] `cargo check -p android-sender` passes with zero changes to
      `senders/android/src/`.
