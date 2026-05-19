# MVP-PHASE-11 — Two-SRT-sources mixer screen with RTMP egress (Slint UI ↔ migration runtime)

> Parent doc. Step-by-step children:
>
> - [`MVP-PHASE-11-STEP-1-preflight-inventory.md`](./MVP-PHASE-11-STEP-1-preflight-inventory.md)
> - [`MVP-PHASE-11-STEP-2-bridge-data-model.md`](./MVP-PHASE-11-STEP-2-bridge-data-model.md)
> - [`MVP-PHASE-11-STEP-3-bridge-callbacks.md`](./MVP-PHASE-11-STEP-3-bridge-callbacks.md)
> - [`MVP-PHASE-11-STEP-4-panel-routing.md`](./MVP-PHASE-11-STEP-4-panel-routing.md)
> - [`MVP-PHASE-11-STEP-5-srt-source-section.md`](./MVP-PHASE-11-STEP-5-srt-source-section.md)
> - [`MVP-PHASE-11-STEP-6-mix-controls-section.md`](./MVP-PHASE-11-STEP-6-mix-controls-section.md)
> - [`MVP-PHASE-11-STEP-7-rtmp-destination-section.md`](./MVP-PHASE-11-STEP-7-rtmp-destination-section.md)
> - [`MVP-PHASE-11-STEP-8-page-assembly.md`](./MVP-PHASE-11-STEP-8-page-assembly.md)
> - [`MVP-PHASE-11-STEP-9-rust-handler-reference.md`](./MVP-PHASE-11-STEP-9-rust-handler-reference.md)
>
> **Doc-only.** Every step describes what the implementation should look
> like, with concrete Slint snippets and `file:line` citations into the
> current tree. **No source-tree files are modified by reading this
> phase.** Implementers should land the steps in order, each as its own
> commit.
>
> **Naming convention.** This phase belongs to the MVP step-by-step
> series (MVP-PHASE-9 / MVP-PHASE-10). The pre-existing
> [`PHASE-11-source-tracking.md`](./PHASE-11-source-tracking.md) is a
> separate, reference-only foundation doc — the two do not collide.

---

## 0. Goal

Add a new in-app screen that lets the user:

1. Configure **two SRT sources** (URL + per-source latency/stream-id),
   mix them into a single A/V graph using the in-process
   `src/migration/` node-graph engine.
2. Tune the mix in real time — per-source **alpha**, **z-order**, and
   **volume** sliders, plus a global **canvas resolution / sample-rate**
   row.
3. Push the mixer's output to an **RTMP server** in the background, via
   the same migration JSON command API (`DestinationFamily::Rtmp` —
   `src/migration/protocol.rs:148-150`).

The screen is a new `Panel` value (`Panel.mixer`) hung off
`FullSettingsPage` (same routing pattern as `Panel.network`, see
`ui/main.slint` and `ui/pages/settings_page.slint`). When the user taps
**Start**, the page invokes a single Bridge callback,
`Bridge.start-mixer-cast()`, which on the Rust side translates into the
sequence of `run_graph_command(...)` calls already proven by
`run_legacy_http_crossfade_test` (`src/lib.rs:264-425`):

```text
createmixer               (id = mixer-X, config = { width, height, sample-rate })
createsource              (id = src-A, uri = srt://…)
createsource              (id = src-B, uri = srt://…)
createdestination         (id = dst-rtmp-X, family = { Rtmp: { uri = "rtmp://…" } })
connect                   (src-A → mixer-X, config = { video::alpha, video::zorder, audio::volume })
connect                   (src-B → mixer-X, config = { … })
connect                   (mixer-X → dst-rtmp-X)
start                     dst-rtmp-X, mixer-X, src-A, src-B
```

The exact JSON shapes are pinned in **STEP-9**.

After this phase:

- `ui/bridge.slint` declares one new `Panel` variant
  (`Panel.mixer`), two new structs (`SrtSource`, `RtmpDestination`),
  three new `in-out` Bridge properties (`srt-source-a`,
  `srt-source-b`, `rtmp-destination`), one read-only `mixer-canvas`
  struct, and four new Slint → Rust callbacks
  (`start-mixer-cast()`, `stop-mixer-cast()`,
  `apply-mixer-slot-config(slot_id, alpha, zorder, volume)`,
  `apply-mixer-canvas(width, height, sample_rate)`).
