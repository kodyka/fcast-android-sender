# 08 — Typed models & enums; kill stringly-typed dispatch

## Goal

Replace stringly-typed discriminators with Slint enums. The two biggest
sites are `NetworkInterface.kind` (compared against `"wifi"`, `"ethernet"`,
`"cellular"`, `"loopback"`) and `QuickAction.id` (compared against
`"settings"`, `"debug"`, …). Both routes are interpreted at multiple
call-sites and break silently if a value drifts. Also: subdivide the
flat 22-variant `Panel` enum into a typed group for clarity (low
priority but easy).

## Findings

### F12 — `NetworkInterface.kind` is a string

`ui/bridge.slint:157–163`:

```slint
export struct NetworkInterface {
    name:        string,
    kind:        string,    // "wifi" / "ethernet" / "cellular" / "loopback"
    address-v4:  string,
    address-v6:  string,
    enabled:     bool,
}
```

`ui/pages/network_page.slint:59–63`:

```slint
text:
    root.data.kind == "wifi"     ? "W" :
    root.data.kind == "ethernet" ? "E" :
    root.data.kind == "cellular" ? "M" :
                                   "•";
```

Problems:

- If Rust pushes `"WiFi"` (different case), the comparison fails
  silently and renders `"•"`.
- Adding a new kind (`"vpn"`) needs a coordinated update in Rust + the
  Slint page; the compiler can't help.
- Future filters or grouping by kind require parsing strings.

### F12 (cont.) — `QuickAction.id` is a string used as a switch

`ui/components/control_bar.slint:66–76`:

```slint
invoked(id) => {
    if (id == "settings")    { Bridge.active-panel = Panel.settings;   return; }
    if (id == "debug")       { Bridge.active-panel = Panel.debug;      return; }
    if (id == "codec-test")  { Bridge.active-panel = Panel.codec-test; return; }
    if (id == "record")      { Bridge.active-panel = Panel.recording;  return; }
    if (id == "pair")        { Bridge.active-panel = Panel.pairing;    return; }
    if (id == "bitrate")     { Bridge.active-panel = Panel.bitrate-presets; return; }
    Bridge.invoke-action(id);
}
```

Plus the macro-edit page hard-codes an action catalogue:

`ui/pages/macro_edit_page.slint:26–33`:

```slint
property <[{action-id: string, label: string}]> available-actions: [
    { action-id: "scan-qr",        label: @tr("Scan QR") },
    { action-id: "audio",          label: @tr("Open Audio") },
    { action-id: "camera",         label: @tr("Open Camera") },
    { action-id: "record",         label: @tr("Start Recording") },
    { action-id: "stop-recording", label: @tr("Stop Recording") },
    { action-id: "stop-cast",      label: @tr("Stop Cast") },
];
```

These two lists must stay in sync but are completely disconnected.

### F19 — `Panel` is one flat 22-variant enum

`ui/bridge.slint:90–113`:

```slint
export enum Panel {
    none,
    settings,        debug,           codec-test,        backup-reset,
    audio,           camera,          quick-actions,
    cast-history,    cast-history-detail,
    recording,       pairing,          receiver-rename,
    bitrate-presets, bitrate-preset-edit,
    macros,          macro-edit,
    debug-log,       debug-video,
    network,         mixer,            media-backend,
}
```

Slint enums are flat, no nesting, so we can't have
`Panel::Settings::Main` / `Panel::Settings::Network`. The fix is
naming-discipline: prefix grouped variants with a shared stem so an
auditor scanning the list can see "this is the settings family", e.g.
`settings-root`, `settings-network`, `settings-audio`, `settings-camera`,
`settings-backup-reset`, `settings-bitrate-presets`,
`settings-bitrate-preset-edit`, etc.

## Slint docs reference

- [`structs-and-enums.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
  — `enum Foo { a, b, c }`, used as a value or as a struct field.
- [`repetition-and-data-models.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/repetition-and-data-models.mdx)
  — `for item in model` against a typed array, including enum fields.

## Before — `NetworkInterface` stringly-typed

```slint
export struct NetworkInterface {
    name:        string,
    kind:        string,    // "wifi" / "ethernet" / "cellular" / "loopback"
    address-v4:  string,
    address-v6:  string,
    enabled:     bool,
}
```

## After — `NetworkKind` enum + typed struct

```slint
// ui/state/types.slint  (or bridge.slint after step 02)
export enum NetworkKind {
    wifi,
    ethernet,
    cellular,
    loopback,
    other,
}

export struct NetworkInterface {
    name:        string,
    kind:        NetworkKind,
    address-v4:  string,
    address-v6:  string,
    enabled:     bool,
}
```

Rust generation in `senders/android/src/lib.rs` becomes:

