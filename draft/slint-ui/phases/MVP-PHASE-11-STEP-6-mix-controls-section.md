# MVP-PHASE-11 — Step 6: `MixerSlotControls` + canvas-config section

> Part 6 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-5](./MVP-PHASE-11-STEP-5-srt-source-section.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Build two pieces of UI inside `ui/pages/mixer_page.slint`:

1. `MixerSlotControls` — private sub-component used twice (once per
   source) to expose `mix-alpha` (0..1), `mix-zorder` (0..9),
   `mix-volume` (0..1) as `SettingsSliderRow`s. Sliders **only** mutate
   the `in-out` struct's mix-* fields locally; the row also fires
   `Bridge.apply-mixer-slot-config(...)` at drag end so Rust can push
   the new config into the live mixer link.
2. A canvas-config block — three `SettingsSliderRow`s (width, height,
   sample-rate) bound to `Bridge.mixer-canvas.*`. STEP-8 places this
   block above the two `SrtSourceRow`s.

> **Slint-doc references:**
>
> - [`reference/std-widgets/basic-widgets/slider.mdx`](../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/slider.mdx)
>   §Properties (`value`, `minimum`, `maximum`, `step`) + §Callbacks
>   (`changed(value)`).
> - [`reference/std-widgets/basic-widgets/combobox.mdx`](../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/combobox.mdx)
>   §Callbacks (`selected(string)`) — used by §2.5 alternative.
> - [`guide/language/coding/functions-and-callbacks.mdx`](../docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
>   §"It's possible to add parameters to a callback".
> - [`guide/language/coding/properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
>   §"Avoid binding loops" — why we fire the callback explicitly at
>   `changed(v)` and not from a `changed` handler.

---

## 1. `MixerSlotControls` sub-component

Add to `ui/pages/mixer_page.slint` immediately below the
`SrtSourceRow` declaration from STEP-5:

```slint
// Private — mix sliders for one source's contribution to the canvas.
//
// Bound to a single SrtSource struct. Each slider re-writes the
// matching mix-* field on the struct AND invokes
// Bridge.apply-mixer-slot-config so Rust can push the new value into
// the live mixer link (no-op while the graph is idle).
component MixerSlotControls inherits Rectangle {
    in-out property <SrtSource> data;
    in property <string> title;

    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 220px;

    VerticalLayout {
        padding-left:   Theme.padding-screen;
        padding-right:  Theme.padding-screen;
        padding-top:    Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing:        Theme.spacing-default;

        Text {
            text: root.title;
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
        }

        // ── Alpha (0..1) ────────────────────────────────────────────
        SettingsSliderRow {
            title: @tr("Alpha");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> root.data.mix-alpha;
            changed(v) => {
                // Push the NEW value plus the other two unchanged values
                // — `v` is canonical, not `root.data.mix-alpha`, because
                // the binding may not have flushed yet when the callback
                // fires. See properties.mdx §"Two-way bindings".
                Bridge.apply-mixer-slot-config(
                    root.data.slot-id, v,
                    root.data.mix-zorder, root.data.mix-volume);
            }
        }

        // ── Z-order (0..9, integer) ─────────────────────────────────
        // One-way bind only: `SettingsSliderRow.value` is `float`,
        // `data.mix-zorder` is `int`. Two-way `<=>` would require
        // identical types on both sides (see §1.1 below). Write back
        // explicitly inside `changed(v)`, where the float-to-int
        // truncation is well-defined.
        SettingsSliderRow {
            title: @tr("Z-order");
            minimum: 0;
            maximum: 9;
            show-fractional: false;
            value: root.data.mix-zorder;
            changed(v) => {
                root.data.mix-zorder = v;
                Bridge.apply-mixer-slot-config(
                    root.data.slot-id, root.data.mix-alpha,
                    v, root.data.mix-volume);
            }
        }

        // ── Volume (0..1) ───────────────────────────────────────────
        SettingsSliderRow {
            title: @tr("Volume");
            minimum: 0;
            maximum: 1;
            show-fractional: true;
            value <=> root.data.mix-volume;
            changed(v) => {
                Bridge.apply-mixer-slot-config(
                    root.data.slot-id, root.data.mix-alpha,
                    root.data.mix-zorder, v);
            }
        }
    }
}
```

### 1.1 Why `<=>` for floats but one-way for `int`

`SettingsSliderRow.value` is declared as `in-out property <float>` in
`ui/components/settings_rows.slint:102`. Slint's two-way binding
operator `<=>` requires **identical types on both sides** — implicit
`int → float` (read direction) and `float → int` (write-back
direction) coercion only fires for **one-way** bindings (the existing
uses in the tree are all `float <=> float` — see
`ui/pages/bitrate_preset_edit_page.slint:109` with
`draft-kbps: <float>` on both sides).

The `mix-zorder` slider uses **one-way bind + explicit write-back**:
`value: root.data.mix-zorder;` (read direction, with implicit
`int → float`) plus an explicit `root.data.mix-zorder = v;` inside
`changed(v)` (write direction, with implicit `float → int`
truncation). For `mix-alpha` and `mix-volume` (both `float` on both
sides) the `<=>` operator is fine.

> **Slint-doc reference:** numeric type coercion is documented under
> [`guide/language/coding/expressions-and-statements.mdx`](../docs/astro/src/content/docs/guide/language/coding/expressions-and-statements.mdx);
> `int → float` widens automatically. The constraint that two-way
> `<=>` requires matching types is implicit in the existing tree
> (all `<=>` uses pair identical property types — see
> `ui/components/std/slider.slint:21`).

### 1.2 Why pass all four args every time

`Bridge.apply-mixer-slot-config(slot_id, alpha, zorder, volume)` is
positional; STEP-3 §1.2 explains the choice. The page never reads
"which slider changed last" from Rust — Rust just sets the link's
config dict to the new triple. Sending stale-but-still-correct values
for the two non-changing fields is fine because the migration runtime
treats `connect` as idempotent on the same `link_id`.

### 1.3 Why `slot-id` (Rust-owned) is the right link key

`Bridge.srt-source-a.slot-id` is populated by Rust when
`Bridge.start-mixer-cast()`'s `connect` returns success (see STEP-9
§3.2). Before the cast starts, `slot-id == ""`, in which case
`apply-mixer-slot-config` is a Rust-side no-op (handler short-circuits
on empty string). So dragging sliders **before** Start is safe —
they only mutate the Bridge struct; nothing hits the migration
runtime.

---

## 2. Canvas-config block (`MixerCanvasControls`)

Add immediately below `MixerSlotControls`:

```slint
// Canvas size + sample-rate. Bound to Bridge.mixer-canvas. Editing
// these has no immediate effect — they are applied on the next
// `start-mixer-cast()` (STEP-9 §3.7).
component MixerCanvasControls inherits Rectangle {
    background: Theme.surface-card;
    border-radius: Theme.radius-card;
    min-height: 240px;

    callback canvas-edited();

    VerticalLayout {
        padding-left:   Theme.padding-screen;
        padding-right:  Theme.padding-screen;
        padding-top:    Theme.padding-screen;
        padding-bottom: Theme.padding-screen;
        spacing:        Theme.spacing-default;

        Text {
            text: @tr("Canvas");
            color: Theme.text-primary;
            font-size: Theme.font-size-heading;
        }

        // `Bridge.mixer-canvas.*` are all `int`. Same one-way bind +
        // explicit write-back pattern as §1.1 — `<=>` is not used
        // because the slider's `value` is `float`.
        SettingsSliderRow {
            title: @tr("Width");
            unit:  @tr(" px");
            minimum: 320;
            maximum: 3840;
            show-fractional: false;
            value: Bridge.mixer-canvas.width;
            changed(v) => {
                Bridge.mixer-canvas.width = v;
                Bridge.apply-mixer-canvas(
                    v, Bridge.mixer-canvas.height,
                    Bridge.mixer-canvas.sample-rate);
                root.canvas-edited();
            }
        }
        SettingsSliderRow {
            title: @tr("Height");
            unit:  @tr(" px");
            minimum: 240;
            maximum: 2160;
            show-fractional: false;
            value: Bridge.mixer-canvas.height;
            changed(v) => {
                Bridge.mixer-canvas.height = v;
                Bridge.apply-mixer-canvas(
                    Bridge.mixer-canvas.width, v,
                    Bridge.mixer-canvas.sample-rate);
                root.canvas-edited();
            }
        }
        SettingsSliderRow {
            title: @tr("Sample rate");
            unit:  @tr(" Hz");
            minimum: 8000;
            maximum: 48000;
            show-fractional: false;
            value: Bridge.mixer-canvas.sample-rate;
            changed(v) => {
                Bridge.mixer-canvas.sample-rate = v;
                Bridge.apply-mixer-canvas(
                    Bridge.mixer-canvas.width,
                    Bridge.mixer-canvas.height, v);
                root.canvas-edited();
            }
        }

        if Bridge.mixer-canvas.last-error != "": Text {
            text: Bridge.mixer-canvas.last-error;
            color: Theme.error-fg;
            font-size: Theme.font-size-label;
            wrap: word-wrap;
        }
    }
}
```

### 2.1 Why sliders, not `LineEdit`s, for the canvas size

A `LineEdit` would need a parser, an error label, and a validator —
all the things `StreamSrtSettingsView.swift` has to write because
SwiftUI's `TextField` does not constrain to a range. Slint
`SettingsSliderRow` has the range built in and renders the current
value inline; for a UI-only phase that's enough.

If a future phase needs free-form values (e.g. 1920x1200, 96kHz), swap
the slider for a `ComboBox` whose `model` is a fixed set of "common
canvas sizes":

```slint
// Optional follow-on (not in PHASE-11):
ComboBox {
    model: ["854x480", "1280x720", "1920x1080"];
    current-index: 1;   // 1280x720 default
    selected(value) => {
        // parse and dispatch — handler is Rust-side, not in this file.
        Bridge.apply-mixer-canvas-preset(value);
    }
}
```

The `ComboBox.selected(string)` callback signature is documented at
[`reference/std-widgets/basic-widgets/combobox.mdx`](../docs/astro/src/content/docs/reference/std-widgets/basic-widgets/combobox.mdx)
§Callbacks. Adding `apply-mixer-canvas-preset` is a follow-on phase
contract, **not** part of PHASE-11.

### 2.2 Why `Bridge.mixer-canvas.width` (not a local property)

`MixerCanvasControls` does **not** declare a `data` property — it
binds directly to the global. This works because there is exactly one
mixer canvas (not one per source). Slint allows global access from
anywhere inside the same file ([`globals.mdx`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx)
§"Access them using `Name.property`"). For the two `SrtSourceRow`s
this would not work (we need two distinct instances), so STEP-5's
sub-component takes a `data` property — but `MixerCanvasControls` is
singleton.

### 2.3 Field-level read and write on a Bridge struct property

The snippet above does **two** distinct things to
`Bridge.mixer-canvas`:

1. **Reads** a struct field (`value: Bridge.mixer-canvas.width;` —
   binding context, evaluates the field).
2. **Writes** a struct field (`Bridge.mixer-canvas.width = v;` —
   imperative assignment inside `changed(v)`).

Both work because `Bridge.mixer-canvas` is declared `in-out` (STEP-2
§2.5). Field access is the same syntax in both directions, mirroring
the `player.score` access pattern from
[`structs-and-enums.mdx`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx)
(the canonical Slint struct example).

If the **pinned Slint fork** rejects field-level assignment with an
error like `cannot assign to component-level property field` (this
would be a version-specific limitation, not a documented language
feature gap), fall back to whole-struct rewrites:

```slint
changed(v) => {
    Bridge.mixer-canvas = {
        node-id:     Bridge.mixer-canvas.node-id,
        width:       v,
        height:      Bridge.mixer-canvas.height,
        sample-rate: Bridge.mixer-canvas.sample-rate,
        state:       Bridge.mixer-canvas.state,
        last-error:  Bridge.mixer-canvas.last-error,
    };
    Bridge.apply-mixer-canvas(
        v, Bridge.mixer-canvas.height,
        Bridge.mixer-canvas.sample-rate);
}
```

Document the regression in
`draft/slint-ui/docs/current-fcast-slint-notes.md` and proceed.

> **Slint-doc reference:**
> [`properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
> §"Properties" — `in-out` properties may be both read and written; the
> guide does not call out struct-field assignment specifically because
> it is a natural consequence of struct property access (no special
> grammar).

---

## 3. Expected diff size

About **140 lines added** to `ui/pages/mixer_page.slint` (two
sub-components × ~70 lines each).

---

## 4. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

Two failure modes specific to this step:

1. **Slider value-type mismatch:** if `SettingsSliderRow.value` is
   `float` and a `changed(v) => …` handler tries to bind `v` to an
   `int` Bridge field without casting, Slint errors with `cannot
   assign expression of type float to int`. Fix: cast at the call
   site, e.g. `Bridge.apply-mixer-canvas(v.round(), …)` — or, since
   `int` is widened to `float` and back, the slider's int field
   accepts `v` directly.
2. **Binding loop:** if a `changed` handler writes the *same* property
   it is bound to (`changed(v) => { root.data.mix-alpha = v; … }`),
   Slint emits a binding-loop warning. **Do not** add an assignment
   inside `changed` — the two-way binding has already updated the
   property. See
   [`properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
   §"Change callback warnings".

---

## 5. Exit gate

- [ ] `MixerSlotControls` declared, takes `in-out property <SrtSource>
      data`, renders three sliders.
- [ ] `MixerCanvasControls` declared, binds directly to
      `Bridge.mixer-canvas.*`.
- [ ] Every slider's `changed(v)` handler dispatches via
      `Bridge.apply-mixer-slot-config(...)` or
      `Bridge.apply-mixer-canvas(...)`.
- [ ] No `changed` handlers fire on `Bridge.in` properties (only on
      `in-out` ones).
- [ ] `cargo build` passes.
- [ ] `ci/ui-validate.sh --no-build` passes.

Proceed to [STEP-7](./MVP-PHASE-11-STEP-7-rtmp-destination-section.md).
