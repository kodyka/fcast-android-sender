# 15 — References: Slint docs index for this guide

This guide cites the upstream Slint documentation mirrored at
[`draft/slint-ui/docs/`](../../draft/slint-ui/docs/_MIRROR.md). Below
is a flat list of the most relevant pages with one-sentence summaries
and which guide step relies on each.

## Mirror metadata

- Upstream repo: [`slint-ui/slint`](https://github.com/slint-ui/slint)
- Pinned commit: `d79203f` (2026-05-02)
- Upstream Slint version on that commit: **v1.17.0**
- FCast Slint version pin: **v1.15.1** (FUTO fork)
- See [`draft/slint-ui/docs/_MIRROR.md`](../../draft/slint-ui/docs/_MIRROR.md)
  for the full mirror contract and version-skew rules.

The pages are forward-compatible: every cited best-practice exists in
v1.15.1 unless flagged inline. v1.16+/1.17+-only features are called
out per-step.

## Language coding

| Doc                                                                                                                       | Summary                                            | Cited in steps  |
| ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | ---------------- |
| [`properties.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx)                 | Direction qualifiers, change-callback caveats.     | 02, 07           |
| [`globals.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/globals.mdx)                       | Singletons, multiple globals, Rust trait gen.      | 02, 11           |
| [`states.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/states.mdx)                         | `states [name when cond: { … }]` + transitions.    | 06               |
| [`functions-and-callbacks.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx) | Public functions, callback dispatch.              | 07, 11           |
| [`expressions-and-statements.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/expressions-and-statements.mdx) | Pure functions, conditional expressions. | 09, 13           |
| [`structs-and-enums.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)   | Defining + using `struct` / `enum`.                | 08               |
| [`positioning-and-layouts.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx) | Layout primitives, stretching, alignment.       | 04, 13           |
| [`repetition-and-data-models.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/repetition-and-data-models.mdx) | `for item in model`, dynamic lists.            | 03, 08           |

## Development guides

| Doc                                                                                                                                                  | Summary                                                            | Cited in steps  |
| ---------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------- |
| [`best-practices.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx)                                        | Accessibility, separate UI/code, translations.                     | 00, 01, 05, 09  |
| [`custom-controls.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/custom-controls.mdx)                                     | Reusable components with `@children`, slots, styling.              | 04, 05          |
| [`focus.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/focus.mdx)                                                          | `FocusScope`, `forward-focus`, key event flow.                     | 12              |
| [`translations.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/translations.mdx)                                            | `@tr(context => msgid, args…)`, plural forms.                      | 09              |
| [`testing.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/testing.mdx)                                                      | `i-slint-backend-testing`, headless, a11y dumps.                   | 14              |

## Reference

| Doc                                                                                                                       | Summary                                            | Cited in steps  |
| ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | ---------------- |
| [`elements/rectangle.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/elements/rectangle.mdx)             | `Rectangle` primitive, drop-shadow, border.       | 01, 04          |
| [`elements/touch-area.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/elements/touch-area.mdx)           | `clicked`, `pressed`, `long-pressed` (1.16+).      | 10              |
| [`elements/animations.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/elements/animations.mdx)           | `animate` block, easing functions.                 | 06, 10          |
| [`timer.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/timer.mdx)                                       | Discrete `Timer { interval; running; triggered }`. | 10              |
| [`window.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/window.mdx)                                     | `Window` properties, `safe-area-insets` (1.16+).   | 13              |
| [`std-widgets/views/scrollview.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/std-widgets/views/scrollview.mdx) | `ScrollView`, `mouse-drag-pan-enabled`, viewport.  | 04, 13          |
| [`std-widgets/inputs/combobox.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/std-widgets/inputs/combobox.mdx) | `ComboBox.model`, `selected`, `current-index-changed`. | 09          |
| [`std-widgets/inputs/lineedit.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/std-widgets/inputs/lineedit.mdx) | `LineEdit.text`, `edited`, `accepted`, `input-type`. | 04, 12         |
| [`global-functions/math.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/reference/global-functions/math.mdx)       | `Math.clamp`, `Math.floor`, etc.                  | 10, 13          |

## Quickstart & tooling

| Doc                                                                                                                       | Summary                                            | Cited in steps  |
| ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- | ---------------- |
| [`quickstart/cli.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/quickstart/cli.mdx)                                 | `cargo install slint-viewer`, hot-reload.          | 14              |

## Examples (mirrored in `draft/slint-ui/examples/`)

The Slint repo ships canonical example apps. These were spot-checked
when authoring the steps; consult them for full working code.

| Example                                                | Demonstrates                            |
| ------------------------------------------------------ | --------------------------------------- |
| `examples/todo-mvc/ui/index.slint`                     | Routing-by-state pattern (step 03).     |
| `examples/gallery/gallery.slint`                       | Card / surface primitives (step 04).    |
| `examples/printerdemo`                                 | Per-panel `FocusScope` (step 12).       |
| `examples/iot-dashboard`                               | `states` for live-update UI (step 06).  |

## How to update this index

When the mirror is refreshed:

1. Walk `draft/slint-ui/docs/_MIRROR.md` to confirm the upstream commit
   bumped without breaking links above.
2. For each guide-step file (`00-overview.md` … `14-testing-and-validation.md`),
   verify every `../../draft/slint-ui/docs/...` link still resolves.
3. If a doc has been moved or renamed upstream, update both
   `_MIRROR.md` (link to new path) and this index simultaneously.
4. Flag any new best-practices content not yet incorporated as a
   follow-up issue.

## Tooling versions used while authoring

- `slint = "1.15.1"` (FCast pin via `Cargo.toml`)
- `slint-viewer 1.15.1`
- `i-slint-backend-testing 1.15.1`
- `slint-tr-extractor 1.15.1`

If a future step depends on a 1.16+/1.17 feature, the step file flags
it inline ("**Slint 1.16+:**") and provides a 1.15-compatible fallback.

## Out of scope

- Slint internals (compiler architecture, IR). The guide is
  application-facing.
- Non-FCast UI projects. References here are tied to the FCast
  Android sender directory layout.
