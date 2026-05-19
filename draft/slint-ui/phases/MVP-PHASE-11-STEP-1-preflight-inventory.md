# MVP-PHASE-11 — Step 1: Pre-flight inventory

> Part 1 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
>
> **Doc-only.** This step is a checklist — confirm each row exists at
> the cited `file:line`, refresh line numbers if the tree has drifted,
> and only then proceed to STEP-2.

---

## 0. Goal of this step

Confirm every prerequisite the parent doc named in §1.1 still exists in
the live tree, at the exact line numbers cited, **before** any
implementer starts editing `bridge.slint`. If a row has drifted, fix
the citation in the parent + downstream STEPs first; otherwise STEP-2
through STEP-9 will reference stale line numbers and reviewers will
have to chase them by hand.

This step adds **zero lines** to the source tree.

---

## 1. Checklist

For each row, run the `grep` (or `read` with line range) command, eyeball
the result, and update the parent doc if drift is found.

### 1.1 Bridge global + re-export

```sh
sed -n '139,155p' ui/bridge.slint
```

Expected: `export global Bridge { … }` block opens around line 139.

```sh
grep -n "import { Bridge" ui/main.slint
```

Expected: `import { Bridge, AppState, Panel } from "bridge.slint";`
followed by `export { Bridge, AppState, Panel }`.

