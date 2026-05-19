# MVP-PHASE-12 — Backend selector: `src/migration/` ⇄ gst-pop daemon (WebSocket)

> **Parent doc** for the MVP-PHASE-12 step series. The screen and the
> trait surface this guide describes are *additions* — no existing
> behaviour changes until the user flips the toggle in **Settings →
> Media backend**.

---

## 0. Goal

Give the user a single switch in the Settings panel that selects which
**media-pipeline engine** the app uses for outbound streaming:

| Backend | Where it runs | What it speaks |
|---|---|---|
| **Migration** (current default) | In-process Rust (`src/migration/runtime.rs`) | Externally-tagged `Command` enum over a thin JNI bridge (`src/lib.rs:217-241` — `run_graph_command(action, params)`) |
| **gst-pop** (this phase) | Out-of-process daemon: `gst-pop daemon` on `ws://<host>:<port>` (default `ws://127.0.0.1:9000`) | JSON-RPC 2.0 over a WebSocket — `create_pipeline { description }`, `set_state { pipeline_id, state }`, `update_pipeline`, `remove_pipeline`, `play|pause|stop`, plus broadcast events (`state_changed`, `error`, `eos`, …) |

The two backends are **not interchangeable at the command level** —
migration is a **node-graph** (atomic ops on individual nodes —
sources, mixers, destinations connected via links); gst-pop is a
**pipeline-string** engine (one opaque `gst-launch`-style string per
pipeline). The phase introduces a **`MediaBackend` Rust trait** that
exposes the *small* set of operations the app actually drives from the
UI today (probe / start a cast / stop a cast / list active pipelines)
and routes calls to whichever backend the user has selected.

> **Quoting the request:** *"I want to create an independent UI so
> that I can switch between the src/migration/ subsystem and the
> gst-pop (GstPrinceOfParser) daemon with websockets in the
> settings."*  This phase implements the **switch + the daemon
> adapter**; existing migration-driven flows (PHASE-9 quick actions,
> PHASE-11 mixer) are not retargeted yet — that lands incrementally
> in a later phase as call sites move from `run_graph_command` to
> `Backend::current().dispatch(...)`.

---

## 1. Vendoring strategy: thin client, not a submodule

