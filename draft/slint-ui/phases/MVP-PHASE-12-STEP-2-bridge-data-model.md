# MVP-PHASE-12 — Step 2: Bridge data model — `MediaBackend` enum + properties

> Part 2 of 9. Parent doc:
> [`MVP-PHASE-12-gstpop-backend-toggle.md`](./MVP-PHASE-12-gstpop-backend-toggle.md).
> Previous: [STEP-1](./MVP-PHASE-12-STEP-1-preflight-inventory.md).
> Next: [STEP-3](./MVP-PHASE-12-STEP-3-bridge-callbacks.md).

---

## 0. Goal of this step

Add the **Slint-side data model** that the Settings page (STEP-4) and
the Rust handlers (STEP-8) read and write. This step adds **no
callbacks** (STEP-3 does that) and **no UI** (STEP-4 does that) — just
the enum + 6 `Bridge` properties + one `Panel` variant.

Splitting data-model from callbacks keeps the diff small enough that
each commit lands cleanly on a passing CI.

---

## 1. New enums

Add to **`ui/bridge.slint`** near the existing `Panel` enum
(`ui/bridge.slint:70-91`):

```slint
// Which media-pipeline backend is currently driving the cast.
//
// `migration` (default) means the in-process node-graph engine under
// src/migration/runtime.rs; `gst-pop` means the out-of-process
// gst-pop daemon reachable over WebSocket. See
// draft/slint-ui/phases/MVP-PHASE-12-gstpop-backend-toggle.md.
export enum MediaBackendKind {
    migration,
    gst-pop,
}

// Lifecycle of the *currently selected* backend's connection.
// `disconnected` means the user has flipped the toggle but Apply
// hasn't been pressed yet (or it was reset on a reload).
// `probing`     means a probe RPC is in flight.
// `ready`       means the most recent probe (or any successful RPC)
//               returned without an error.
// `error`       means the last operation failed; details in
//               Bridge.media-backend-error-text.
export enum MediaBackendState {
    disconnected,
    probing,
    ready,
    error,
}
```

> **Slint-doc reference for enum declaration:**
> [`structs-and-enums.mdx §"Enums"`](../docs/astro/src/content/docs/guide/language/coding/structs-and-enums.mdx).
> The syntax mirrors `BannerSeverity` and `AppState` already declared
> in `ui/bridge.slint`.

### 1.1 Why two enums (not one combined state machine)

`MediaBackendKind` is the user's choice (rendered as a toggle in the
settings page); `MediaBackendState` is the connection lifecycle
(rendered as a status dot). Conflating them would force the page to
re-derive "is the user currently selecting migration?" from a
combined `(Migration|Migration-Ready|GstPop|GstPop-Probing|…)` enum,
which the existing PHASE-7 settings rows are not shaped for.

---

## 2. New `Panel` variant

Append `media-backend` to the existing `Panel` enum
(`ui/bridge.slint:70-91`):

```slint
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
    media-backend,    // ← STEP-4 routes Bridge.active-panel == Panel.media-backend
}
```

> **Why append (not insert at start):** existing
> `if Bridge.active-panel == Panel.X` branches in `ui/main.slint:107-135`
> bind by name, not by ordinal — appending is safe — but appending
> also avoids reshuffling diffs for unrelated panels in code review.

---

## 3. New `Bridge` properties

Add these to **`ui/bridge.slint`** at the end of the property block,
just above the migration callbacks (`ui/bridge.slint:249-253`):

```slint
    // ── Media backend selector (MVP-PHASE-12) ───────────────────────────
    // The active backend. Defaults to migration so first-launch
    // behaviour is unchanged.
    in-out property <MediaBackendKind> media-backend: MediaBackendKind.migration;

    // The lifecycle of the *selected* backend's connection.
    // Owned by Rust — Slint reads only. Initial state is disconnected
    // because the migration backend, while in-process, is also lazy
    // (start_graph_runtime is only called on first command — see
    // src/migration/runtime.rs:302).
    in property <MediaBackendState> media-backend-state: MediaBackendState.disconnected;

    // Human-readable status line. Rust pushes things like
    // "gst-pop v0.2.0 — 3 pipelines, 0 errors" or
    // "Migration runtime ready — nodes=2".
    in property <string> media-backend-status-text: "";

    // Last error line shown under the status when state == error.
    // Cleared (set to "") on any successful subsequent operation.
    in property <string> media-backend-error-text: "";

    // gst-pop daemon URL. Editable by the user; persisted by STEP-8.
    in-out property <string> gstpop-url: "ws://127.0.0.1:9000";

    // gst-pop optional API key. Editable by the user (masked input
    // in STEP-4). Persisted by STEP-8.
    in-out property <string> gstpop-api-key: "";

    // Pipeline id the app binds to inside the gst-pop daemon. STEP-7
    // dispatches every method against this id by default; STEP-9 lets
    // the user override per-test.
    in-out property <string> gstpop-pipeline-id: "0";
```