> **Slint-doc reference:** globals + re-export pattern is
> [`guide/language/coding/globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
> §"Export a global to make it accessible from other files".

### 1.2 `Panel` enum

```sh
grep -n "^export enum Panel\|^    none,\|^    network," ui/bridge.slint | head -5
```

Expected: `Panel` enum declared in `ui/bridge.slint:72-94` with members
`none`, `settings`, `debug`, …, `network`. STEP-4 will add `mixer` to
this enum.

> **Slint-doc reference:**
> [`guide/language/coding/structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
> §Enums.

### 1.3 `Bridge.active-panel` writer/reader pattern

```sh
grep -n "active-panel" ui/bridge.slint ui/main.slint ui/pages/settings_page.slint
```

Expected matches:

- `ui/bridge.slint` — `in-out property <Panel> active-panel: Panel.none;`
- `ui/main.slint` — multiple `if Bridge.active-panel == Panel.X` branches.
- `ui/pages/settings_page.slint` — `clicked => { Bridge.active-panel = Panel.X; }` set-sites.

### 1.4 `FullSettingsPage` and the close-panel pattern

```sh
grep -n "FullSettingsPage\|Bridge.active-panel = Panel.none" ui/pages/settings_page.slint
```

Expected: `export component FullSettingsPage` declared (around the line
`width: 100%; height: 100%; background: Theme.surface-primary;`), with
a header containing a `TextButton { label: @tr("close-panel-button" =>
"Done"); clicked => { Bridge.active-panel = Panel.none; } }`. STEP-4
+ STEP-8 reuse this exact close-button markup.

### 1.5 `NetworkPage` as multi-row template

```sh
sed -n '1,30p' ui/pages/network_page.slint
sed -n '110,160p' ui/pages/network_page.slint
```

Expected:

- `component NetworkInterfaceRow inherits Rectangle { … }` declared
  privately (not `export`-ed) — this is the model for the private
  `SrtSourceRow`, `MixerSlotControls`, `RtmpDestinationRow` declared in
  STEP-5/6/7.
- `for iface in Bridge.network-interfaces: NetworkInterfaceRow { … }`
  inside `NetworkPage` — model for STEP-8 if the implementer prefers a
  data-driven repeat over two explicit rows.

> **Slint-doc reference:**
> [`guide/development/custom-controls.mdx`](../docs/astro/src/content/docs/guide/development/custom-controls.mdx)
> §"Private sub-components" plus
> [`guide/language/coding/repetition-and-data-models.mdx`](../docs/astro/src/content/docs/guide/language/coding/repetition-and-data-models.mdx).

### 1.6 Reusable components

```sh
grep -n "^export component" ui/components/settings_rows.slint ui/components/buttons.slint
```

Expected:

| Component | File |
|---|---|
| `SettingsTextRow`     | `ui/components/settings_rows.slint` |
| `SettingsValueRow`    | `ui/components/settings_rows.slint` |
| `SettingsToggleRow`   | `ui/components/settings_rows.slint` |
| `SettingsSliderRow`   | `ui/components/settings_rows.slint` |
| `SettingsSection`     | `ui/components/settings_rows.slint` |
| `PrimaryButton`       | `ui/components/buttons.slint` |
| `TextButton`          | `ui/components/buttons.slint` |
| `DestructiveButton`   | `ui/components/buttons.slint` |
| `LoadingView`         | `ui/components/buttons.slint` |

All of these are stable since PHASE-3 and are used unchanged here.

### 1.7 Theme tokens

```sh
grep -n "^    out property" ui/theme.slint | head -20
```

Expected (non-exhaustive): `surface-primary`, `surface-card`,
`text-primary`, `text-secondary`, `accent`, `accent-active`,
`accent-pressed`, `error`, `warning`, `success`, `padding-screen`,
`spacing-default`, `radius-card`, `font-size-heading`,
`font-size-body`, `font-size-label`, `row-height`.

STEP-5–8 use these directly without proposing any new tokens.

### 1.8 Migration JSON command API surface

```sh
sed -n '50,120p' src/migration/protocol.rs
sed -n '148,168p' src/migration/protocol.rs
```

Expected: `pub enum Command { … }` carries (at minimum) `CreateSource`,
`CreateMixer`, `CreateDestination`, `Connect`, `Start`, `Remove`,
`Disconnect`, `GetInfo`; `DestinationFamily` carries an `Rtmp { uri }`
variant.

```sh
sed -n '217,242p' src/lib.rs
```

Expected: `fn run_graph_command(action: &str, params: Value) ->
Result<Value, String>` exists. This is the single function STEP-9 will
spec the Rust handlers against.

### 1.9 Working crossfade reference (non-RTMP but same shape)

```sh
sed -n '283,365p' src/lib.rs
```

Expected: the crossfade test calls `createmixer` → `createdestination`
(family `LocalPlayback`) → `connect` → `start` → `createvideogenerator`
→ `connect (with config = { video::zorder, video::alpha, video::width,
video::height, video::sizing-policy })` → `start`. This is the **only
working example** of mixer + connect-config in the tree; STEP-9
reproduces its shape verbatim, swapping `LocalPlayback` for
`{ "Rtmp": { "uri": "rtmp://…" } }`.

### 1.10 Bridge re-exports from `main.slint`

```sh
grep -n "export { Bridge" ui/main.slint
```

Expected: `export { Bridge, AppState, Panel }` exists.  STEP-4 does
**not** need to extend this line — the existing re-export covers the
new `Panel.mixer` variant automatically because `Panel` is the same
enum type.

> **Slint-doc reference:**
> [`guide/language/coding/structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
> — adding a variant to an `enum` does not require updating any
> re-export site.

---

## 2. Drift fix-up procedure

If any of the §1 line numbers have shifted:

1. Open the parent doc
   ([`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md))
   and update the citation table in §1.1.
2. Open every downstream STEP file (STEP-2 → STEP-9) and search for
   the same `file:line` citation. Update in place.
3. **Do not** edit `ui/`, `src/`, or any other source file. This phase
   is doc-only; line-number drift is a doc bug, not a source bug.

If a **component** is missing entirely (e.g. `SettingsSliderRow`
disappears between this phase being written and being landed), stop
here and file a separate phase to restore it — STEP-6 hard-depends on
it.

---

## 3. Exit gate

Before declaring STEP-1 complete:

- [ ] Every row in §1.1–§1.10 verified against the live tree.
- [ ] Any drift propagated through the parent + every STEP file.
- [ ] `ci/ui-validate.sh --no-build` still passes (this should be a
      no-op — STEP-1 ships zero code).

Proceed to [STEP-2](./MVP-PHASE-11-STEP-2-bridge-data-model.md).
