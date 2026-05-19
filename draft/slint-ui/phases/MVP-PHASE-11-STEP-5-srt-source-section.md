# MVP-PHASE-11 — Step 5: `SrtSourceRow` private sub-component

> Part 5 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-4](./MVP-PHASE-11-STEP-4-panel-routing.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Build the **private** `SrtSourceRow` sub-component used twice (once for
`Bridge.srt-source-a`, once for `Bridge.srt-source-b`) inside
`MixerPage` (assembled in STEP-8). The row exposes:

- a header with enable toggle (`Switch`) + connection status text
  (derived from `data.state`),
- URL field (`LineEdit`),
- latency slider (`SettingsSliderRow`, 0..8000 ms),
- stream-id field (`LineEdit`),
- an inline error label visible when `data.last-error != ""`.

The row uses the same `inherits Rectangle` + `in property <SrtSource>
data` + private-component pattern as
`ui/pages/network_page.slint`'s `NetworkInterfaceRow`.

> **Slint-doc reference:** private sub-components +
> `in property <SrtSource>` carrying a struct value are documented at
> [`guide/development/custom-controls.mdx`](../docs/astro/src/content/docs/guide/development/custom-controls.mdx)
> and [`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx).

---

## 1. The change

**File:** `ui/pages/mixer_page.slint` (new file, also touched by
STEP-6/7/8 — all four steps may be folded into a single commit if
preferred).

### 1.1 Top of file (imports)

```slint
// mixer_page.slint — Mixer screen: two SRT sources mixed onto one canvas,
// pushed to one RTMP destination via the src/migration runtime.
// PHASE-11 — see draft/slint-ui/phases/MVP-PHASE-11-srt-mix-rtmp-screen.md
//
// Slint docs ref (sub-components, callbacks):
//   draft/slint-ui/docs/astro/src/content/docs/guide/development/custom-controls.mdx
// Slint docs ref (structs):
//   draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx
// Slint docs ref (LineEdit):
//   draft/slint-ui/docs/astro/src/content/docs/reference/std-widgets/views/lineedit.mdx
// Slint docs ref (Switch):
//   draft/slint-ui/docs/astro/src/content/docs/reference/std-widgets/basic-widgets/switch.mdx

import { LineEdit, Switch, ScrollView } from "std-widgets.slint";
import { Bridge, Panel, SrtSource, RtmpDestination,
         MixerCanvas, MixerState } from "../bridge.slint";
import { Theme } from "../theme.slint";
import { PrimaryButton, TextButton, DestructiveButton } from "../components/buttons.slint";
import {
    SettingsSection,
    SettingsValueRow,
    SettingsToggleRow,
    SettingsSliderRow,
} from "../components/settings_rows.slint";
```

### 1.2 Private `SrtSourceRow` component

```slint
// Private — only used inside MixerPage. Same pattern as
// NetworkInterfaceRow in ui/pages/network_page.slint.
component SrtSourceRow inherits Rectangle {
    // Two-way: the row mutates `data.uri`, `data.latency-ms`,
    // `data.stream-id`, `data.enabled` directly via two-way bindings.
    // The `data.slot-id`, `data.state`, `data.last-error` fields are
    // Rust-owned and never written from inside the row.
    in-out property <SrtSource> data;

    // Title shown above the row body (e.g. "Source A" / "Source B").
    in property <string> title;

    // Slot id slot identifier ("a" / "b") — only used to pre-fill the
    // generated graph node id if the user has not entered one. Not the
    // same thing as `data.slot-id` (which is a Rust-owned graph link
    // id, not a UI label).
    in property <string> slot-label;

    callback edited();   // fires whenever the page should treat the
                         // row as "dirty" — currently no consumer in
                         // PHASE-11, kept as an extension point for
                         // PHASE-12 (e.g. dim the global Start button
                         // until edits stop).

    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 200px;

    VerticalLayout {
        padding-left:   Theme.padding-screen;
        padding-right:  Theme.padding-screen;
        padding-top:    Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing:        Theme.spacing-default;

        // ── Header: title + enable + connection status ──────────────
        HorizontalLayout {
            spacing: Theme.spacing-default;
            Text {
                text: root.title;
                color: Theme.text-primary;
                font-size: Theme.font-size-heading;
                vertical-alignment: center;
                horizontal-stretch: 1;
            }
            // Status badge — text derived from data.state.
            Text {
                text:
                    root.data.state == MixerState.idle     ? @tr("idle") :
                    root.data.state == MixerState.starting ? @tr("starting") :
                    root.data.state == MixerState.running  ? @tr("running") :
                    root.data.state == MixerState.stopping ? @tr("stopping") :
                                                              @tr("error");
                color:
                    root.data.state == MixerState.running  ? Theme.success :
                    root.data.state == MixerState.error    ? Theme.error-fg :
                                                              Theme.text-secondary;
                font-size: Theme.font-size-label;
                vertical-alignment: center;
            }
            Switch {
                checked <=> root.data.enabled;
                toggled() => { root.edited(); }
            }
        }

        // ── URL (LineEdit) ──────────────────────────────────────────
        Text {
            text: @tr("URL");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            // SrtSourceRow uses LineEdit (not the SettingsTextRow
            // wrapper) because the field needs to be editable inline,
            // not a push-to-edit nav row. Same choice
            // RtmpDestinationRow makes in STEP-7.
            placeholder-text: @tr("srt://relay.example:9710?mode=caller");
            text <=> root.data.uri;
            edited(text) => { root.edited(); }
        }

        // ── Latency slider ──────────────────────────────────────────
        // One-way bind: `SettingsSliderRow.value` is `float`,
        // `data.latency-ms` is `int`. Same caveat documented in
        // STEP-6 §1.1. Write back explicitly inside `changed(v)`.
        SettingsSliderRow {
            title: @tr("Latency");
            unit:  @tr(" ms");
            minimum: 0;
            maximum: 8000;
            show-fractional: false;
            value: root.data.latency-ms;
            changed(v) => {
                root.data.latency-ms = v;
                root.edited();
            }
        }

        // ── Stream-id (LineEdit, optional) ──────────────────────────
        Text {
            text: @tr("Stream ID (optional)");
            color: Theme.text-secondary;
            font-size: Theme.font-size-label;
        }
        LineEdit {
            placeholder-text: @tr("publish:my-stream-key");
            text <=> root.data.stream-id;
            edited(text) => { root.edited(); }
        }

        // ── Error footer (visible only when something is wrong) ─────
        if root.data.last-error != "": Text {
            text: root.data.last-error;
            color: Theme.error-fg;
            font-size: Theme.font-size-label;
            wrap: word-wrap;
        }
    }
}
```

> **Slint-doc references:**
>
> - `Switch.toggled()`:
>   [`reference/std-widgets/basic-widgets/switch.mdx`](../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/switch.mdx)
>   §Callbacks.
> - `LineEdit.text` is `in-out`; the two-way binding `<=>` is the
>   canonical pattern for syncing UI text to a Bridge property. See
>   [`reference/std-widgets/views/lineedit.mdx`](../docs/astro/src/content/docs/reference/std-widgets/views/lineedit.mdx)
>   §Properties → `text`.
> - `LineEdit.edited(text)` fires on every keystroke; we use it to
>   bubble a single `edited()` signal up to `MixerPage` so the page
>   can dim the Start button.
> - `if cond: Element { … }` is the conditional-element form from
>   [`positioning-and-layouts.mdx`](../docs/astro/src/content/docs/guide/language/coding/positioning-and-layouts.mdx).
> - `SettingsSliderRow` already wraps the `Slider` std widget and
>   forwards `changed(v)`; see
>   [`reference/std-widgets/basic-widgets/slider.mdx`](../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/slider.mdx)
>   for the underlying widget surface (`value`, `minimum`, `maximum`,
>   `changed`).

### 1.3 Why `LineEdit` not `SettingsTextRow`

`SettingsTextRow` (in `ui/components/settings_rows.slint`) is a
**display-only** label + subtitle. The Moblin reference for the SRT
URL field is `TextEditNavigationView` (push-to-edit secondary screen)
— but that is a SwiftUI pattern that wraps every field in its own
navigation push. On a single-screen Slint page, inline `LineEdit` is
simpler and matches what `StreamWizardNetworkSetupMyServersRtmpSettingsView.swift`
does for RTMP URL entry (see `draft/moblin-ui/.../Wizard/.../StreamWizardNetworkSetupMyServersRtmpSettingsView.swift:30-40`).

### 1.4 Why two-way bindings (`<=>`) not bound assignment (`=`)

Two-way `<=>` is the only way to keep `data.uri` and the `LineEdit`'s
internal text buffer in sync **bidirectionally**. With a one-way
`text: root.data.uri;`, the Rust side could push a new URL into the
Bridge struct and the field would update — but typing in the field
would *not* propagate to the struct. See
[`properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
§"Two-way bindings".

### 1.5 Touch-target audit

`ci/ui-validate.sh` flags any clickable element with `(min-)height
<48px`. The `SrtSourceRow` body has `min-height: 200px;` and every
internal widget (`Switch`, `LineEdit`, `SettingsSliderRow`) wraps the
std-widget defaults which are already ≥48px. No extra padding needed.

---

## 2. Why no `states { … }` block

The status text in §1.2 is computed inline (`text: root.data.state ==
MixerState.idle ? … : …`). This is simpler than a `states { idle {
… } running { … } }` block because there is only one property being
state-driven (`text`) and no transitions are needed (snap-to-target is
fine for a text label).

`states` is the right pattern when **multiple** properties change
together (e.g. a card animating its size, color, and content opacity
simultaneously) — see
[`guide/language/coding/states.mdx`](../docs/astro/src/content/docs/guide/language/coding/states.mdx)
for the canonical example. For our single-field status indicator,
inline ternary is idiomatic.

---

## 3. Expected diff size

About **90 lines added** to `ui/pages/mixer_page.slint` (mostly the
`SrtSourceRow` body + comments). STEP-6 / STEP-7 / STEP-8 extend the
same file; their additions are independent of this one.

---

## 4. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

The compiler must accept the `<=>` bindings against `SrtSource.uri`,
`SrtSource.latency-ms`, `SrtSource.stream-id`, `SrtSource.enabled`.
If a field name is misspelled, the error message will be:

```
error: Cannot assign to 'data.uriX' (no such property on type SrtSource)
```

If the Switch's `toggled()` callback is missing the `()` argument list,
the parser will emit:

```
expected '(' after callback name
```

…because Slint's callback declaration grammar requires the parameter
list even when empty.

---

## 5. Exit gate

- [ ] `SrtSourceRow` declared as `component … inherits Rectangle` (not
      `export component` — it must stay private to the file).
- [ ] `enabled`, `uri`, `stream-id` are two-way-bound (`<=>`) to
      `data.*`. `latency-ms` is one-way bound + written back in
      `changed(v)` (the int/float coercion caveat from STEP-6 §1.1).
- [ ] The status text branch covers all five `MixerState` variants.
- [ ] The error footer is conditional on `data.last-error != ""`.
- [ ] `cargo build` passes.
- [ ] `ci/ui-validate.sh --no-build` passes.

Proceed to [STEP-6](./MVP-PHASE-11-STEP-6-mix-controls-section.md).
