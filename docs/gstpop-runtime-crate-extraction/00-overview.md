# 00 — Overview: extract `src/backend/gstpop` → `crates/gstpop-runtime`

Per-step recipe for hoisting the gst-pop daemon-host runtime out of the
app `cdylib` and into a workspace-local crate `crates/gstpop-runtime`.

> **Status:** **implemented** on branch `gstpop-runtime-extraction`.
> The plan landed as a single branch (not six per-milestone PRs). See
> [`IMPLEMENTATION-NOTES.md`](./IMPLEMENTATION-NOTES.md) for the
> as-shipped delta. Per-step files below remain as the recipe-of-record.

## Why

- The gst-pop module today is `src/backend/gstpop/{embedded, client,
  protocol, backend, service, mod}.rs` (≈900 LOC) plus the JNI exports
  in `src/lib.rs:3000-3052`. It is the second-largest self-contained
  subsystem in the app after migration-runtime.
- Migration-runtime extraction (`crates/migration-runtime/`, recent
  commits `a5ed87b → 32b5f3a`) proves the pattern works without
  destabilising the build. Apply the same template here.
- Benefits: faster host-target rebuilds when iterating on the daemon
  control plane; lets the protocol + client be unit-tested without the
  whole app build; clearer ownership boundary for a future
  `cargo test --package gstpop-runtime` job in CI.

## Reference: how it's wired today

Cross-checked against [`gstpop-service-architecture.md`](../gstpop-service-architecture.md):

- `embedded.rs`: process-global statics (`CLAIMED`, `READY`, `STATE`,
  `HANDLE`) + `start_embedded` / `stop_embedded` / `embedded_status` +
  helpers (`is_localhost`, `url_port`).
- `client.rs`: WebSocket JSON-RPC client (`GstPopClient`).
- `protocol.rs` (+ `protocol_tests.rs`): frame classifier + Request /
  Response / Event types.
- `backend.rs`: `GstPopBackend` (implements `MediaBackend`). Trait
  comes from `crate::backend::{BackendKind, BackendStatus,
  MediaBackend}`.
- `service.rs`: `request_service_start` / `request_service_stop` —
  calls into `crate::android_context()` + `GstPopServiceBridge` Java
  class. Serialises `crate::backend::persistence::StoredBackendConfig`.

External callers of this module:

| File | Symbol used |
|---|---|
| `src/lib.rs:3012,3028,3042,3051` | `start_embedded` / `stop_embedded` / `embedded_status` / `url_port` |
| `src/backend/lifecycle.rs:12,84,92,136-141,205` | `GstPopBackend`, `service::request_service_*`, `embedded::{is_localhost, embedded_status, EmbeddedState}` |
| `src/backend/lifecycle.rs:243` | `embedded::is_localhost` |

## Target shape

```
crates/gstpop-runtime/
├── Cargo.toml
└── src/
    ├── lib.rs              # pub use {embedded::*, client::*, protocol::*}
    ├── embedded.rs         # daemon lifecycle (moved verbatim)
    ├── client.rs           # WS JSON-RPC client (moved verbatim)
    ├── protocol.rs         # classifier + types (moved verbatim)
    └── protocol_tests.rs   # moved verbatim

src/
├── backend/
│   ├── gstpop_backend.rs   # NEW — only the MediaBackend impl
│   ├── lifecycle.rs        # uses gstpop_runtime::embedded::* directly
│   └── mod.rs              # `pub mod gstpop_backend;` (deletes `pub mod gstpop;`)
├── gstpop_service.rs       # NEW — Rust → Java bridge, mirrors migration_service.rs
└── lib.rs                  # JNI exports call gstpop_runtime::embedded::*
```

The pattern mirrors migration-runtime:
- **Pure runtime moves to the crate** (no `crate::*` references).
- **Trait impl stays in app** (binds the crate's API to the app's
  `MediaBackend` trait).
- **JNI exports stay in app** (symbol names encode the Java package).
- **Android-only Java dispatch stays in app** (depends on
  `crate::android_context()`).

## How to read