```rust
let interfaces: Vec<NetworkInterface> = enumerate_interfaces()
    .into_iter()
    .map(|n| NetworkInterface {
        name:       n.name.into(),
        kind:       match n.kind {
            InterfaceKind::Wifi     => NetworkKind::Wifi,
            InterfaceKind::Ethernet => NetworkKind::Ethernet,
            InterfaceKind::Cellular => NetworkKind::Cellular,
            InterfaceKind::Loopback => NetworkKind::Loopback,
            InterfaceKind::Other    => NetworkKind::Other,
        },
        address_v4: n.address_v4.unwrap_or_default().into(),
        address_v6: n.address_v6.unwrap_or_default().into(),
        enabled:    n.enabled,
    })
    .collect();
ui.global::<Network>().set_interfaces(ModelRc::new(VecModel::from(interfaces)));
```

…and `ui/pages/network_page.slint` uses the enum:

```slint
// network_page.slint (target)
Rectangle {
    width: 32px; height: 32px;
    border-radius: 8px;
    background: Theme.accent-active.darker(20%);

    Text {
        // Glyph dispatched on the typed enum — no quoted strings.
        text:
            root.data.kind == NetworkKind.wifi     ? @tr("network-kind-glyph-wifi",     "W")
            : root.data.kind == NetworkKind.ethernet ? @tr("network-kind-glyph-ethernet", "E")
            : root.data.kind == NetworkKind.cellular ? @tr("network-kind-glyph-cellular", "M")
            : root.data.kind == NetworkKind.loopback ? @tr("network-kind-glyph-loopback", "L")
            : @tr("network-kind-glyph-other", "•");

        color: Theme.text-primary;
        horizontal-alignment: center;
        vertical-alignment: center;
        font-size: Theme.font-size-body;
    }
}
```

…or, applying step 06 (states):

```slint
component KindGlyph inherits Text {
    in property <NetworkKind> kind: NetworkKind.other;
    color: Theme.text-primary;
    horizontal-alignment: center;
    vertical-alignment: center;
    font-size: Theme.font-size-body;

    states [
        wifi     when root.kind == NetworkKind.wifi     : { text: @tr("network-kind-glyph-wifi",     "W"); }
        ethernet when root.kind == NetworkKind.ethernet : { text: @tr("network-kind-glyph-ethernet", "E"); }
        cellular when root.kind == NetworkKind.cellular : { text: @tr("network-kind-glyph-cellular", "M"); }
        loopback when root.kind == NetworkKind.loopback : { text: @tr("network-kind-glyph-loopback", "L"); }
        other    when root.kind == NetworkKind.other    : { text: @tr("network-kind-glyph-other",    "\u2022"); }
    ]
}
```

## Before — `QuickAction.id` is a free-form string

```slint
export struct QuickAction {
    id:      string,
    title:   string,
    enabled: bool,
    active:  bool,
    is-macro: bool,
}
```

## After — `QuickActionKind` enum with a `macro-id` payload field

```slint
export enum QuickActionKind {
    open-settings,
    open-debug,
    open-codec-test,
    open-recording,
    open-pairing,
    open-bitrate,
    open-audio,
    open-camera,
    open-network,
    open-media-backend,
    open-mixer,
    open-quick-actions,
    open-cast-history,
    scan-qr,
    start-record,
    stop-record,
    stop-cast,
    run-macro,           // payload: macro-id
    custom,              // payload: custom-id (extension hook)
}

export struct QuickAction {
    kind:     QuickActionKind,
    macro-id: string,      // populated iff kind == run-macro
    custom-id: string,     // populated iff kind == custom
    title:    string,
    enabled:  bool,
    active:   bool,
}
```

`control_bar.slint` dispatch becomes:

```slint
invoked(action) => {
    if action.kind == QuickActionKind.open-settings {
        PanelBridge.push(Panel.settings); return;
    }
    if action.kind == QuickActionKind.open-debug {
        PanelBridge.push(Panel.debug); return;
    }
    if action.kind == QuickActionKind.open-codec-test {
        PanelBridge.push(Panel.codec-test); return;
    }
    if action.kind == QuickActionKind.open-recording {
        PanelBridge.push(Panel.recording); return;
    }
    if action.kind == QuickActionKind.open-pairing {
        PanelBridge.push(Panel.pairing); return;
    }
    if action.kind == QuickActionKind.open-bitrate {
        PanelBridge.push(Panel.bitrate-presets); return;
    }
    if action.kind == QuickActionKind.run-macro {
        Macros.run(action.macro-id); return;
    }
    if action.kind == QuickActionKind.start-record {
        Recording.start(); return;
    }
    // … etc …
    Quickbar.invoke(action.kind, action.custom-id);
}
```

`macro_edit_page.slint` available-actions becomes:

