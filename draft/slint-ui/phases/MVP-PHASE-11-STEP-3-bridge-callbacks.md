# MVP-PHASE-11 — Step 3: Bridge callbacks (Slint → Rust)

> Part 3 of 9. Parent doc:
> [`MVP-PHASE-11-srt-mix-rtmp-screen.md`](./MVP-PHASE-11-srt-mix-rtmp-screen.md).
> Previous: [STEP-2](./MVP-PHASE-11-STEP-2-bridge-data-model.md).
>
> **Doc-only.** Snippets are illustrative — no source-tree files are
> modified by reading this step.

---

## 0. Goal of this step

Declare the four new callbacks on the `Bridge` global that the new
Mixer screen will use as its **only** path into the migration runtime.
Same convention as MVP-PHASE-9 (`start-migration-server` /
`run-migration-test` / `stop-migration-server` — see
`ui/bridge.slint:251-253`): every UI → migration interaction is a typed
Bridge callback, never a stringly-typed `invoke-action(id)` round trip.

After this step, the Rust side compiles cleanly (callbacks without
handlers compile but emit a Slint warning the first time they fire —
this is the same trade-off PHASE-9 STEP-1 accepted; see
`MVP-PHASE-9-STEP-1-bridge-callbacks.md` §3.1 for why).

This step is **declaration-only** — no Rust handler ships in this
phase. The PHASE-12 follow-on (and the spec in STEP-9 below) is where
`on_start_mixer_cast` / `on_stop_mixer_cast` /
`on_apply_mixer_slot_config` / `on_apply_mixer_canvas` get wired.

