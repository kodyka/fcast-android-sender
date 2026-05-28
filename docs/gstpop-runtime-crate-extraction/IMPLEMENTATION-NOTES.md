# Implementation notes — what actually shipped

The extraction landed on branch `gstpop-runtime-extraction`. This file
captures the delta between the plan (steps 01–07) and the as-shipped
diff, so future readers don't have to grep around.

## How it shipped

- **One branch, not six PRs.** The plan proposed six per-milestone
  PRs for easy rollback (`07-verification.md §7.5`). Implementation
  collapsed them into a single branch. The rollback table no longer
  applies as written — `git revert` the branch as a whole, or
  cherry-pick the file moves out individually.
- **Git tracked the moves as renames** (`git status -R` shows
  `client.rs`, `embedded.rs`, `protocol.rs`, `protocol_tests.rs`,
  `backend.rs`, `service.rs` all as renames). The plan's "pure
  relocation" framing is borne out in `git log --follow`.

## Verification (run by implementer)

```bash
nix develop . -c cargo build
nix develop .#android -c cargo build --target aarch64-linux-android
nix develop . -c cargo test --lib
nix develop . -c cargo test -p gstpop-runtime
nix develop . -c cargo test -p gstpop-runtime -- --ignored --test-threads=1
nix develop . -c cargo test -p migration-runtime
```

All green.

**Not yet run:** the on-device smoke flow in
[`07-verification.md §7.3`](./07-verification.md#73-on-device-smoke-after-step-6).
This is the only gate that catches behavioural regressions; run it
before merge.

## Deviations from the plan

### 1. `log` was promoted to workspace deps

Plan only promoted `async-trait`, `futures-util`, `once_cell`,
`tokio`, `tokio-tungstenite`. The shipped diff additionally promoted
`log = "0.4"` because `gstpop-runtime` calls `log::info!` from
`client.rs:48`. Harmless extra cleanup; doesn't change the app's
dep set.

### 2. `gstpop-runtime` depends on `uuid`

Not listed in `01-workspace-skeleton.md` step 1.3. Required by
`client.rs` (request-ID generation, `uuid::Uuid::new_v4()`). The
workspace dep entry already existed for the app; the crate just
references it via `uuid.workspace = true`.

Final `crates/gstpop-runtime/Cargo.toml` deps list:

```toml
anyhow.workspace = true
async-trait.workspace = true
futures-util.workspace = true
gstpop = { path = "../../vendor/gstpop" }
log.workspace = true
once_cell.workspace = true
parking_lot.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tokio-tungstenite.workspace = true
tracing.workspace = true
uuid.workspace = true
```

### 3. Re-exports collapsed straight to final form

The plan's M2–M5 kept compatibility re-exports in
`src/backend/gstpop/mod.rs` so each step left the build green in
isolation. Since the work shipped as one branch, the intermediate
re-export layer was skipped — `src/backend/gstpop/` was deleted in
the same change that rewrote callers. Same end state.

### 4. `lifecycle.rs` uses module aliases

The plan in `06-rewrite-app-imports.md §6.3` proposed inlining the
full path everywhere. Implementation chose the alias form mentioned
in `05-relocate-service.md §5.3`:

```rust
use crate::gstpop_service as service;
use gstpop_runtime as embedded;
```

Used in both `apply` and `autostart`. The 1Hz poller (lifecycle.rs
136-141) uses the fully-qualified `gstpop_runtime::EmbeddedState::*`
form — both styles coexist. Pick one if it ever bothers anyone;
not worth a separate PR.

### 5. `parse_gstpop_config_port` helper stayed in `lib.rs`

Step 6.6 floated moving it into the crate as
`url_port_from_config_json`. Not done — the helper is a 4-liner
that depends on `StoredBackendConfig`'s `gstpop_url` field name, so
keeping it in the app keeps the crate ignorant of the app's Serde
schema. Defensible call.

## What didn't change

- JNI symbol set is identical. `nm | grep GstPopServiceBridge`
  still returns three lines (Start, Stop, GetStatus).
- Runtime behaviour is identical — the extraction is a pure
  refactor.
- Manifest, Java side, Slint UI: completely untouched.
- `vendor/gstpop` workspace member: completely untouched.

## Files renamed (for `git log --follow`)

| Old path | New path |
|---|---|
| `src/backend/gstpop/client.rs` | `crates/gstpop-runtime/src/client.rs` |
| `src/backend/gstpop/embedded.rs` | `crates/gstpop-runtime/src/embedded.rs` |
| `src/backend/gstpop/protocol.rs` | `crates/gstpop-runtime/src/protocol.rs` |
| `src/backend/gstpop/protocol_tests.rs` | `crates/gstpop-runtime/src/protocol_tests.rs` |
| `src/backend/gstpop/backend.rs` | `src/backend/gstpop_backend.rs` |
| `src/backend/gstpop/service.rs` | `src/gstpop_service.rs` |

Files newly created (not renames):

- `crates/gstpop-runtime/Cargo.toml`
- `crates/gstpop-runtime/src/lib.rs`

Files deleted:

- `src/backend/gstpop/mod.rs` (and the now-empty directory)

## Pre-existing warnings

Builds still surface pre-existing warnings in `android-sender` and
`migration-runtime`. The plan explicitly forbade adjacent cleanup
(`00-overview.md` "Out of scope"); leave them for a dedicated
warning-sweep PR.

## Follow-ups still applicable

The list in [`07-verification.md §7.6`](./07-verification.md#76-post-merge-follow-ups)
is unchanged:

- Port-switch fix in `embedded.rs` (architecture doc §10 invariant).
- JNI symbol grep in CI.
- Robolectric tests for `GstPopServiceBridge`.
- Possible `crates/jni-glue` consolidation.

None blocking; tackle separately.
