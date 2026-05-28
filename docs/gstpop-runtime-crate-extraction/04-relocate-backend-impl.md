# 04 — Relocate `gstpop/backend.rs` → `src/backend/gstpop_backend.rs`

`GstPopBackend` implements the **app's** `MediaBackend` trait
(`src/backend/mod.rs:31-37`). The trait is part of the app's API
surface — it can't move into a leaf crate without dragging the
`BackendStatus`/`BackendKind` types along.

The migration-runtime PR resolved the same issue by leaving
`MigrationBackend` in the app at `src/backend/migration_backend.rs`.
Apply the same shape here.

## 4.1 Move

| Move from | Move to |
|---|---|
| `src/backend/gstpop/backend.rs` | `src/backend/gstpop_backend.rs` |

Only one file moves. No code logic changes — just import paths.

## 4.2 Import rewrites inside the moved file

```diff
 // src/backend/gstpop_backend.rs (was gstpop/backend.rs)

-use super::client::GstPopClient;
-use crate::backend::{BackendKind, BackendStatus, MediaBackend};
+use gstpop_runtime::GstPopClient;
+use crate::backend::{BackendKind, BackendStatus, MediaBackend};
```

(After step 2 the `GstPopClient` re-export came from
`gstpop_runtime`. We tighten the path now that `backend.rs` is no
longer a sibling of `client.rs`.)

The tests in this file already use `GstPopBackend::new(…)` directly;
no further change needed.

## 4.3 `src/backend/mod.rs` — register the new module

```diff
 // src/backend/mod.rs

-pub mod gstpop;
 mod kind;
 pub mod lifecycle;
+pub mod gstpop_backend;
 mod migration_backend;
 pub mod persistence;

 pub use kind::BackendKind;
 pub use migration_backend::MigrationBackend;
+pub use gstpop_backend::GstPopBackend;
```

Note: `pub mod gstpop;` is **not yet deleted** — `service.rs` still
lives under that directory until step 5. Keep it `pub mod gstpop;`
for now, and remove just the inner `pub mod backend;` line from
`gstpop/mod.rs`:

```diff
 // src/backend/gstpop/mod.rs — interim shape during M4

-pub mod backend;
 pub mod service;

-pub use backend::GstPopBackend;
-
 // Compatibility re-exports during the transition.
 pub use gstpop_runtime::{
     client, embedded, protocol,
     embedded_status, is_localhost, start_embedded, stop_embedded,
     url_port, EmbeddedState, EmbeddedStatus, GstPopClient,
 };
```

## 4.4 Caller updates

Exactly one external caller: `src/backend/lifecycle.rs:12`.

```diff
-use crate::backend::gstpop::GstPopBackend;
+use crate::backend::GstPopBackend;
```

(`mod.rs` now re-exports it at the top level — symmetric with
`MigrationBackend`.)

## 4.5 Tests

`backend.rs` ships four `#[tokio::test]` cases:

- `probe_against_docker` (ignored)
- `translate_passes_through_native_verbs`
- `translate_maps_start_to_play`
- `round_trip_against_echo_server`
- `probe_returns_version_and_pipeline_count_from_mock_server`
- `probe_fails_cleanly_when_nothing_is_listening`

All move with the file. They run under `cargo test --lib` against the
app crate (where `GstPopBackend` now lives).

## 4.6 Build verification

```bash
cargo build --target aarch64-linux-android
cargo test  --lib                           # all backend tests still pass
cargo test  -p gstpop-runtime               # crate still green
```

## 4.7 Why not move `GstPopBackend` into the crate too?

Tempting, but:

1. The crate would need to depend on the app's `MediaBackend` trait
   → circular dependency, or the trait moves down (large blast
   radius).
2. `backend.rs` is only ~250 LOC and mostly verb-translation logic
   that is specific to **this app's** backend-switching contract. It
   isn't reusable enough to justify the trait split.
3. Migration-runtime made the same call (`MigrationBackend` stays in
   the app). Be consistent.

If the trait ever moves into a separate `crates/backend-api` crate
(out of scope), `GstPopBackend` can move with it.

## 4.8 Commit message

```
refactor(gstpop): promote backend trait impl to src/backend/gstpop_backend.rs

Renames src/backend/gstpop/backend.rs → src/backend/gstpop_backend.rs
to mirror migration_backend.rs. Imports GstPopClient from
gstpop_runtime directly. lifecycle.rs uses the re-exported path
crate::backend::GstPopBackend.

Next: relocate service dispatch (step 5).
```

Next: [05-relocate-service.md](./05-relocate-service.md).
