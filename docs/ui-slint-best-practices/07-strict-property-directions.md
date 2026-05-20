# 07 — Strict property directions; kill the `changed` re-emit anti-pattern

## Goal

For every property on the Bridge globals (`AppBridge`, `MediaBackend`,
`Recording`, etc.), pick the **narrowest valid direction** (`in`, `out`,
`in-out`, `private`) so the Slint compiler enforces the writer
invariants documented in `ui/main.slint:4–22`. Remove the
`changed selected-history-id =>` re-emit pattern from `bridge.slint`,
since it's the textbook example the Slint properties doc warns against.

## Findings

### F8 — `changed` re-emit anti-pattern

`ui/bridge.slint:274–277`:

```slint
callback selected-history-id-changed(string);
changed selected-history-id => {
    Bridge.selected-history-id-changed(Bridge.selected-history-id);
}
```

The intent is clear: "let Rust know when Slint changed
`selected-history-id`". But the implementation re-emits the value via
a callback that fires from inside a `changed` handler — exactly the
pattern called out in
[`properties.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx):

> *Therefore, it's crucial not to overuse changed callbacks.*
>
> ***Warning:*** *Utilize changed callbacks only when an alternative
> through binding isn't feasible.*

It also opens a feedback loop if Rust ever writes back to
`selected-history-id` from the same callback — the change handler
re-fires, calls the callback, Rust may re-set the property, … Slint
breaks the loop after a few iterations but the behaviour is undefined
in the meantime.

### F11 — `in-out` leakage

`ui/main.slint:4–22` documents three invariants:

> *1. `Bridge.active-panel` — Slint pages may write directly. Rust
> writes only through `open_panel(panel)` in lib.rs.*
> *2. `Bridge.lifecycle` — Rust is the authoritative writer
> (set_lifecycle).*
> *3. `Bridge.app-state` — Slint writes ONLY via `change-state(to)`. No
> Slint-side direct writes …*

But all three properties are declared `in-out property`, which means
both Slint and Rust can write *and* read them freely. The invariants
are conventions, not language-enforced. Move the language to enforce
them:

- Properties Rust writes, Slint only reads → `in`
- Properties Slint writes (via change-state), Rust only reads → `out`
- Genuine bidirectional (form-input bindings) → `in-out`
- Component-private state → `private`

Quick triage against the current `bridge.slint`:

| Property                             | Today      | Should be |
| ------------------------------------ | ---------- | --------- |
| `app-state`                          | `in-out`   | `out` (writes only via `change-state` helper) |
| `lifecycle`                          | `in-out`   | `in` (Rust writes, Slint reads, except for `LockOverlay` exit — see below) |
| `active-panel`                       | `in-out`   | `in-out` (genuinely bidirectional, but go through `PanelBridge.push/pop`) |
| `snapshot-secs`                      | `in-out`   | `in-out` (form input)  |
| `banner-message` / `banner-visible` / `banner-severity` | `in-out` | `in` (Rust writes; Slint only reads + the auto-hide timer writes `visible`, which can stay `in-out`) |
| `devices`                            | `in`       | `in` (good) |
| `recording-state`, `recording-elapsed-s` | `in`   | `in` (good) |
| `media-backend-state` / `status-text` / `error-text` | `in` | `in` (good) |
| `gstpop-url`, `gstpop-api-key`, `gstpop-pipeline-id` | `in-out` | `in-out` (form input) |
| `media-backend` (selector enum)      | `in-out`   | `in-out` (form-state) |
| `selected-history-id`                | `in-out`   | `in-out` + drop `changed` (see below) |

### `LockOverlay` writes `lifecycle = LifecycleMode.normal`

`ui/components/lock_overlay.slint:19` does
`Bridge.lifecycle = LifecycleMode.normal` to exit the lock state. If
`lifecycle` becomes `in`, this write fails to compile. The fix is to
introduce an explicit callback:

```slint
// AppBridge
callback exit-lifecycle();   // already exists at line 310 of bridge.slint!
```

…and have `LockOverlay` call `AppBridge.exit-lifecycle()` instead of
writing the property. Rust catches the callback, sets the property.
Same outcome, one writer.

## Slint docs reference

- [`properties.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/properties.mdx)
  — direction qualifiers (`in` / `out` / `in-out` / `private`) and the
  "don't overuse changed callbacks" warning.
- [`functions-and-callbacks.mdx`](../../draft/slint-ui/docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx)
  — the canonical pattern is "Rust binds to a callback" + "Slint
  binds to a property", not "Slint writes a property and emits a
  callback".

## Before — `bridge.slint:215–278`

```slint
in-out property <AppState> app-state: AppState.Disconnected;
in-out property <Panel>    active-panel: Panel.none;

in-out property <LifecycleMode> lifecycle: LifecycleMode.normal;

callback selected-history-id-changed(string);
changed selected-history-id => {
    Bridge.selected-history-id-changed(Bridge.selected-history-id);
}
```

