# MVP-PHASE-12 — Step 3: Bridge callbacks (Slint → Rust)

> Part 3 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-2](./MVP-PHASE-12-STEP-2-bridge-data-model.md).
> Next: [STEP-4](./MVP-PHASE-12-STEP-4-settings-page-section.md).

---

## 0. Goal of this step

Declare the three **Slint → Rust** callbacks that the new settings
page (STEP-4) fires, and that the Rust lifecycle handler (STEP-8)
implements. Following the PHASE-9 convention, callbacks are **typed
and named** — never the stringly-typed `Bridge.invoke-action(string)`
pattern.

This step adds no UI, no Rust code, and no behaviour change — only
callback declarations. The callbacks compile cleanly without
handlers; firing one before STEP-8 wires it emits a warning to the
Android log and is otherwise harmless.

---

## 1. New callbacks

Add to **`ui/bridge.slint`** just below the PHASE-9 migration
callbacks (`ui/bridge.slint:249-253`):

```slint
    // ── Media backend selector callbacks (MVP-PHASE-12) ────────────────
    // STEP-4's page fires these; STEP-8's Rust handlers implement them.

    // Persist the current values of media-backend / gstpop-url /
    // gstpop-api-key / gstpop-pipeline-id to disk via the existing
    // settings store (see STEP-8 §3). Does not flip the live backend
    // selector — that's apply-media-backend's job.
    callback save-media-backend-settings();

    // Probe the *currently selected* backend without making it live.
    //   - Migration:  call try_handle_command_json("{\"getinfo\":{}}")
    //                 and surface node count + state on success.
    //   - gst-pop:    open a temporary WebSocket, send a single
    //                 get_version request, close.
    // Writes media-backend-state, media-backend-status-text, and on
    // failure media-backend-error-text. Safe to call repeatedly.
    callback probe-media-backend();

    // Persist (as save-media-backend-settings does) **and** swap the
    // process-wide backend selector to the value of Bridge.media-backend.
    // Triggers a probe as a side effect. Use this on the Apply button.
    callback apply-media-backend();
```

> **Slint-doc reference for callback declaration syntax:**
> [`functions-and-callbacks.mdx §"Callbacks"`](../docs/astro/src/content/docs/guide/language/coding/functions-and-callbacks.mdx).
> The empty parameter list is the same shape as
> `Bridge.stop-migration-server()` at `ui/bridge.slint:253`.

### 1.1 Why three callbacks, not one

A single `apply-media-backend(MediaBackendKind, string, string, string)`
callback would force every UI change to round-trip a typed enum + 3
strings, while a "Save without applying" button could not exist. The
chosen split mirrors the PHASE-7 settings convention where individual
toggles persist eagerly but section-level state (backend selector,
preset selector) require an explicit Apply.

### 1.2 Why callbacks read state from the global rather than taking arguments

Each handler reads `Bridge.media-backend`, `Bridge.gstpop-url`,
`Bridge.gstpop-api-key`, and `Bridge.gstpop-pipeline-id` directly from
Rust (`ui.global::<Bridge>().get_media_backend()`, etc.). That matches
PHASE-9's `start-migration-server(bind-addr)` handler, which **does**
take the one variable that is page-specific (the bind address) but
reads everything else from the global. For PHASE-12 there is no
"single dominant parameter" — all four fields are equally important —
so the consistent thing is to take none and read all from the global.

> **Slint-doc reference for global access from Rust:**
> [`globals.mdx §"Rust"`](../docs/astro/src/content/docs/guide/language/coding/globals.mdx) —
> `app.global::<Bridge>().get_property()` / `set_property(value)`.

---

## 2. No `apply-media-backend` race condition

A reader might worry: "What if the user mashes Apply twice in 100 ms?
Won't the two handlers race on the global backend selector?"

The lifecycle handler in STEP-8 acquires a `parking_lot::Mutex` around
the selector before mutating. Each call to `apply-media-backend()`:

1. Spawns a worker thread (UI thread is never blocked).
2. Inside the worker, takes the selector lock.
3. Writes the new `BackendKind` + spawns a probe.
4. Releases the lock.

Two near-simultaneous fires linearise on the mutex — the second one
overwrites the first's choice (last-write-wins, which is what the
user actually intended).

---

## 3. No callback for "user changed the toggle but hasn't pressed
Apply yet"

The PHASE-7 convention is that toggles update their `in-out` property
synchronously and *don't* notify Rust until Apply is pressed. Bound
to `MediaBackendKind`, this means:

- User flips toggle → `Bridge.media-backend` mutates → STEP-4's status
  badge text updates ("Apply to commit").
- User presses Apply → `apply-media-backend()` fires → Rust reads
  `Bridge.media-backend`, swaps the live selector, probes.

There is no `changed media-backend => { ... }` block on the Bridge
property. Adding one would force every drag through the toggle to
hammer Apply, which is exactly the UX we're trying to avoid.

---

## 4. Final placement

After STEP-2 + STEP-3, `ui/bridge.slint`'s tail reads:

```slint
    // ── Migration-runtime callbacks (MVP-PHASE-9) ───────────────────────
    callback start-migration-server(string);
    callback run-migration-test(string);
    callback stop-migration-server();

    // ── Media backend selector callbacks (MVP-PHASE-12) ────────────────
    callback save-media-backend-settings();
    callback probe-media-backend();
    callback apply-media-backend();

    // ── Public functions (callable from Slint) ───────────────────────────
    public function change-state(to: AppState) {
        Bridge.app-state = to;
    }
}
```

---

## 5. Expected diff size

~10 lines added to `ui/bridge.slint`.

---

## 6. Verification

```sh
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

Callbacks without registered handlers compile cleanly. The Slint
runtime logs a single warning on the first fire of an unhandled
callback (see `ui/bridge.slint:251-253`'s migration callbacks during
the gap between PHASE-9 STEP-1 and STEP-2).

---

## 7. Exit gate

- [ ] All three callbacks are declared with the exact signatures in §1.
- [ ] No `changed media-backend => { ... }` block was accidentally
      added.
- [ ] `cargo build` and `ci/ui-validate.sh` both pass.

Proceed to [STEP-4](./MVP-PHASE-12-STEP-4-settings-page-section.md).
