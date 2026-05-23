# Test Functionality Page — Implementation Guide

Step-by-step guide for creating `ui/pages/test_functionality_page.slint`,
a unified test screen combining a **camera phone video source**, **SRT
source**, and **image overlay** section with start/stop test controls.

> **Guide only — no code modifications.**
> All Slint snippets follow the existing theme-token and `@tr()`
> conventions enforced by `.pre-commit-config.yaml`.

## STEP files

| # | Title | Scope |
|---|-------|-------|
| 01 | [Add Panel enum variant](./STEP-01-panel-enum-variant.md) | `ui/bridge.slint` — add `test-functionality` to `Panel` |
| 02 | [Bridge properties & callbacks](./STEP-02-bridge-properties.md) | `ui/bridge.slint` — camera, SRT, overlay, test-state properties |
| 03 | [Feature-scoped state global](./STEP-03-state-global.md) | `ui/state/test_functionality.slint` (new) + barrel re-export |
| 04 | [Camera source section](./STEP-04-camera-source-section.md) | Internal `CameraSourceSection` component with full snippet |
| 05 | [SRT source section](./STEP-05-srt-source-section.md) | Internal `TestSrtSourceCard` component with full snippet |
| 06 | [Image overlay section](./STEP-06-image-overlay-section.md) | Internal `ImageOverlayCard` component with full snippet |
| 07 | [Assemble the page](./STEP-07-assemble-page.md) | `ui/pages/test_functionality_page.slint` — root component |
| 08 | [Register in main.slint](./STEP-08-register-main.md) | Import + PanelHost conditional |
| 09 | [Wire Rust callbacks](./STEP-09-rust-callbacks.md) | `on_start_test`, `on_stop_test`, `on_pick_test_overlay_image` |
| 10 | [Add navigation entry](./STEP-10-navigation-entry.md) | Settings page row + optional QuickAction |

## Conventions

* Slint snippets use only `Theme.*` tokens — no raw hex colours or
  hard-coded `font-size: Npx`.
* All user-visible strings wrapped in `@tr(...)`.
* Panel navigation via `PanelBridge.push/pop` — no direct
  `Bridge.active-panel` writes.
* Camera section adapted from `camera_page.slint` SOURCE section +
  `draft/moblin-ui/.../CameraSettingsView.swift`.
* SRT section adapted from `mixer_page.slint` `SrtSourceRow` +
  `draft/moblin-ui/.../SrtlaServerSettingsView.swift`.
* Image overlay section adapted from
  `draft/moblin-ui/.../WidgetImageSettingsView.swift`.

## Swift → Slint mapping reference

| Moblin (SwiftUI) | Slint equivalent |
|------------------|------------------|
| `Section { } header: { Text("...") }` | `SettingsSection { title: @tr("...") }` |
| `Toggle("label", isOn:)` | `SettingsToggleRow { title: ...; toggled(...) }` |
| `Slider(value:, in:)` | `SettingsSliderRow { value <=> ...; changed(...) }` |
| `NavigationLink { ... } label: { Text(...) }` | `SettingsValueRow { clicked => { ... } }` |
| `TextField("placeholder", text:)` | `LineEdit { placeholder-text: ...; text <=> ... }` |
| `PhotosPicker(selection:)` | `LineEdit` + `PrimaryButton { clicked => pick-image() }` |
| `Image(uiImage:)` | Placeholder `Rectangle` (live preview needs `NativeImage` from Rust) |
| `Form { Section { ... } }` | `ScrollView { VerticalLayout { SettingsSection { ... } } }` |
| `Toggle(isOn:).disabled(condition)` | `SettingsToggleRow { enabled: !condition; }` |