## After — narrowed directions, no `changed` re-emit

```slint
// ui/state/lifecycle.slint  (after step 02)
export global AppBridge {
    // Slint writes via the change-state helper below. Rust reads
    // freely. Direction is `out` so Rust cannot bypass `change-state`.
    out property <AppState> app-state: AppState.Disconnected;

    // Rust writes via lib.rs set_lifecycle. Slint reads — and uses
    // exit-lifecycle() to *request* an exit, never writes directly.
    in property <LifecycleMode> lifecycle: LifecycleMode.normal;

    in-out property <int> snapshot-secs: 5;
    in property <string>  app-version: "";

    // Lifecycle entry / exit callbacks
    callback engage-lock();
    callback engage-stealth();
    callback start-snapshot-countdown(int);
    callback exit-lifecycle();

    callback back-requested();

    // The single Slint-side writer for app-state.
    public function change-state(to: AppState) {
        AppBridge.app-state = to;
    }
}
```

```slint
// ui/state/history.slint  (after step 02)
export global History {
    in property <[CastHistoryEntry]> entries: [];

    // Selection is genuinely bidirectional — both Rust and Slint can
    // set it (Rust: deep-link via push notification; Slint: tap on row).
    in-out property <string> selected-id: "";

    // ── Pure callback contract (Slint → Rust) — REPLACES the
    //    `changed selected-history-id` block in bridge.slint:275-277.
    callback open-detail(entry-id: string);
    callback clear();
    callback delete(entry-id: string);
    callback recast(entry-id: string);

    // Rust reads `selected-id` lazily when populating
    // `selected-entry`; no need for a "did change" callback at all.
    in property <CastHistoryEntry> selected-entry;
}
```

### After — call-site that used to write `selected-history-id`

`ui/pages/cast_history_page.slint` today does:

```slint
// (current)
TouchArea {
    clicked => {
        Bridge.selected-history-id = entry.id;
        Bridge.active-panel = Panel.cast-history-detail;
    }
}
```

…and relies on `changed selected-history-id` to tell Rust about the new
selection. After the fix:

```slint
// (target)
TouchArea {
    clicked => {
        History.open-detail(entry.id);
        PanelBridge.push(Panel.cast-history-detail);
    }
}
```

Rust handles `History.on_open_detail()`, sets `selected-id` (so the
form input still shows the chosen id), populates `selected-entry`, and
the detail page binds to `History.selected-entry`. No `changed` handler
needed.

### After — `LockOverlay` exits via callback, not direct write

`ui/components/lock_overlay.slint` (target):

```slint
// Before
Timer {
    interval: 16ms;
    running: hold-area.pressed;
    triggered => {
        root.hold-elapsed += 16ms;
        if (root.hold-elapsed >= 1.5s) {
            Bridge.lifecycle = LifecycleMode.normal;  // ⚠ writes `in` prop
        }
    }
}

// After
Timer {
    interval: 16ms;
    running: hold-area.pressed;
    triggered => {
        root.hold-elapsed += 16ms;
        if (root.hold-elapsed >= 1.5s) {
            AppBridge.exit-lifecycle();   // call, don't write
        }
    }
}
```

Rust's `lib.rs`:

```rust
ui.global::<AppBridge>().on_exit_lifecycle({
    let ui_weak = ui.as_weak();
    move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.global::<AppBridge>().set_lifecycle(LifecycleMode::Normal);
        }
    }
});
```

(Note: step 10 also rewrites that 16 ms Timer to use `animate` —
applies the same fix; the change here is purely about the property
direction.)

### After — `app-state` writes via `change-state(...)` only

`ui/pages/casting_page.slint`, `connect_page.slint`, etc. already use
`Bridge.change-state(AppState.Casting)` or similar. The change to
`out property <AppState> app-state` enforces that — any forgotten
direct write becomes a compile error.

Rust side already calls a helper to invoke the function:

```rust
// senders/android/src/lib.rs
ui.global::<AppBridge>().invoke_change_state(AppState::Connecting);
// or, equivalently in the new model:
ui.global::<AppBridge>().set_app_state(AppState::Connecting);
// (set_app_state still exists on `out` props for Rust — `out` means
//  "Slint UI users can't write it from inside .slint code", not
//  "Rust can't write it via the generated trait".)
```

## Direction quick reference

Slint property directions are about **Slint-side** access. The Rust-
generated trait always exposes `get_*` and `set_*` for every property
the global owns. The qualifier rules:

| Direction   | Slint can read | Slint can write | Rust can read | Rust can write |
| ----------- | -------------- | --------------- | ------------- | -------------- |
| `private`   | ✓ (within file) | ✓ (within file) | ✗ (not in generated trait) | ✗ |
| `in`        | ✓              | ✗               | ✓             | ✓              |
| `out`       | ✓              | ✓ (within owning component) | ✓             | ✓              |
| `in-out`    | ✓              | ✓               | ✓             | ✓              |

