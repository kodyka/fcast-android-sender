# 00 — Overview: findings & severity matrix

## Goal

Establish the **state of the UI**, score every FCast-authored `.slint`
file against Slint best practices, and pick the order for the remaining
steps so the highest-impact fixes land first.

## Inventory

`find ui -type f -name '*.slint' | wc -l` → **64 files / 8 486 lines** as
of the branch this guide was authored on. Splitting by ownership:

| Bucket                                  | Files | Lines | Authored by                              |
| --------------------------------------- | ----- | ----- | ---------------------------------------- |
| `ui/{bridge,main,theme}.slint`          | 3     | 614   | FCast                                    |
| `ui/components/` (excluding `std/`, `mcore/`) | 15 | 1 122 | FCast                                    |
| `ui/components/std/`                    | 23    | 2 663 | **Vendored** — verbatim from `slint-ui/slint` |
| `ui/components/mcore/`                  | 1     | 29    | Vendored (Moblin core picker presets)    |
| `ui/pages/`                             | 25    | 4 058 | FCast                                    |

This guide **only** addresses the 43 FCast-authored files. The
`ui/components/std/` fork must not drift from upstream — touching it is
out of scope for *every* step in this guide.

## Findings — severity ladder

Each finding has been graded by:

- **Impact (I):** how much code/behaviour is affected if not fixed.
- **Effort (E):** rough size of the change (S = ≤1 file, M = 2–5 files, L = ≥6 files).
- **Risk (R):** L/M/H for the chance of breaking working behaviour.

| #  | Finding                                                                                               | I | E | R | Step              |
| -- | ----------------------------------------------------------------------------------------------------- | - | - | - | ----------------- |
| F1 | `ui/bridge.slint` is a 378-line god-singleton mixing 13 unrelated concerns.                            | 3 | L | M | [02](./02-split-bridge-globals.md) |
| F2 | `ui/main.slint:141–172` repeats `y: root.sa-top; height: root.height - root.sa-top - root.sa-bottom;` for **22 panels**. | 3 | M | L | [03](./03-panel-host-component.md) |
| F3 | "Header bar" pattern (Rectangle 56px + Text + TextButton "Done") is duplicated in 17 pages.            | 2 | M | L | [04](./04-card-and-header-primitives.md) |
| F4 | "Card" wrapping (Rectangle { surface-card; radius-card; padding-screen }) inlined ≥60 times.          | 2 | M | L | [04](./04-card-and-header-primitives.md) |
| F5 | Hard-coded font sizes: `48px`, `72px`, `20px`; hard-coded scrim `#00000080`; hard-coded glyph colors. | 2 | S | L | [01](./01-theme-tokens.md) |
| F6 | `media_backend_page.slint:54–69` four-arm `?:` chain for status pill color *and* label.               | 2 | S | L | [06](./06-states-instead-of-ternaries.md) |
| F7 | `media_backend_page.slint:108–113` parses ComboBox selection by `value == @tr("Migration…")` — breaks if locale changes. | 3 | S | L | [09](./09-localization-and-tr.md) |
| F8 | `bridge.slint:275–277` `changed selected-history-id => Bridge.selected-history-id-changed(...)` re-emits the same value as a callback — explicitly warned against by the Slint properties doc. | 2 | S | L | [07](./07-strict-property-directions.md) |
| F9 | Custom buttons (`PrimaryButton`, `TextButton`, `DestructiveButton`) lack `accessible-role: button` and `accessible-label`. | 2 | S | L | [05](./05-button-accessibility.md) |
| F10 | `lock_overlay.slint` polls a 16 ms `Timer` to drive a hold-progress bar; expressible as `animate hold-progress { duration: 1.5s }`. | 1 | S | M | [10](./10-timers-and-animation.md) |
| F11 | `bridge.slint` exposes many `in-out` properties that are written from one side only (e.g. `app-state` is documented to be Slint-write-only via `change-state()` but is still declared `in-out`, leaking write capability to Rust). | 2 | M | M | [07](./07-strict-property-directions.md) |
| F12 | `network_page.slint` and `control_bar.slint` use stringly-typed `kind == "wifi"` / `id == "settings"`. | 2 | M | M | [08](./08-typed-models-and-enums.md) |
| F13 | `connect_page.slint:54–66` rolls its own 600 ms long-press detector with `Timer { interval: 600ms }`. Slint 1.16 ships `TouchArea.long-pressed`. | 1 | S | L | [10](./10-timers-and-animation.md) |
| F14 | `Bridge.active-panel` is written **directly** from 50+ sites — no back-stack tracking, so "Back" always lands on `Panel.none`. | 3 | M | M | [11](./11-back-stack-and-navigation.md) |
| F15 | Only one `FocusScope` in the entire FCast tree (the top-level `back-key-scope` in `main.slint`); panels capturing the back key all rely on it bubbling. | 2 | S | L | [12](./12-focus-and-keyboard.md) |
| F16 | Safe-area fallback `> 65px ? insets.bottom : 65px` (line `main.slint:101`) is a magic number; relies on observed device geometry. | 1 | S | M | [13](./13-responsive-and-safe-area.md) |
| F17 | No `slint-viewer` workflow documented; CI's `ui-validate` runs `slint-build` only — no snapshot tests, no accessibility lint. | 1 | M | L | [14](./14-testing-and-validation.md) |
| F18 | `i18n/messages.pot` exists but no `.po` translations are checked in; no `@tr("...", arg)` discipline for plural forms is enforced. | 1 | S | L | [09](./09-localization-and-tr.md) |
| F19 | `Panel` is one flat enum of 22 variants; no grouping → easy to miss a routing arm when a panel is added. | 2 | S | L | [08](./08-typed-models-and-enums.md) |
| F20 | `lock_overlay.slint` Timer keeps running at 16 ms even when the overlay isn't visible (it's gated by `hold-area.pressed`, but the *element tree* is still alive whenever `LifecycleMode != normal`). | 1 | S | L | [10](./10-timers-and-animation.md) |

