# MVP-PHASE-11 — Step 4: `Panel.mixer` enum + main.slint route + `FullSettingsPage` entry

> Part 4 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-3](./MVP-PHASE-11-STEP-3-bridge-callbacks.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Plumb the new screen into the existing panel router so the user can
actually open it:

1. Add `Panel.mixer` to the `Panel` enum.
2. Import `MixerPage` in `ui/main.slint` and add the
   `if Bridge.active-panel == Panel.mixer : MixerPage { … }` branch.
3. Add a `SettingsValueRow` to `FullSettingsPage` (in
   `ui/pages/settings_page.slint`) that sets
   `Bridge.active-panel = Panel.mixer` on tap.

This step **introduces an import** for a file (`pages/mixer_page.slint`)
that does **not exist yet** — implementers must land STEP-5 (or, more
practically, a minimal stub `MixerPage` component) **in the same
commit** as this step, otherwise `build.rs` will fail.

Recommended commit order: STEP-4 + STEP-8 commit together (STEP-8 is
where `MixerPage` itself is declared); STEP-5/6/7 can land before
STEP-4 because they add private sub-components used only inside
`MixerPage`.

> **Slint-doc reference:**
> [`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
> §Enums — extending an enum is additive.

---

## 1. Add `Panel.mixer`

**File:** `ui/bridge.slint`

Locate the `Panel` enum (`ui/bridge.slint:72-94`, post-STEP-1
verification). Add `mixer,` as a new variant. Position **after**
`network,` so existing call sites match the alphabetical/group order
the file already uses:

```diff
 export enum Panel {
     none,
     settings,
     debug,
     codec-test,
     backup-reset,
     audio,
     camera,
     quick-actions,
     cast-history,
     cast-history-detail,
     recording,
     pairing,
     receiver-rename,
     bitrate-presets,
     bitrate-preset-edit,
     macros,
     macro-edit,
     debug-log,
     debug-video,
     network,
+    mixer,
 }
```

> **Slint-doc reference:** adding an enum variant does not require any
> change to the `export { Panel }` re-export in `ui/main.slint` —
> re-exports publish the type, not the variant set. See
> [`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
> + [`globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
> §"Export a global".

### 1.1 Why position matters

The Slint enum default value is always the **first** variant
([`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
§Enums "The default value of each enum type is always the first
value."). `none` is the first variant, which is what every default
`Bridge.active-panel: Panel.none;` initialization in the tree relies
on. Do **not** insert `mixer` at the top.

---

## 2. Add `MixerPage` import + route

**File:** `ui/main.slint`

### 2.1 Import (top of file, after the existing page imports)

```diff
 import { NetworkPage }               from "pages/network_page.slint";
+import { MixerPage }                 from "pages/mixer_page.slint";
```

> **Slint-doc reference:** module imports use `import { Name } from
> "path";` per
> [`globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
> §"Export a global". A trailing semicolon is required (the file's
> existing `import` lines all carry one).

### 2.2 Route branch

Locate the `if Bridge.active-panel == Panel.network: NetworkPage { … }`
branch inside `MainWindow`'s overlay layer (the routing block sits
under the casting-state switch). Add the new branch immediately after
it:

```diff
     if Bridge.active-panel == Panel.network: NetworkPage {
         width: parent.width;
         height: parent.height;
     }
+    if Bridge.active-panel == Panel.mixer: MixerPage {
+        width: parent.width;
+        height: parent.height;
+    }
```

> **Slint-doc reference:** conditional elements via `if expression : X
> { … }` are documented at
> [`positioning-and-layouts.mdx`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
> §"Conditional elements" (and used throughout the existing tree —
> grep `^\s*if Bridge\.active-panel` in `ui/main.slint`).

### 2.3 Why explicit `width: parent.width; height: parent.height`

The page is rendered outside any layout (it sits on top of the
`MainWindow`'s root), so it must be sized explicitly. Elements outside
layouts need explicit `width`/`height`; elements inside layouts are
sized automatically. See
[`positioning-and-layouts.mdx`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx)
§"Logical pixels and length / px" + the FCast-side note in
`draft/slint-ui/docs/swiftui-to-slint-guide.md` ("Elements outside
layouts need explicit width/height").

---

## 3. Add `SettingsValueRow` to `FullSettingsPage`

**File:** `ui/pages/settings_page.slint`

Locate the `FullSettingsPage` component. Within the
`SettingsSection { title: @tr("NETWORK"); … }` block (or the
equivalent existing settings-section that holds the "Network" row),
add a new row above or below the existing "Network" entry:

```diff
                 SettingsSection {
                     title: @tr("CONNECTIVITY");
                     SettingsValueRow {
                         title: @tr("Network");
                         value: "";
                         clicked => { Bridge.active-panel = Panel.network; }
                     }
+                    SettingsValueRow {
+                        title: @tr("Mixer");
+                        value: "";
+                        clicked => { Bridge.active-panel = Panel.mixer; }
+                    }
                 }
```

The exact section title may differ from `CONNECTIVITY` — the
implementer should drop the row into whichever section "Network" lives
in, so the user finds it next to the existing media-related
configuration entries.

> **Slint-doc reference:** see the `SettingsValueRow` component
> declaration at `ui/components/settings_rows.slint:34-69`. The
> `clicked` callback is invoked by a `TouchArea` inside the row.

---

## 4. Why this step does **not** modify `lib.rs`

`Panel.mixer` is a Slint-only enum variant. The Rust side never
matches `Panel` directly; it only calls
`ui.global::<Bridge>().set_active_panel(Panel::None)` at known reset
points (e.g. after `stop-casting`). Those existing call sites do not
need to learn about `mixer` because they only ever **write** `none`,
never **read** the current value.

If a future phase needs Rust to react to a panel becoming visible
(e.g. start enumerating interfaces when `Panel.network` opens), it
will register a `Bridge::on_active_panel_changed` (a generated `changed`
handler) — and PHASE-11 would extend that. PHASE-11 itself does not
require any such hook.

---

## 5. Expected diff size

- `ui/bridge.slint`: **+1 line** (new enum variant).
- `ui/main.slint`: **+1 import line, +4 route lines**.
- `ui/pages/settings_page.slint`: **+5 lines** (new row).

Total: ~11 lines added, 0 removed.

---

## 6. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

The Slint compiler **must** see `pages/mixer_page.slint` on disk. If
landing STEP-4 standalone for review purposes, ship a one-line
placeholder file:

```slint
// ui/pages/mixer_page.slint — STEP-4 placeholder, real impl lands in STEP-8.
export component MixerPage inherits Rectangle {
    background: red;
    Text { text: "PHASE-11 MixerPage placeholder"; color: white; }
}
```

**Do not commit this placeholder past STEP-8** — STEP-8 replaces it
with the real implementation.

The `ci/ui-validate.sh` Panel-routing audit (see
`ci/ui-validate.sh` §"Panel routing") will fail with:

```
Panel.mixer is set somewhere but no `if Bridge.active-panel == Panel.mixer`
route in main.slint
```

…if the `main.slint` branch in §2.2 is missing, or:

```
Panel.mixer is routed in main.slint but nothing in the rest of the UI
sets `Bridge.active-panel = Panel.mixer`
```

…if the `FullSettingsPage` row in §3 is missing.

---

## 7. Exit gate

- [ ] `Panel.mixer` exists at the **end** of the enum, not the start.
- [ ] `ui/main.slint` imports `MixerPage` and routes
      `Panel.mixer → MixerPage`.
- [ ] `FullSettingsPage` has a "Mixer" row that sets the panel.
- [ ] `cargo build -p android-sender --target aarch64-linux-android`
      succeeds (either because STEP-8's real `MixerPage` has landed, or
      because the §6 placeholder is in place).
- [ ] `ci/ui-validate.sh --no-build` passes.

Proceed to [STEP-5](./MVP-PHASE-11-STEP-5-srt-source-section.md).
