# Slint UI — best-practices implementation guide

> **Scope:** documentation only. No code changes. Every snippet below
> shows the **target** shape; the current shape is shown alongside as
> "Before" so the diff is obvious.

This guide reviews every `.slint` file currently under
[`ui/`](../../ui/) of the FCast Android sender (8.5 KLoC of Slint,
~64 files) against the upstream Slint documentation mirrored in
[`draft/slint-ui/docs/`](../../draft/slint-ui/docs/_MIRROR.md) — in
particular the [Best Practices guide](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/best-practices.mdx)
and the language-coding pages on
[properties](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx),
[globals](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/globals.mdx),
[states](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/states.mdx),
and [translations](../../draft/slint-ui/docs/astro/src/content/docs/guide/development/translations.mdx).

The output is a **15-step implementation plan**, one file per step, ordered
so each step compiles standalone on top of the previous ones. Apply them
top-to-bottom or pick the subset that matches your appetite — the file
list is independent except where called out.

## Reading order

| #  | File                                                                                       | What it fixes                                                                 |
| -- | ------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------- |
| 00 | [`00-overview.md`](./00-overview.md)                                                       | Findings summary, severity matrix, file-by-file score sheet.                  |
| 01 | [`01-theme-tokens.md`](./01-theme-tokens.md)                                               | Hard-coded colors/sizes → `Theme` tokens; scrim, elevation, type scale.       |
| 02 | [`02-split-bridge-globals.md`](./02-split-bridge-globals.md)                               | 378-line god-`Bridge` → per-feature globals (`MediaBackend`, `Recording`, …). |
| 03 | [`03-panel-host-component.md`](./03-panel-host-component.md)                               | 22× repeated panel-overlay routing in `main.slint` → `PanelHost`.             |
| 04 | [`04-card-and-header-primitives.md`](./04-card-and-header-primitives.md)                   | Repeating `Rectangle { surface-card; radius; padding }` → `Card`, `PanelHeader`. |
| 05 | [`05-button-accessibility.md`](./05-button-accessibility.md)                               | Custom buttons missing `accessible-role` / `enabled` forwarding to focus.     |
| 06 | [`06-states-instead-of-ternaries.md`](./06-states-instead-of-ternaries.md)                 | Long `?:` chains for status-pill colors → `states [ … when … ]` blocks.       |
| 07 | [`07-strict-property-directions.md`](./07-strict-property-directions.md)                   | `in-out` leakage, the `changed selected-history-id` re-emit anti-pattern.     |
| 08 | [`08-typed-models-and-enums.md`](./08-typed-models-and-enums.md)                           | Stringly-typed `kind == "wifi"`, `id == "settings"` → enums + match tables.   |
| 09 | [`09-localization-and-tr.md`](./09-localization-and-tr.md)                                 | ComboBox `value == @tr("...")` is fragile; use index/enum mapping.            |
| 10 | [`10-timers-and-animation.md`](./10-timers-and-animation.md)                               | Busy 16ms tickers → `animate` and change-driven derived properties.           |
| 11 | [`11-back-stack-and-navigation.md`](./11-back-stack-and-navigation.md)                     | Direct `Bridge.active-panel = …` writes → typed `push-panel`/`pop-panel`.     |
| 12 | [`12-focus-and-keyboard.md`](./12-focus-and-keyboard.md)                                   | `forward-focus`, `FocusScope` per panel, back-key contract.                   |
| 13 | [`13-responsive-and-safe-area.md`](./13-responsive-and-safe-area.md)                       | `sa-top`/`sa-bottom` fallback, logical pixels, sizing conventions.            |
| 14 | [`14-testing-and-validation.md`](./14-testing-and-validation.md)                           | `slint-viewer` previewing, CI `ui-validate`, `i-slint-backend-testing`.       |
| 15 | [`15-references.md`](./15-references.md)                                                   | Pointer index into `draft/slint-ui/docs/` per topic.                          |

## What this guide is NOT

- **Not a redesign.** The visual style — surfaces, accents, layout —
  stays exactly as it is today. The guide moves layout details into
  reusable primitives, tokens, and globals; it does not move pixels.
- **Not a Rust refactor.** The Rust-side `slint::ComponentHandle`,
  callback registration, `set_*` / `on_*` calls keep their current names
  in the steps below. Step 02 (Bridge split) does mechanically rename
  callbacks (`Bridge.probe_media_backend` → `MediaBackend.probe`) but
  preserves their signatures; the Rust changes shown there are minimal
  wrappers, not behavior changes.
- **Not a Slint version upgrade.** Everything in this guide is valid
  Slint 1.16.0 (the version pinned by `Cargo.toml`).
  Anything that only works on 1.17+ is flagged inline.

## Estimated effort

- **Steps 01, 04, 05, 06, 09, 10, 13** — purely additive or mechanical.
  Safe to do per page, no cross-cutting risk. Few hours each.
- **Steps 02, 03, 11, 12** — Bridge surface + window-level changes.
  Touches Rust callsites and panel overlay routing. Plan one PR per step.
- **Steps 07, 08, 14** — cross-cutting cleanups and tooling. Best done
  after the structural steps so the new patterns are in place to enforce.

## How to use this guide

Each step file follows the same template:

1. **Goal** — one sentence on what the step changes.
2. **Findings** — concrete grep-able evidence in `ui/`, with file:line
   citations.
3. **Slint docs reference** — pointer into `draft/slint-ui/docs/`.
4. **Before** — the current shape, copied verbatim.
5. **After** — the target shape, runnable as a drop-in.
6. **Migration** — file-by-file checklist, including any Rust-side
   touch-up (registration names, getter renames).
7. **Out of scope** — what this step does *not* try to fix.

> This guide intentionally leaves `ui/components/std/` (the vendored
> standard-widgets fork) **untouched** — those files are a verbatim
> copy of `slint-ui/slint/internal/compiler/widgets/` and should
> only ever change by re-vendoring from upstream. Best-practice fixes
> apply only to the FCast-authored files: `bridge.slint`, `main.slint`,
> `theme.slint`, `ui/pages/*`, and `ui/components/*` (excluding `std/`).