> *I = Impact (1 low, 3 high). E = Effort. R = Risk.*

## What's already good

Worth calling out so the steps below don't accidentally regress it:

- `bridge.slint` already documents the "who can write which property"
  invariants in `main.slint:1–23`. The invariants are correct — the
  problem is that they're enforced by **convention**, not by property
  direction (`in` / `out` / `in-out`).
- `theme.slint` already has a well-named token palette (`surface-*`,
  `text-*`, `accent-*`, `font-size-*`, `padding-*`, `radius-*`). The
  bulk of the inline color/size literals can be migrated **into** it
  without inventing a new system.
- `confirm_dialog.slint`, `info_banner.slint`, `status_overlay.slint`
  are already side-effect-free leaf components with caller-owned
  visibility. They're the **template** the other components should
  follow.
- `recording_page.slint:42–53` shows the right pattern for derived
  formatting: a `pure function format-elapsed(total-s)` declared on the
  component instead of an inline ternary mess. The other pages should
  adopt this where they currently inline string interpolation.

## Step ordering rationale

The 15 steps are ordered to **maximise compileable check-points** —
every step's "After" snippet should still compile, and most steps are
independent. The dependency edges are:

```
01 (theme tokens) ──────┬──> 04 (card/header primitives)
                        ├──> 06 (states)
                        └──> 13 (responsive)
02 (split Bridge) ──────┬──> 07 (property directions)
                        ├──> 08 (typed models)
                        └──> 11 (navigation)
03 (PanelHost)   ───────┬──> 11 (navigation)
                        └──> 12 (focus)
05 (a11y) ──────────────┘
```

Steps 09, 10, 14, 15 are independent of all others. Step 00 (this file)
exists only to set the scoreboard.

## Out of scope

- Visual redesign (colours, layout, type).
- The vendored `ui/components/std/` and `ui/components/mcore/`
  directories.
- Rust-side state-machine refactors. The Bridge split (step 02) is the
  only step that touches Rust, and only to rename callbacks.
- `i18n/messages.pot` content. Step 09 changes call-sites, not the
  translation catalogue.

## Acceptance for this step

- [ ] Reader can locate every numbered finding (F1…F20) in the linked
      step file.
- [ ] Reader agrees with the impact/effort/risk scoring or proposes
      a revised matrix.
- [ ] Step order has been confirmed before the team starts on step 01.
