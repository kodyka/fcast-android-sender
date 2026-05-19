# Phase 3 — Reusable Button and Row Components

> Build the shared component library that every page will use.
> Inspired by Moblin `View/Utils` (36 files); implemented as pure Slint.
> Reference: `draft/moblin-ui/Moblin/View/Utils/`

**Status:** `[x] Complete`
**Depends on:** Phase 1 (module layout exists), Phase 2 (Theme tokens available)
**Unlocks:** Phase 4 (control bar uses buttons), Phase 7 (settings page uses rows)
**Related files:**
- `senders/android/ui/components/buttons.slint` — stub from Phase 1-H
- `senders/android/ui/components/settings_rows.slint` — stub from Phase 1-H
- `senders/android/ui/theme.slint` — provides all color/size tokens

**Moblin source analogues (reference only — do not copy Swift):**

| Moblin file | Slint equivalent |
|---|---|
| `ButtonView.swift` | `PrimaryButton` |
| `BorderlessButtonView.swift` | `TextButton` |
| `CloseToolbarView.swift` | `PanelHeader` |
| `TextItemView.swift` | `SettingsTextRow` |
| `TextValueView.swift` | `SettingsValueRow` |
| `SliderView.swift` | `SettingsSliderRow` |
| `InlinePickerView.swift` | `SegmentedChoiceRow` (Phase 7) |

---

## Tasks

### 3-A — `buttons.slint` — `PrimaryButton`

- [x] Open `senders/android/ui/components/buttons.slint`.
- [x] Add `import { Theme } from "../theme.slint";`.
- [x] Implement `export component PrimaryButton`.
- [x] **Build check.**

---

### 3-B — `buttons.slint` — `TextButton`

- [x] Implement `export component TextButton` (borderless).
- [x] **Build check.**

---

### 3-C — `buttons.slint` — `DestructiveButton`

- [x] Implement `export component DestructiveButton` (red stop/cancel).
- [x] **Build check.**

---

### 3-D — `buttons.slint` — `LoadingView`

> Shared spinner + label used by `ConnectingView` and `WaitingForMediaView`.

- [x] Add `import { Spinner } from "std-widgets.slint";`.
- [x] Implement `export component LoadingView`.
- [x] Update `connecting_page.slint`: replace inline `Spinner + Text` with `LoadingView { label: "Connecting"; }`.
- [x] Update `casting_page.slint` (`WaitingForMediaView`): replace inline `Spinner + Text` with `LoadingView { label: "Waiting for media"; }`.
- [x] **Build check.**

---

### 3-E — `settings_rows.slint` — `SettingsTextRow`

- [x] Open `senders/android/ui/components/settings_rows.slint`.
- [x] Add `import { Theme } from "../theme.slint";`.
- [x] Implement `export component SettingsTextRow` (label-only, non-interactive).
- [x] **Build check.**

---

### 3-F — `settings_rows.slint` — `SettingsValueRow`

- [x] Implement `export component SettingsValueRow` (label + trailing value + chevron).
- [x] **Build check.**

---

### 3-G — `settings_rows.slint` — `SettingsToggleRow`

- [x] Add `import { CheckBox } from "std-widgets.slint";`.
- [x] Implement `export component SettingsToggleRow`.
- [x] **Build check.**

---

### 3-H — `settings_rows.slint` — `SettingsSliderRow`

- [x] Add `import { Slider } from "std-widgets.slint";`.
- [x] Implement `export component SettingsSliderRow`.
- [x] **Build check.**

---

### 3-I — `settings_rows.slint` — `SettingsSection`

- [x] Implement `export component SettingsSection` (titled card container).
- [x] **Build check.**

---

### 3-J — Replace raw `Button` usages with `PrimaryButton`

- [x] In `connect_page.slint`: Replace `Button { text: "Scan QR"; clicked => Bridge.scan-qr(); }` with `PrimaryButton { label: "Scan QR"; clicked => { Bridge.scan-qr(); } }`.
- [x] In `settings_page.slint` (`SettingsPageView`): Replace `Button { text: "Start"; ... }` with `PrimaryButton` and Replace `Button { text: "Disconnect"; ... }` with `DestructiveButton`.
- [x] In `casting_page.slint`: Replace `Button { text: "Cancel"; ... }` with `DestructiveButton` and Replace `Button { text: "Stop"; ... }` with `DestructiveButton`.
- [x] In `debug_page.slint`: Replace all plain `Button` instances with `PrimaryButton` or `TextButton` as appropriate.
- [x] **Final build check.**

---

## Exit criteria

Phase 3 is complete when:

1. `buttons.slint` exports: `PrimaryButton`, `TextButton`, `DestructiveButton`, `LoadingView`.
2. `settings_rows.slint` exports: `SettingsTextRow`, `SettingsValueRow`, `SettingsToggleRow`,
   `SettingsSliderRow`, `SettingsSection`.
3. No raw `Button` from `std-widgets.slint` remains in page files (all replaced with typed buttons).
4. `ConnectingView` and `WaitingForMediaView` use `LoadingView`.
5. `cargo build -p android-sender` passes cleanly.
6. All five app states still render and function correctly on device or `slint-viewer`.

---

## Slint best practices applied here

- **Std-widget callback signatures match the docs verbatim.** `CheckBox.toggled` is `()`,
  `Slider.changed` is `changed(float)`. When forwarding to a wrapper component callback,
  always declare the parameter list explicitly (`toggled() => { ... }`, `changed(value) => { ... }`)
  rather than relying on the no-param shorthand. References:
  [`reference/std-widgets/basic-widgets/checkbox.mdx`](https://github.com/slint-ui/slint/blob/master/docs/astro/src/content/docs/reference/std-widgets/basic-widgets/checkbox.mdx),
  [`reference/std-widgets/basic-widgets/slider.mdx`](https://github.com/slint-ui/slint/blob/master/docs/astro/src/content/docs/reference/std-widgets/basic-widgets/slider.mdx).
- **`@children` is the canonical way to make a container component.** `SettingsSection`
  uses `@children` to accept arbitrary row components, which is how std-widgets like
  `VerticalBox` work internally.
- **Color manipulation uses brush methods**, e.g. `Theme.error.darker(20%)`,
  `Theme.accent.brighter(0.2)`. Reference:
  [`reference/colors-and-brushes.mdx`](https://github.com/slint-ui/slint/blob/master/docs/astro/src/content/docs/reference/colors-and-brushes.mdx).
- **Button accessibility tip (future improvement):** the std-widget `Button` exposes
  accessibility properties (`accessible-role`, `accessible-label`) that our custom
  Rectangle-based buttons currently miss. If accessibility becomes a requirement, swap
  the `inherits Rectangle` chassis for `inherits TouchArea` (no Rectangle wrapping needed)
  or add explicit `accessible-*` properties.
