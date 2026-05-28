# 02 — Move `protocol` + `client`

The cheapest move: neither file references `crate::*`. Both depend
only on workspace crates (`serde`, `serde_json`, `tokio`,
`tokio-tungstenite`, `futures-util`, `tracing`, `anyhow`). Pure
relocation.

## 2.1 Files in scope

| Move from | Move to |
|---|---|
| `src/backend/gstpop/protocol.rs` | `crates/gstpop-runtime/src/protocol.rs` |
| `src/backend/gstpop/protocol_tests.rs` | `crates/gstpop-runtime/src/protocol_tests.rs` |
| `src/backend/gstpop/client.rs` | `crates/gstpop-runtime/src/client.rs` |

`protocol_tests.rs` opens with `use super::protocol::{classify,
ClassifiedFrame, Event};` — the `super::` reference is preserved by
the move (still resolves to the new crate's `protocol` module).

`client.rs:17` has `use super::protocol::{…}` — same story, still
resolves after the move.

## 2.2 New crate `lib.rs`

```rust
// crates/gstpop-runtime/src/lib.rs

pub mod client;
pub mod protocol;

#[cfg(test)]
mod protocol_tests;

pub use client::GstPopClient;
pub use protocol::{classify, ClassifiedFrame, Event, Request, Response};
```

(Re-export only the types the app actually uses. Today no caller
outside `gstpop/*` imports anything from `protocol.rs` directly, so
the re-export list mostly exists for clarity.)

## 2.3 Update the old module

```rust
// src/backend/gstpop/mod.rs — interim shape during M2.

pub mod backend;
pub mod embedded;
pub mod service;

pub use backend::GstPopBackend;

// Compatibility re-export: keep `crate::backend::gstpop::client::*`
// resolving until callers move in step 6.
pub use gstpop_runtime::{client, protocol, GstPopClient};
```

The two `#[cfg(test)] mod protocol_tests;` (current `mod.rs` line 6)
is deleted from the app — the tests live in the crate now.

## 2.4 Update internal `client.rs` callers

`backend.rs` line 7:

```diff
-use super::client::GstPopClient;
+use gstpop_runtime::GstPopClient;
```

`embedded.rs` does not import from `client` or `protocol`. No edit.

## 2.5 Build + test

```bash
cargo build --target aarch64-linux-android
cargo test  -p gstpop-runtime              # the relocated protocol_tests run here
cargo test  --lib                           # the app's existing tests still pass
```

Expected: same test count as before. The protocol tests have moved
binaries but not behaviour.

## 2.6 What we explicitly do NOT do

- Touch `embedded.rs` — it's the next step.
- Touch `service.rs` — it stays in the app until step 5.
- Re-name `client.rs` / `protocol.rs` — they keep their filenames so
  `git mv` produces a clean rename in the diff.
- Add new API surface — re-exports mirror what already exists.

## 2.7 Commit message

```
refactor(gstpop): move protocol + client into gstpop-runtime crate

Pure move. No call-site changes outside the gstpop module. The
in-app gstpop/mod.rs re-exports GstPopClient via the new crate so
backend.rs and any future callers keep compiling.

Next: move embedded.rs (step 3).
```

Next: [03-move-embedded.md](./03-move-embedded.md).