Read 00 → 06 once for context, then implement in the listed order.
Each step keeps the tree green (`cargo build --target
aarch64-linux-android` passes after every step).

| # | File | What it covers |
|---|---|---|
| 0 | [00-overview.md](./00-overview.md) | This file. |
| 1 | [01-workspace-skeleton.md](./01-workspace-skeleton.md) | Add `crates/gstpop-runtime` to the workspace; empty `lib.rs`. |
| 2 | [02-move-protocol-and-client.md](./02-move-protocol-and-client.md) | Move `protocol.rs`, `protocol_tests.rs`, `client.rs` — no app couplings. |
| 3 | [03-move-embedded.md](./03-move-embedded.md) | Move `embedded.rs` (daemon lifecycle + statics). |
| 4 | [04-relocate-backend-impl.md](./04-relocate-backend-impl.md) | Promote `gstpop/backend.rs` to `src/backend/gstpop_backend.rs`. |
| 5 | [05-relocate-service.md](./05-relocate-service.md) | Promote `gstpop/service.rs` to `src/gstpop_service.rs`. |
| 6 | [06-rewrite-app-imports.md](./06-rewrite-app-imports.md) | Replace `crate::backend::gstpop::*` with `gstpop_runtime::*`; delete the old module. |
| 7 | [07-verification.md](./07-verification.md) | Build matrix, smoke checklist, rollback plan. |

## Milestone mapping

```
M1 — Workspace skeleton             → step 1
M2 — Move protocol + client         → step 2
M3 — Move embedded daemon control   → step 3
M4 — Relocate backend trait impl    → step 4
M5 — Relocate service dispatch      → step 5
M6 — Rewrite app imports + cleanup  → step 6
M? — Verification + rollback        → step 7
```

Ship each milestone as its own PR (≤ ~250 LOC each). They are
individually mergeable, reviewable, and revertible.

## Naming choice: `gstpop-runtime`

The upstream daemon crate (vendored at `vendor/gstpop`) is already
called `gstpop`. To avoid a clash:

- Crate name: `gstpop-runtime` (in `Cargo.toml`).
- Rust module path: `gstpop_runtime` (cargo's default mapping).
- Crate description: *"In-process gst-pop daemon host + JSON-RPC
  client (extracted from android-sender)."*

This parallels `migration-runtime` exactly.

## Out of scope

- Moving the JNI exports themselves (`Java_org_fcast_..._native*`) —
  the symbol name encodes the Java package, so they stay in the app
  `cdylib`. A follow-up could extract them into `crates/jni-glue`.
- Touching `vendor/gstpop` — it stays a separate workspace member.
- Splitting `src/backend/` or `src/lib.rs` further.
- Adding port-switch / multi-daemon support (called out in
  `gstpop-service-architecture.md §10`). The crate boundary makes
  that easier later but doesn't deliver it.
- Robolectric tests for `GstPopServiceBridge` (still missing per the
  guide §10.3); orthogonal to the crate extraction.

## Risks & mitigations

| Risk | Mitigation |
|---|---|
| `embedded.rs` uses `parking_lot::RwLock` / `Lazy` statics — moving them changes which compilation unit owns the singletons. | Statics still live in *one* place after the move (the crate). No double-initialisation risk. Verify with a logcat-grep for `Embedded gst-pop running on …` (must appear once per process). |
| Test gating: `protocol_tests.rs` is `#[cfg(test)] mod` under the old layout. | Move it as `mod protocol_tests;` inside the crate, keep the `#[cfg(test)]` gate. |
| `service.rs` depends on `crate::android_context()` + `StoredBackendConfig`. Moving it into the crate would drag those along. | Don't move it into the crate — promote it to `src/gstpop_service.rs` (step 5). Same pattern as `src/migration_service.rs`. |
| `tokio-tungstenite` is only used by `client.rs`. | Add it as a dep of the new crate; remove from app deps once the move is complete (step 6 cleanup). |
| The crate ends up unused on non-Android targets if the JNI side is the only caller. | It isn't — `lifecycle.rs` is host-buildable and uses it for autostart/apply. The `cargo test` host target keeps the crate covered. |