The gst-pop repo ships as a Rust workspace plus a Meson C build. For
this phase we **do not vendor the whole repo**; we write a ~120-line
adapter against the WebSocket protocol (already public and stable —
see [`gstpop/daemon/README.md`](https://github.com/dabrain34/gstpop/blob/main/daemon/README.md)).
Reasoning:

1. The gst-pop daemon is intended to run **out-of-process** (Docker
   image at `ghcr.io/dabrain34/gstpop`, or a `systemd` service on a
   relay box, or a dev-machine `cargo run --bin gst-popd`). It is not
   embedded into Android sender APKs.
2. Pulling the entire daemon's GStreamer-binding stack (`gstreamer`,
   `libsoup-3`, `json-glib`, `readline`, `meson`) into the sender's
   Cargo graph would add ~30 MB of build artefacts for code we do not
   exercise.
3. The protocol is a stable JSON-RPC 2.0 + broadcast-events surface
   ([`daemon/README.md §WebSocket API`](https://github.com/dabrain34/gstpop/blob/main/daemon/README.md#websocket-api)) — speaking it requires only
   `tokio-tungstenite` and `serde_json` (both small).

A future phase can switch from "WebSocket adapter" to "in-process
crate dependency" by listing `gst-pop = { git = "https://github.com/dabrain34/gstpop", subdirectory = "daemon" }`
in `Cargo.toml` and calling `gstpop::daemon::PipelineManager` directly
— the trait surface defined in STEP-5 is the same either way.

---

## 2. Pre-flight inventory (what already exists)

| Surface | File / line | Status |
|---|---|---|
| Migration subsystem (in-process node-graph engine) | `src/migration/` | Stable — `Command` enum at `protocol.rs:50-127`, runtime at `runtime.rs:302-356` |
| JNI bridge call site (`run_graph_command(action, params)`) | `src/lib.rs:217-241` | Stable since PHASE-9 |
| Slint Bridge global (single source of truth for UI ↔ Rust state) | `ui/bridge.slint:143-258` | Stable |
| Settings page chrome (`FullSettingsPage`) | `ui/pages/settings_page.slint:67-269` | Stable since PHASE-7 |
| Reusable settings rows (`SettingsSection`, `SettingsToggleRow`, `SettingsValueRow`) | `ui/components/settings_rows.slint` | Stable |
| `Panel` enum (UI-side panel routing) | `ui/bridge.slint:70-91` | Add one variant (`media-backend`) |
| `tokio` + `serde_json` Cargo deps | `Cargo.toml:19,33` | Already present (tokio with `"full"`) |
| `tokio-tungstenite` + `futures-util` (WebSocket client) | — | **Add in STEP-9** |
| `src/migration/runtime.rs::start_graph_runtime / shutdown_graph_runtime` | `src/migration/runtime.rs:302,312` | Used in STEP-5 to drive the migration backend |

> **STEP-1** runs the grep ladder that turns every entry in this table
> into a concrete `file:line` anchor — re-run it before editing if any
> line numbers have drifted.

---

## 3. What this phase changes (the diff inventory)

| File | Change | Step | Lines (≈) |
|---|---|---|---|
| `ui/bridge.slint` | `MediaBackend` enum + 4 new properties + 3 new callbacks + 1 new `Panel` variant | STEP-2 / STEP-3 | +50 |
| `ui/pages/settings_page.slint` | Open-row for "Media backend" inside the existing `SettingsSection` ladder | STEP-3 | +12 |
| `ui/pages/media_backend_page.slint` *(new)* | Toggle + URL field + API-key field + connection-status banner + Probe / Apply buttons | STEP-4 | +220 |
| `ui/main.slint` | Route `Panel.media-backend` → `MediaBackendPage` | STEP-4 | +3 |
| `src/backend/mod.rs` *(new)* | `MediaBackend` trait + `BackendKind` enum + global selector | STEP-5 | +180 |
| `src/backend/migration_backend.rs` *(new)* | `MigrationBackend` struct implementing `MediaBackend` over `run_graph_command` | STEP-5 | +110 |
| `src/backend/gstpop/client.rs` *(new)* | `GstPopClient` — `tokio-tungstenite` + JSON-RPC 2.0 with `id` correlation | STEP-6 | +220 |
| `src/backend/gstpop/backend.rs` *(new)* | `GstPopBackend` implementing `MediaBackend` over `GstPopClient` | STEP-7 | +150 |
| `src/backend/lifecycle.rs` *(new)* | Connection lifecycle: probe on Apply, status writeback to Bridge, event-broadcast hook | STEP-8 | +130 |
| `Cargo.toml` | Add `tokio-tungstenite = "0.26"` + `futures-util` | STEP-9 | +2 |
| `src/lib.rs` | Wire the 3 new Bridge callbacks (`apply-media-backend`, `probe-media-backend`, `save-media-backend-settings`) into `register_bridge_callbacks` | STEP-8 | +60 |
| Tests | `src/backend/gstpop/protocol_tests.rs` + `src/backend/migration_backend_tests.rs` | STEP-9 | +180 |

**Total new lines: ~1 320** spread across 12 STEP-1 files and 1 new Slint page.

---

## 4. Out of scope (this phase)

- **Retargeting existing flows.** Quick actions (PHASE-9), the
  cast-history rerun callback (`Bridge.recast(string)` —
  `ui/bridge.slint:226`), and the PHASE-11 mixer screen all keep
  calling `run_graph_command` directly. A follow-on phase migrates
  call sites one-at-a-time once the trait has bedded in.
- **Per-pipeline UI.** gst-pop supports multiple pipelines per daemon
  (each addressed by string id `"0"`, `"1"`, …). For PHASE-12 the app
  binds to a **single pipeline id** (`pipeline_id == "0"` by default,
  configurable in the settings page). A multi-pipeline picker is a
  follow-on.
- **DBus interface.** gst-pop also exposes a DBus session-bus surface
  on Linux. Android has no DBus broker, so this phase only targets
  the WebSocket interface.
- **Pipeline-string composer.** Building gst-launch strings from
  high-level intent ("cast camera A at 1080p to RTMP X") is a UX
  problem of its own. PHASE-12 ships a **raw pipeline-string text
  field** in the new page so testers can paste a `gst-launch`
  expression and verify the round-trip; a composer lands later.
- **Reconnect-on-network-change.** The lifecycle in STEP-8 implements
  a single probe-on-Apply + a one-shot keepalive. Auto-reconnect on
  Wi-Fi handoff is a follow-on.

---

## 5. Why the snippets in this phase are illustrative

Every Slint snippet is **mentally validated** against the existing
tree and the Slint docs mirror under `draft/slint-ui/docs/`. Every
Rust snippet is **mentally validated** against the existing types in
`src/migration/` and the `gst-popctl` reference client at
[`gstpop/client/rust/src/main.rs`](https://github.com/dabrain34/gstpop/blob/main/client/rust/src/main.rs)
(notably lines 263–339, which show the canonical `connect_async` →
`split` → `send` → `read.next` → filter-by-id loop).

That said, the **pinned Slint fork** (`1.16.0` in `Cargo.toml`) and
the **pinned `tokio-tungstenite`** version may surface small API
differences (e.g. `Message::Text` taking a `Utf8Bytes` vs. `String`).
Each STEP file ends with a **verification block** (`cargo build` +
`ci/ui-validate.sh --no-build`) so the implementer catches drift the
moment it appears.

---

## 6. Exit criteria for the full phase

- [x] User can open **Settings → Media backend**.
- [x] The page renders a toggle (`Migration` | `gst-pop`), a URL
      input (default `ws://127.0.0.1:9000`), an optional API-key
      input (masked), a "Probe" button, and an "Apply" button.
- [x] Pressing **Probe** issues a `get_version` JSON-RPC call (or a
      legacy `getinfo` for the migration backend) and writes the
      result back to `Bridge.media-backend-status-text`.
- [x] Pressing **Apply** persists the selection (via existing
      settings persistence — STEP-8) and updates the global backend
      selector so the next `Backend::current().dispatch(...)` lands
      on the right impl.
- [x] A green/red status dot at the top of the page reflects
      `Bridge.media-backend-state` (`disconnected | probing | ready |
      error`).
- [x] `cargo build -p android-sender --target aarch64-linux-android`
      and `ci/ui-validate.sh --no-build` both pass.
- [ ] STEP-9's manual smoke run (against
      `ghcr.io/dabrain34/gstpop:latest` in Docker) creates a
      `videotestsrc ! autovideosink` pipeline and gets back a
      `pipeline_added` event in the app's debug log.

### Verification status

- Verified in this repo:
  `nix develop .#default -c cargo test backend::`
  `nix develop .#default -c bash ci/ui-validate.sh --no-build`
  `nix --offline develop .#android -c bash ci/build-gstreamer-android-glue.sh`
  `nix --offline develop .#android -c cargo build -p android-sender --target aarch64-linux-android`
- Still pending:
  Docker-backed smoke on a machine with `docker` installed.
- CI coverage added:
  `.github/workflows/gstpop-smoke.yml` now provisions Android + GStreamer,
  starts `ghcr.io/dabrain34/gstpop:latest`, runs the backend smoke tests,
  and builds the Android target so STEP-9 can run automatically in GitHub
  Actions even when local Docker is unavailable.

---

## 7. Step roadmap

| Step | What | Independently shippable? |
|---|---|---|
| **STEP-1** | Pre-flight inventory (greps, anchor checks) | ✔ Zero LOC delta |
| **STEP-2** | Bridge data model — `MediaBackend` enum + properties | ✔ Slint compiles even before STEP-5 lands the Rust trait |
| **STEP-3** | Bridge callbacks — `apply-media-backend`, `probe-media-backend`, `save-media-backend-settings` | ✔ Slint warns on first fire; STEP-8 wires Rust |
| **STEP-4** | Settings page section + `MediaBackendPage` + `Panel.media-backend` routing | ✔ UI navigable end-to-end with stubbed callbacks |
| **STEP-5** | Rust `MediaBackend` trait + `MigrationBackend` adapter | ✔ Adapter wraps existing `run_graph_command`; new behaviour gated by selector |
| **STEP-6** | `GstPopClient` WebSocket adapter (no app wiring yet) | ✔ Unit-testable in isolation against a `tokio` echo server |
| **STEP-7** | `GstPopBackend` impl on top of `GstPopClient` | ✔ Selector still defaults to migration, so no runtime change |
| **STEP-8** | Lifecycle — probe-on-Apply, status writeback to Bridge | ✔ The settings page becomes live |
| **STEP-9** | Cargo deps + tests + manual smoke against Docker daemon | ✔ Closes the phase |

Each step file links forward to the next and back to this parent.
End of parent doc.