This is what makes the invariants enforceable: declaring `app-state` as
`out` means *external `.slint` files cannot write to it*. The owning
global (`AppBridge`) writes it through its `change-state` helper, and
Rust writes through the generated `set_app_state`. No third path.

## Migration

1. Switch each `bridge.slint` (or, post-step-02, per-feature global)
   property to the narrowest qualifier from the table above.
2. Fix the resulting compile errors at call-sites. Each error is one
   of:
   - **Slint writing `in` property** → introduce a callback, have Rust
     do the write.
   - **Slint writing `out` property from outside the owning global** →
     route through the owning global's `public function` (e.g.
     `AppBridge.change-state`).
   - **Rust reading `private` property** → bump it to `in` or expose
     a Slint-side `out` accessor.
3. Delete the `changed selected-history-id =>` block in
   `bridge.slint:275–277`.
4. Add explicit callbacks (`History.open-detail`, …) for the original
   "Rust observes Slint writes" patterns.
5. Audit other potential `changed` handlers — there is currently one
   more (`lock_overlay.slint:27` `changed pressed =>`). That one is a
   legitimate UI-internal state reset (resets `hold-elapsed` to zero on
   release) and **does not** trigger a callback or write to a Bridge
   property; leave it alone.

### Per-file checklist

| File / global property          | Current        | Target           | Why                                              |
| ------------------------------- | -------------- | ---------------- | ------------------------------------------------ |
| `AppBridge.app-state`           | `in-out`       | `out`            | Slint writes only via `change-state` helper      |
| `AppBridge.lifecycle`           | `in-out`       | `in`             | Rust writes; Slint calls `exit-lifecycle` only   |
| `AppBridge.snapshot-secs`       | `in-out`       | `in-out`         | form input                                       |
| `AppBridge.app-version`         | `in`           | `in`             | already correct                                  |
| `PanelBridge.active`            | `in-out`       | `in-out`         | bidirectional but write only via `push`/`pop`    |
| `PanelBridge.stack`             | (new)          | `private`        | internal state of back-stack helper              |
| `BannerBridge.message` / `severity` / `visible` | `in-out` | `in-out` (visible) / `in` (message + severity) | Rust pushes content; auto-hide flips `visible` |
| `MediaBackend.state`            | `in`           | `in`             | already correct                                  |
| `MediaBackend.kind` / `gstpop-url` / `api-key` / `pipeline-id` | `in-out` | `in-out` | form input |
| `History.selected-id`           | `in-out`       | `in-out`         | bidirectional                                    |
| `History.selected-entry`        | `in`           | `in`             | computed by Rust                                 |
| Drop `selected-history-id-changed` callback + `changed` block | | | replaced by explicit `open-detail(id)` callback |
| `lock_overlay.slint` `Bridge.lifecycle = …` write | direct | `AppBridge.exit-lifecycle()` | match new `in` direction |
| `snapshot_countdown.slint` `Bridge.lifecycle = LifecycleMode.normal` write | direct | `AppBridge.exit-lifecycle()` | same |

## Pitfalls

- **`out` does not mean "compile error if Rust writes".** It means
  "compile error if a `.slint` file *outside* the owning component
  writes". Rust always has both `get` and `set` via the generated
  trait. Don't rely on `out` to lock Rust out — guard with code
  review.
- **`in-out` with one-writer convention** is fine for form state where
  Rust seeds the initial value and Slint mutates. The Bridge split
  (step 02) puts each one in a clearly-named global so the convention
  is obvious in context.
- **`changed` on an `in` property** is **valid** Slint and useful for
  cross-property derivations. The anti-pattern is only the
  *re-emit-as-callback* shape.

## Out of scope

- Detecting "Rust wrote this property" from Slint. Slint does not
  expose a "last writer was Rust vs. Slint" predicate; if you need
  that, design a separate callback contract.
- Adding Rust-side validation that the writer convention is honoured.
  Static enforcement is what the `out` qualifier *is*; runtime
  enforcement is overkill.

## Acceptance

- [ ] `grep -nE '^\s*changed [a-z-]+\s*=>' ui/bridge.slint ui/state/*.slint`
      returns only legitimate UI-internal changes (e.g. `changed
      pressed =>` in `lock_overlay.slint`).
- [ ] No `.slint` file writes `Bridge.lifecycle = …` directly (after
      step 02 this is `AppBridge.lifecycle`).
- [ ] No `.slint` file outside `state/lifecycle.slint` writes
      `AppBridge.app-state` directly. All writes go through
      `AppBridge.change-state(…)`.
- [ ] `cargo check -p android-sender` passes after Rust call-sites are
      adjusted.