```slint
property <[{kind: QuickActionKind, label: string}]> available-actions: [
    { kind: QuickActionKind.scan-qr,      label: @tr("Scan QR") },
    { kind: QuickActionKind.open-audio,   label: @tr("Open Audio") },
    { kind: QuickActionKind.open-camera,  label: @tr("Open Camera") },
    { kind: QuickActionKind.start-record, label: @tr("Start Recording") },
    { kind: QuickActionKind.stop-record,  label: @tr("Stop Recording") },
    { kind: QuickActionKind.stop-cast,    label: @tr("Stop Cast") },
];
```

…and `MacroStep` gains a typed action field:

```slint
export struct MacroStep {
    kind:    QuickActionKind,
    macro-id: string,
    label:   string,
}
```

(`label` stays — it's the display name for the step's row in the edit
list, distinct from the `@tr()` looked up by `kind`. If you prefer,
make it computed in Rust and drop the field; for the guide we keep it.)

## Before — flat `Panel` enum

```slint
export enum Panel {
    none,
    settings, debug, codec-test, backup-reset,
    audio, camera, quick-actions,
    cast-history, cast-history-detail,
    recording, pairing, receiver-rename,
    bitrate-presets, bitrate-preset-edit,
    macros, macro-edit,
    debug-log, debug-video,
    network, mixer, media-backend,
}
```

## After — group by stem prefix; same enum, better names

```slint
export enum Panel {
    none,

    // Settings family
    settings-root,
    settings-backup-reset,
    settings-audio,
    settings-camera,
    settings-network,
    settings-quick-actions,
    settings-bitrate-presets,
    settings-bitrate-preset-edit,
    settings-media-backend,
    settings-cast-history,
    settings-cast-history-detail,
    settings-macros,
    settings-macro-edit,

    // Debug family
    debug-root,
    debug-codec-test,
    debug-log,
    debug-video,

    // Modal / cast-flow panels (not under Settings)
    recording,
    pairing,
    receiver-rename,
    mixer,
}
```

Touch-up:

- `Bridge.active-panel = Panel.settings` → `Panel.settings-root`.
- The 22 `if PanelBridge.active == Panel.<x>` arms in `PanelHost`
  rename automatically; everything else is grep-replace.
- Rust enums mirror the new names (`Panel::SettingsRoot`,
  `Panel::SettingsAudio`, etc.).

This is purely cosmetic / documentation, but it pays off the next time
you grep the codebase for "everything under settings".

## Migration

1. **Bottom-up**: introduce the enum (`NetworkKind`, `QuickActionKind`)
   first, update Rust to populate the new field, leave the struct's
   `string` field in place during transition.
2. **Slint reads from both** for one PR — `data.kind-new` (enum) is
   added alongside `data.kind` (string). All page comparisons swap to
   the enum.
3. Once nothing reads the string, drop it from the struct.
4. Repeat for `QuickAction.id` → `kind` + `macro-id` / `custom-id`.
5. Rename `Panel` variants last — affects ~50 `Bridge.active-panel =
   Panel.x` writes, all easy to find with `grep`.

### Per-file checklist

| File / type                          | Add                                  | Drop later             |
| ------------------------------------ | ------------------------------------ | ---------------------- |
| `ui/state/types.slint` (or `bridge.slint`) | `enum NetworkKind`, `enum QuickActionKind` | n/a            |
| `NetworkInterface`                   | `kind: NetworkKind`                  | `kind: string`         |
| `QuickAction`                        | `kind: QuickActionKind`, `macro-id`, `custom-id` | `id: string` |
| `MacroStep`                          | `kind: QuickActionKind`              | `action-id: string`    |
| `ui/pages/network_page.slint`        | enum compare + `KindGlyph` component | string compares        |
| `ui/components/control_bar.slint`    | enum dispatch                        | string compares        |
| `ui/pages/macro_edit_page.slint`     | enum-typed `available-actions`       | string-typed table     |
| `senders/android/src/lib.rs`         | populate enum fields                 | string fields          |
| `senders/android/src/quick_actions.rs` (or wherever) | `QuickActionKind` ↔ enum dispatch | string dispatch |

## Out of scope

- Slint sum types (Rust-style `enum X { A(i32), B(String) }`). Slint
  enums are unit variants only — payload fields live alongside on the
  struct, as `macro-id` and `custom-id` do above. The discriminant +
  payload-by-convention pattern is sufficient.
- A typed-action library (matching every action to a typed payload).
  Step at a time.
- Renaming `Panel` if you're not ready for the grep churn. The other
  parts of this step stand on their own.

## Acceptance

- [ ] `git grep -nE '"wifi"|"ethernet"|"cellular"|"loopback"' ui/`
      returns no hits.
- [ ] `git grep -nE '"settings"|"debug"|"codec-test"' ui/components ui/pages`
      returns no hits.
- [ ] `git grep -F 'id: string,' ui/bridge.slint ui/state/types.slint`
      returns no hits (action-id has been replaced by typed `kind`).
- [ ] Network page renders the right glyph per kind in
      `slint-viewer`.