- `ui/main.slint` adds one new `if Bridge.active-panel == Panel.mixer`
  branch routing to `MixerPage`.
- `ui/pages/mixer_page.slint` (new file) contains the full screen
  (`MixerPage` + private sub-components `SrtSourceRow`,
  `MixerSlotControls`, `RtmpDestinationRow`).
- A new "Mixer" `SettingsValueRow` is added to `FullSettingsPage` in
  `ui/pages/settings_page.slint`.
- The Rust side (out of scope for this phase — landed in a follow-on
  PHASE-12) registers callback handlers that fan the four callbacks out
  into the migration runtime's `run_graph_command` ladder
  (`src/lib.rs:217-241`).

This phase is **purely additive**: nothing in `src/migration/` is
touched, no existing UI behaviour changes, no callback signatures on
`Bridge` are renamed.

---

## 1. Pre-flight

### 1.1 What already exists (do not re-create)

| Component | Location | Why it matters |
|---|---|---|
| `Bridge` global singleton + re-export pattern | `ui/bridge.slint:139-258`, `ui/main.slint:56-57` | This is the only Slint↔Rust integration surface. New props/callbacks go here. |
| `Panel` enum (the panel router) | `ui/bridge.slint:72-94` | Add `Panel.mixer` here. |
| `FullSettingsPage` + `Panel.none` close pattern | `ui/pages/settings_page.slint:68-…` | Mirror this for `MixerPage`'s back/Done button. |
| `NetworkPage` — multi-row list + per-row sub-component | `ui/pages/network_page.slint` | Closest existing template for "list of N rows that each open / configure an entity." Two SRT source rows reuse this layout. |
| `SettingsSection`, `SettingsValueRow`, `SettingsToggleRow`, `SettingsSliderRow` | `ui/components/settings_rows.slint` | Reuse for the mix controls section (alpha, z-order, volume sliders) and the SRT/RTMP form rows. |
| `PrimaryButton` / `TextButton` / `DestructiveButton` | `ui/components/buttons.slint` | Start / Done / Stop actions. |
| `Theme` design tokens | `ui/theme.slint` | All colors / spacing / radii. |
| `run_graph_command(action, params)` JSON shim | `src/lib.rs:217-241` | Single entry point — every UI action eventually funnels through here. Documented end-to-end in STEP-9. |
| `try_handle_command_json` (in-process, no socket) | `src/migration/runtime.rs:349-356` | The function `run_graph_command` calls. **Already wired** — no migration changes needed. |
| `DestinationFamily::Rtmp { uri }` | `src/migration/protocol.rs:148-150` | The destination shape this phase targets. |
| Working `createmixer` / `connect` config keys (`video::zorder`, `video::alpha`, `video::width`, `video::height`, `video::sizing-policy`) | `src/lib.rs:354-362` (crossfade reference) | Source of truth for the JSON shapes used in STEP-9. |

### 1.2 What needs to change

| File | Edit |
|---|---|
| `ui/bridge.slint` | Add `SrtSource`, `RtmpDestination`, `MixerCanvas` structs; add 3 `in-out` props + 1 `in` prop; add 4 callbacks; add `Panel.mixer` enum variant. **STEP-2 + STEP-3 + STEP-4.** |
| `ui/main.slint` | Add `import { MixerPage } from "pages/mixer_page.slint";` and one new `if Bridge.active-panel == Panel.mixer` branch. **STEP-4.** |
| `ui/pages/settings_page.slint` | Add one `SettingsValueRow { title: @tr("Mixer"); … clicked => { Bridge.active-panel = Panel.mixer; }}` to `FullSettingsPage`. **STEP-4.** |
| `ui/pages/mixer_page.slint` | **New file.** Contains `MixerPage` (the screen), `SrtSourceRow`, `MixerSlotControls`, `RtmpDestinationRow`. **STEP-5 → STEP-8.** |
| `src/lib.rs` | **Out of scope for this phase.** STEP-9 lays out exactly what the Rust handlers must do but does **not** ship them — that work is a follow-up PHASE-12. |
| `src/migration/*` | **Untouched.** This phase pre-supposes the existing protocol surface and adds no new variants. |