> **Slint-doc reference:**
> [`guide/language/coding/functions-and-callbacks.mdx`](../docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
> §Callbacks (the `callback name(arg1: type, …)` syntax with named
> arguments).

---

## 1. The change

**File:** `ui/bridge.slint`

Inside the `export global Bridge { … }` block, immediately after the
PHASE-9 migration-runtime callbacks (`callback start-migration-server`
/ `callback run-migration-test` / `callback stop-migration-server` at
`ui/bridge.slint:251-253`), add:

```slint
    // ── PHASE-11 — Mixer screen callbacks (Slint → Rust) ────────────
    //
    // Lifecycle:
    //   start-mixer-cast()  → Rust reads srt-source-a, srt-source-b,
    //                         rtmp-destination, mixer-canvas, runs the
    //                         createsource/createmixer/createdestination/
    //                         connect/start sequence (STEP-9 §3) and
    //                         publishes the resulting slot ids /
    //                         MixerState back to the matching `in-out`
    //                         struct fields.
    //
    //   stop-mixer-cast()   → Rust issues `remove` for every live
    //                         node id stashed on the four structs (or
    //                         `disconnect` + `remove`, see STEP-9 §3.6)
    //                         and walks Bridge.mixer-state back to idle.
    //
    //   apply-mixer-slot-config(slot_id, alpha, zorder, volume)
    //     → Rust issues a `connect` (re-)dispatch for the given
    //       link_id with config = { "video::alpha": alpha,
    //                                "video::zorder": zorder,
    //                                "audio::volume": volume }.
    //       The exact mapping is in STEP-9 §3.5. The slot_id argument
    //       is the value of `srt-source-{a,b}.slot-id`, *not* a UI
    //       string — the page passes it verbatim.
    //
    //   apply-mixer-canvas(width, height, sample_rate)
    //     → Rust issues a (re-)`createmixer` if `Bridge.mixer-state
    //       == idle`, otherwise stashes the values and applies them at
    //       next Start. STEP-9 §3.7.
    //
    // All four callbacks fire on the UI thread; Rust handlers must
    // hand off to a worker thread before touching the migration
    // runtime (matching PHASE-9 STEP-2's `std::thread::spawn` pattern
    // in src/lib.rs:2149-2174 for `run-migration-test`).
    callback start-mixer-cast();
    callback stop-mixer-cast();
    callback apply-mixer-slot-config(string, float, int, float);
    callback apply-mixer-canvas(int, int, int);
```

> **Slint-doc reference:** the positional-args form `callback
> name(type1, type2, …)` is what
> [`functions-and-callbacks.mdx`](../docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
> §"It's possible to add parameters to a callback" documents. **Named**
> args (`callback name(slot_id: string, alpha: float, …)`) are also
> valid and recommended for self-documentation, but the rest of
> `bridge.slint` (e.g. `callback invoke-action(string)`,
> `callback set-interface-enabled(string, bool)`) uses the positional
> form, so PHASE-11 follows suit for consistency.

### 1.1 Why no `start-mixer-cast(string)` argument list

Every input the mixer needs (URLs, latency, stream-key, slot configs,
canvas size) is already on a `Bridge.in-out` property by STEP-2. The
Rust handler reads the structs back via `ui.global::<Bridge>().get_…()`
rather than threading 12 arguments through one callback. Same pattern
as PHASE-9 `stop-migration-server()` (no args) vs.
`start-migration-server(string)` (one arg, only because the bind
address is *not* on a Bridge property).

### 1.2 Why `apply-mixer-slot-config` takes 4 explicit args

The slider drag callsite (STEP-6 §2.3) needs to push values **at drag
end**, not on every frame of the drag. Reading
`Bridge.srt-source-a.mix-alpha` is fine for paint-time data flow
(Slint sees the prop change and re-renders the slider thumb); but
firing `apply-mixer-slot-config` from inside a `Slider.changed(value)`
handler means the value the handler receives is the **new** value, and
the slot_id needs to be plumbed in from the enclosing component. So:

```slint
SettingsSliderRow {
    title: @tr("Alpha");
    minimum: 0; maximum: 1; show-fractional: true;
    value <=> root.data.mix-alpha;
    changed(v) => {
        Bridge.apply-mixer-slot-config(
            root.data.slot-id, v,
            root.data.mix-zorder, root.data.mix-volume);
    }
}
```

vs the alternative of stuffing four sliders' deltas into one
"apply-config" hand-off with no arguments and forcing Rust to diff the
old struct against the new — uglier, racier, and harder to test.

### 1.3 Why `apply-mixer-canvas` is **not** an automatic
property-change side-effect

Slint does support `changed` handlers on `in-out` properties:

```slint
in-out property <int> w: 1280;
// implicit: changed w => { Bridge.apply-mixer-canvas(self.w, …); }
```

…but the
[`properties.mdx`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx)
guide explicitly warns against them ("Avoid binding loops; prefer
direct property bindings over `changed` handlers" — also echoed in
`draft/slint-ui/docs/swiftui-to-slint-guide.md` §Slint key patterns).
Firing a Rust call from a `changed` handler races with Rust's setter,
which Slint also fires `changed` for. The page calls the callback
explicitly at the canvas-row's `accepted` / `edited` site (STEP-6 §2.5).

---

## 2. Expected diff size

About **8 lines of callback declarations + 30 lines of comments**
added to `ui/bridge.slint`. No lines removed. No other files touched.

---

## 3. Verification

```sh
ci/ui-validate.sh --no-build
```

Slint will compile cleanly even without registered Rust handlers
(`on_start_mixer_cast`, etc.). The first time any of the four
callbacks fires at runtime *without* a registered handler, Slint logs
a warning to stderr; that warning is harmless because PHASE-11's exit
criterion explicitly allows "Rust handlers not yet wired" (see parent
§3 row 6).

To prove the callbacks are reachable from Rust at compile time (a
nice optional sanity check), add a temporary line to
`src/lib.rs:run_event_loop`:

```rust
ui.global::<Bridge>().on_start_mixer_cast(|| {
    tracing::warn!("Bridge.start-mixer-cast called but no handler wired (PHASE-11; handler ships in PHASE-12)");
});
```

…then `cargo build`. **Remove the line before committing** — PHASE-11
is doc-only on the Rust side.

---

## 4. Exit gate

- [ ] All four callback declarations exist in `ui/bridge.slint` after
      the PHASE-9 callbacks.
- [ ] Slint compiler accepts the file (`build.rs` succeeds).
- [ ] No Rust handler is shipped in this phase.
- [ ] `ci/ui-validate.sh --no-build` passes.

Proceed to [STEP-4](./MVP-PHASE-11-STEP-4-panel-routing.md).