### 3.1 Property qualifier matrix (why `in` vs `in-out`)

| Property | Qualifier | Owner | Why |
|---|---|---|---|
| `media-backend` | `in-out` | Slint writes via toggle, Rust reads on Apply | Two-way is required because the settings page mutates it before persisting. |
| `media-backend-state` | `in` | Rust writes on probe/start/stop, Slint reads for status dot | UI never sets the state — only the lifecycle handler does. |
| `media-backend-status-text` | `in` | Rust writes after each successful op | Same reason — Slint never composes this string. |
| `media-backend-error-text` | `in` | Rust writes when state transitions to `error` | Same. |
| `gstpop-url` | `in-out` | Slint writes via `LineEdit`, Rust reads on Apply / probe | Same as `media-backend`. |
| `gstpop-api-key` | `in-out` | Slint writes via masked `LineEdit` | Same. |
| `gstpop-pipeline-id` | `in-out` | Slint writes via `LineEdit`, Rust reads on every dispatch | Same. |

> **Slint-doc reference:**
> [`properties.mdx §"Properties"`](../docs/astro/src/content/docs/guide/language/coding/properties.mdx) —
> `in`, `out`, `in-out`, and `private` qualifiers.

### 3.2 Defaults rationale

- `media-backend: migration` — keeps first-launch behaviour unchanged.
- `media-backend-state: disconnected` — true until the user presses
  Probe (the migration backend is lazy and the gst-pop one needs a
  network probe).
- `gstpop-url: "ws://127.0.0.1:9000"` — the gst-pop daemon's default
  bind ([gstpop/daemon/README.md §"Daemon Options"](https://github.com/dabrain34/gstpop/blob/main/daemon/README.md#daemon-options)).
  On Android this works when the user runs `adb reverse tcp:9000 tcp:9000`
  or uses a phone running on the same Wi-Fi as a desktop daemon.
- `gstpop-api-key: ""` — gst-pop only requires the `Authorization`
  header **if** the daemon was started with `--api-key` (or the
  `GSTPOP_API_KEY` env var). Empty string means "send no Authorization
  header" — STEP-6's client uses `Option<String>` and only adds the
  header when `Some(non-empty)`.
- `gstpop-pipeline-id: "0"` — gst-pop assigns ids starting at `"0"`.
  When the app creates a pipeline it gets a fresh id back; STEP-7's
  selector writes the returned id into this property.

---

## 4. Final placement

After this step, the relevant region of `ui/bridge.slint` reads:

```slint
// … existing enums …
export enum MediaBackendKind { migration, gst-pop }
export enum MediaBackendState { disconnected, probing, ready, error }

export enum Panel {
    none, settings, debug, codec-test, backup-reset, audio, camera,
    quick-actions, cast-history, cast-history-detail, recording,
    pairing, receiver-rename, bitrate-presets, bitrate-preset-edit,
    macros, macro-edit, debug-log, debug-video, network,
    media-backend,    // ← new
}

// … existing struct + global declarations …

export global Bridge {
    // … existing properties …

    // ── Media backend selector (MVP-PHASE-12) ───────────────────────────
    in-out property <MediaBackendKind>  media-backend: MediaBackendKind.migration;
    in     property <MediaBackendState> media-backend-state: MediaBackendState.disconnected;
    in     property <string>            media-backend-status-text: "";
    in     property <string>            media-backend-error-text:  "";
    in-out property <string>            gstpop-url:                "ws://127.0.0.1:9000";
    in-out property <string>            gstpop-api-key:            "";
    in-out property <string>            gstpop-pipeline-id:        "0";

    // ── Migration-runtime callbacks (MVP-PHASE-9) ───────────────────────
    callback start-migration-server(string);
    callback run-migration-test(string);
    callback stop-migration-server();

    // … (callbacks for MVP-PHASE-12 land in STEP-3) …
}
```

---

## 5. Expected diff size

~40 lines added to `ui/bridge.slint` (2 enums + 1 Panel variant + 7
properties + ASCII section heading).

---

## 6. Verification

```sh
# Both must pass after STEP-2 even though no UI consumes the new
# properties yet — Slint allows declared-but-unused properties.
cargo build -p android-sender --target aarch64-linux-android
ci/ui-validate.sh --no-build
```

---

## 7. Exit gate

- [ ] `MediaBackendKind` and `MediaBackendState` are declared as
      top-level `export enum`s.
- [ ] `Panel.media-backend` appears as the last variant.
- [ ] All 7 new properties are present with the qualifiers in §3.1.
- [ ] `cargo build` and `ci/ui-validate.sh` both pass.

Proceed to [STEP-3](./MVP-PHASE-12-STEP-3-bridge-callbacks.md).