### 1.3 Why a single page (not a wizard)

Moblin's SwiftUI equivalents (`SrtlaServerSettingsView.swift`,
`StreamWizardNetworkSetupMyServersRtmpSettingsView.swift`) are
multi-screen `NavigationLink` flows. On Android the
`Bridge.active-panel` router already gives us one panel = one screen,
and the surface is small enough (2 sources + 1 destination + ~6
sliders) to fit in a single `ScrollView`. Splitting into wizard pages
adds 4–6 extra `Panel` variants and corresponding Rust routing, with no
upside — every field is independent.

### 1.4 Why dispatch via Slint callbacks rather than direct
`run_graph_command` calls from Rust handlers wired to `invoke-action`

The four debug quick-actions used to do exactly that (call
`migration::runtime::*` directly inside `on_invoke_action`'s `match
id_str` ladder). PHASE-9 explicitly moved that coupling **out** in
favour of typed Bridge callbacks (`start-migration-server`,
`run-migration-test`, `stop-migration-server` — see
`ui/bridge.slint:251-253` and `src/lib.rs:2138-2185`). PHASE-11 follows
the same convention: every UI → migration interaction is a typed
callback on `Bridge`, never a stringly-typed `invoke-action(id)` round
trip.

### 1.5 Slint-doc reference index

Every step cites the Slint upstream doc that justifies its pattern.
The mirror lives at `draft/slint-ui/docs/` (see
`draft/slint-ui/docs/_MIRROR.md`). Documents referenced across this
phase:

| File (under `draft/slint-ui/docs/astro/src/content/docs/`) | Topic | Used in steps |
|---|---|---|
| `guide/language/coding/globals.mdx` | `global Bridge` + Rust `app.global::<Bridge>()` access | STEP-2, STEP-3, STEP-9 |
| `guide/language/coding/structs-and-enums.mdx` | `struct` and `enum` declarations + default value rules | STEP-2, STEP-4 |
| `guide/language/coding/properties.mdx` | `in` / `out` / `in-out` property qualifiers | STEP-2 |
| `guide/language/coding/functions-and-callbacks.mdx` | `callback name(arg1: type, arg2: type)` declarations | STEP-3 |
| `guide/language/coding/repetition-and-data-models.mdx` | `for item in model : Component { … }` repeated children | STEP-5, STEP-8 |
| `guide/language/coding/positioning-and-layouts.mdx` | `VerticalLayout` / `HorizontalLayout` / `GridLayout` | STEP-5, STEP-6, STEP-7, STEP-8 |
| `guide/language/coding/states.mdx` | `states` + transitions for "connected / connecting / stopped" indicator | STEP-5 (mention only) |
| `guide/development/custom-controls.mdx` | private `component Foo inherits Rectangle { … }` sub-component pattern + `callback foo();` exposure | STEP-5, STEP-6, STEP-7 |
| `guide/development/translations.mdx` | `@tr("…")` user-visible string wrapping | every page step |
| `reference/std-widgets/views/scrollview.mdx` | `ScrollView` + `mouse-drag-pan-enabled` | STEP-8 |
| `reference/std-widgets/views/lineedit.mdx` | `LineEdit` (URL / stream key entry) | STEP-5, STEP-7 |
| `reference/std-widgets/basic-widgets/slider.mdx` | `Slider` `value` / `minimum` / `maximum` / `changed` | STEP-6 |
| `reference/std-widgets/basic-widgets/combobox.mdx` | `ComboBox` (sample-rate / canvas size dropdowns) | STEP-6 |
| `reference/std-widgets/basic-widgets/switch.mdx` | `Switch` (enable / disable source toggle) | STEP-5 |
| `reference/std-widgets/basic-widgets/button.mdx` | `Button` (Start / Stop) — but we use the in-tree `PrimaryButton` wrapper | STEP-8 |

### 1.6 Moblin SwiftUI reference index (design provenance only)

These files are **read-only design references** — they justify the
information architecture, not the code structure. The actual Slint
markup is in the steps below.

| Moblin SwiftUI file | Maps to |
|---|---|
| `draft/moblin-ui/Moblin/View/Settings/Ingests/SrtlaServer/SrtlaServerSettingsView.swift` | `SrtSourceRow` (toggle + port + stream-id) — STEP-5 |
| `draft/moblin-ui/Moblin/View/Settings/Ingests/SrtlaServer/SrtlaServerStreamSettingsView.swift` | per-source URL + status row — STEP-5 |
| `draft/moblin-ui/Moblin/View/Settings/Streams/Stream/Srt/StreamSrtSettingsView.swift` | latency slider on `SrtSourceRow` — STEP-5 |
| `draft/moblin-ui/Moblin/View/Settings/Scenes/Scene/SceneSettingsView.swift` | the "two sources composed onto one canvas" mental model — STEP-6 |
| `draft/moblin-ui/Moblin/View/Settings/Scenes/Widgets/Widget/VideoSource/WidgetVideoSourceSettingsView.swift` | per-slot alpha / z-order / volume controls — STEP-6 |
| `draft/moblin-ui/Moblin/View/Settings/Ingests/RtmpServer/RtmpServerStreamSettingsView.swift` | RTMP URL + stream key row — STEP-7 |
| `draft/moblin-ui/Moblin/View/Settings/Streams/Stream/Wizard/NetworkSetup/MyServers/StreamWizardNetworkSetupMyServersRtmpSettingsView.swift` | RTMP URL validation footer text — STEP-7 |
| `draft/moblin-ui/Moblin/View/Settings/Streams/Stream/StreamSettingsView.swift` | overall "source + destination in one form" — STEP-8 |

### 1.7 Why this phase **does not** modify `src/migration/`

Every command this screen needs already exists in
`src/migration/protocol.rs::Command` and is dispatched by
`NodeManager::dispatch()` (`src/migration/node_manager.rs`):

- `CreateMixer` — `protocol.rs:84-91`
- `CreateSource` — `protocol.rs:57-64`
- `CreateDestination` (with `family: DestinationFamily::Rtmp { uri }`) — `protocol.rs:74-83` + `protocol.rs:148-150`
- `Connect` (with mixer slot config map) — `protocol.rs:92-101`
- `Start` / `Remove` / `Disconnect` — `protocol.rs:102-115`

This phase only writes a **UI** for invoking the existing commands. If
a connect-time config key (e.g. `audio::volume`) turns out to be
missing in `NodeManager::dispatch`'s mixer slot setup, that's a bug to
fix in the migration runtime under a separate phase, **not** in this
one.

---

## 2. Out of scope

- Live preview / video-thumbnail rendering of the two SRT sources.
  Showing a preview would require either a WHEP loopback (PHASE-5
  surface) or a `LocalPlayback` destination, plus a `slint::Image` sink
  for raw frames. Defer to a follow-on phase.
- Cast-history integration (`Bridge.cast-history`,
  `Bridge.cast-history-entries`) — the Mixer screen does not write to
  history in this phase; that wiring belongs with the eventual Rust
  handlers in PHASE-12.
- Multiple simultaneous mixers. The screen owns exactly one mixer at a
  time. Adding "list of mixers" is a separate feature and would
  re-introduce the `for … in Bridge.mixers` patterns from
  `network_page.slint`.
- RTMPS / authenticated RTMP. The destination accepts a single URL
  string; FCast's migration runtime already passes the URL through to
  `rtmp2sink` so `rtmps://…` works at the protocol level, but the
  screen's footer text in STEP-7 documents only the `rtmp://` shape.
- Form-level URL validation (regex, scheme allow-list). Moblin uses
  `isValidUrl(...)` from `Common/Utils.swift`; the Slint side validates
  only "non-empty". A follow-on phase can add a `LineEdit.edited(…)`
  validator + `FormFieldError`-style red label, mirroring
  `StreamWizardNetworkSetupMyServersRtmpSettingsView.swift:14-26`.

---

## 3. Exit criteria

After all 9 steps land:

1. `Bridge.active-panel = Panel.mixer` opens the new screen.
2. The screen shows two `SrtSourceRow`s (A and B) with editable URL,
   latency, stream-id, and enable toggle.
3. The screen shows a `MixerSlotControls` block per slot with alpha
   (0…1), z-order (0…9), volume (0…1) sliders.
4. The screen shows an `RtmpDestinationRow` with URL + stream key.
5. The "Start" button calls `Bridge.start-mixer-cast()`. The "Stop"
   button calls `Bridge.stop-mixer-cast()`. Slider drag end calls
   `Bridge.apply-mixer-slot-config(...)`.
6. With no Rust handlers wired (PHASE-12 not yet landed), all callbacks
   are no-ops at runtime. The screen still renders, all inputs still
   accept text, the `in-out` Bridge props still mutate. **This is the
   intended end-state for this phase** — UI ships first; Rust handlers
   ship in PHASE-12.
7. `ci/ui-validate.sh --no-build` passes (Slint compile, touch-target
   ≥48px, no `Panel.mixer` orphan, no nested `ListView` inside
   `ScrollView`).

---

## 4. Step-by-step

See the linked STEP files. Each step is independently committable and
each commit leaves `cargo build -p android-sender --target
aarch64-linux-android` and `ci/ui-validate.sh --no-build` green.

- **STEP-1** — Pre-flight inventory: confirm everything in §1.1 above
  is present at the expected line numbers; refresh citations if the
  tree has drifted.
- **STEP-2** — Add `SrtSource`, `RtmpDestination`, `MixerCanvas`
  structs and the four new Bridge properties.
- **STEP-3** — Add the four new Bridge callbacks (`start-mixer-cast`,
  `stop-mixer-cast`, `apply-mixer-slot-config`, `apply-mixer-canvas`).
- **STEP-4** — Add `Panel.mixer`, wire the page route in `main.slint`,
  and add the `SettingsValueRow` entry to `FullSettingsPage`.
- **STEP-5** — Build `SrtSourceRow` (private sub-component used twice
  on the page).
- **STEP-6** — Build `MixerSlotControls` (alpha / z-order / volume
  sliders) and the global canvas-config row.
- **STEP-7** — Build `RtmpDestinationRow`.
- **STEP-8** — Compose all sub-components into `MixerPage` and wire
  the header, start / stop buttons, and scrollview shell.
- **STEP-9** — Rust handler reference: spell out the
  `run_graph_command(...)` ladder each callback must execute, with the
  exact JSON shapes. **No Rust code is shipped in this phase** — this
  is the spec PHASE-12 will implement against.

---

## 5. Why all the snippets in this phase are illustrative

Slint version drift is the dominant risk. The repo pins Slint to a
futo fork (`Cargo.toml`) — see `draft/slint-ui/docs/current-fcast-slint-notes.md`.
Snippets in this phase are written against the std-widgets API
documented in the mirror (Slint 1.17) but only use features that
already work in the in-tree files (1.15.1-compatible patterns —
`VerticalLayout`, `HorizontalLayout`, `ScrollView`,
`for x in Bridge.foo`, struct literal field assignment, callback
declarations with named parameters). If a snippet does not compile
against the actual pinned fork, **fix the snippet first** before
declaring the upstream API broken — the same constraint already
applied throughout PHASE-7 / PHASE-8.

---

## 6. References

- `draft/slint-ui/docs/_MIRROR.md` — provenance of the Slint doc mirror.
- `draft/slint-ui/docs/slint-docs-used.md` — running index of upstream
  docs cited from FCast phase docs (extend this when STEP-9 lands).
- `draft/slint-ui/docs/swiftui-to-slint-guide.md` — Moblin SwiftUI →
  FCast Slint pattern dictionary, the source of every "this SwiftUI X
  becomes Slint Y" claim in §1.6.
- `draft/slint-ui/phases/MVP-PHASE-9-debug-bridge-decoupling.md` — the
  precedent for "every UI → migration interaction is a typed Bridge
  callback, never a stringly-typed `invoke-action(id)` round trip"
  (cited from §1.4).
- `draft/slint-ui/phases/MVP-PHASE-10-android-sender-repo-extraction.md`
  — the precedent for parent + STEP-N file split (this phase mirrors
  its structure).
