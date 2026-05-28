# 03 — Move `embedded.rs` (daemon lifecycle)

This is the largest move (≈285 LOC) but still a pure relocation —
`embedded.rs` only depends on `gstpop::*` (the vendored upstream
daemon) and standard workspace crates. No `crate::*` references.

## 3.1 What moves

| Move from | Move to |
|---|---|
| `src/backend/gstpop/embedded.rs` | `crates/gstpop-runtime/src/embedded.rs` |

Symbols that become public (the app + the JNI layer call them):

- `pub enum EmbeddedState { Stopped, Starting, Running, Error }`
- `pub struct EmbeddedStatus { state, externally_owned, bind, port, last_error, started_at_unix_ms }`
- `pub async fn start_embedded(port: u16) -> EmbeddedStatus`
- `pub async fn stop_embedded() -> EmbeddedStatus`
- `pub fn embedded_status() -> EmbeddedStatus`
- `pub fn is_localhost(url: &str) -> bool`
- `pub fn url_port(url: &str) -> u16`

Internal helpers stay private (`start_server`, `probe_port_open`,
`wait_for_port`, `now_unix_ms`, `snapshot`).

## 3.2 Lib re-exports

```rust
// crates/gstpop-runtime/src/lib.rs (after step 3)

pub mod client;
pub mod embedded;
pub mod protocol;

#[cfg(test)]
mod protocol_tests;

pub use client::GstPopClient;
pub use embedded::{
    embedded_status, is_localhost, start_embedded, stop_embedded,
    url_port, EmbeddedState, EmbeddedStatus,
};
pub use protocol::{classify, ClassifiedFrame, Event, Request, Response};
```

## 3.3 Statics ownership: what changes

The process-global statics (`CLAIMED`, `READY`, `STATE`, `HANDLE`)
move along with `embedded.rs`. After the move they live **once**, in
the crate, and are reached from the app via the public functions.

Invariant: a single Linux process loads `libfcastsender.so` exactly
once → the crate's statics initialise exactly once. The move does
not split or duplicate them.

Verify post-move with a logcat grep (per
[`gstpop-service-architecture.md §12`](../gstpop-service-architecture.md#12-debugging-cheatsheet)):

```bash
adb logcat -s GstPopService:D | grep -E "Embedded gst-pop running on|adopting"
# Expect: exactly one line per process lifetime.
```

## 3.4 Interim module shape

```rust
// src/backend/gstpop/mod.rs — interim shape during M3.

pub mod backend;
pub mod service;

pub use backend::GstPopBackend;

// Compatibility re-exports during the transition. Step 6 removes them
// and rewrites callers to import from gstpop_runtime directly.
pub use gstpop_runtime::{
    client, embedded, protocol,
    embedded_status, is_localhost, start_embedded, stop_embedded,
    url_port, EmbeddedState, EmbeddedStatus, GstPopClient,
};
```

This keeps the existing `crate::backend::gstpop::embedded::*` paths
resolving even though the code lives elsewhere. Step 6 rewrites them.

## 3.5 No callsite changes in this step

The JNI exports in `src/lib.rs` still write
`crate::backend::gstpop::embedded::start_embedded(port)` — that path
now resolves via the re-export above. The same goes for
`lifecycle.rs`. We change paths in step 6, not here, to keep the
diff per-commit small.

## 3.6 Tests

`embedded.rs` has two ignored integration tests
(`start_then_stop_is_idempotent`, `external_listener_is_adopted_…`)
gated by `#[ignore = "uses process-global state; run with
--test-threads=1 --ignored"]`. They move with the file and run via:

```bash
cargo test -p gstpop-runtime -- --ignored --test-threads=1
```

The app's `cargo test --lib` no longer sees these tests — that's
fine, they need process isolation anyway.

## 3.7 Build verification

```bash
cargo build --target aarch64-linux-android
cargo build -p gstpop-runtime
cargo test  -p gstpop-runtime                              # protocol tests
cargo test  -p gstpop-runtime -- --ignored --test-threads=1 # embedded tests
```

## 3.8 Commit message

```
refactor(gstpop): move embedded daemon lifecycle into gstpop-runtime

Moves embedded.rs verbatim. Process-global statics (CLAIMED, READY,
STATE, HANDLE) now live in the crate. App-side imports continue to
work via re-exports from src/backend/gstpop/mod.rs — step 6 will
rewrite them.

Next: relocate the MediaBackend trait impl (step 4).
```

Next: [04-relocate-backend-impl.md](./04-relocate-backend-impl.md).
