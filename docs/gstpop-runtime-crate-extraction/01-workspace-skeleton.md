# 01 — Workspace skeleton

Add an empty `gstpop-runtime` crate to the workspace. No code moves
yet — this step just makes the dependency graph editable.

## 1.1 `Cargo.toml` (root) — add the member

Edit the workspace members list:

```diff
 [workspace]
-members = [".", "crates/migration-runtime", "vendor/gstpop"]
+members = [".", "crates/migration-runtime", "crates/gstpop-runtime", "vendor/gstpop"]
 resolver = "2"
```

And add the new crate to the root `[dependencies]` block, next to
`migration-runtime`:

```diff
 migration-runtime = { path = "crates/migration-runtime" }
+gstpop-runtime    = { path = "crates/gstpop-runtime" }
```

Leave `gstpop = { path = "vendor/gstpop" }` exactly where it is — the
new crate uses it as a workspace dep, the app keeps it for now (we'll
drop the app's direct use of `gstpop::*` in step 6 once `embedded.rs`
has moved).

## 1.2 Add shared deps to `[workspace.dependencies]`

The new crate needs a couple of deps not yet promoted to the workspace
table. Add them so both the app and the crate can use a single
version:

```diff
 [workspace.dependencies]
 anyhow = "1"
+async-trait = "0.1"
 chrono = { version = "0.4", features = ["serde"] }
+futures-util = { version = "0.3", default-features = false, features = ["sink", "std"] }
 gst = { package = "gstreamer", version = "0.25" }
 …
+once_cell = "1"
 parking_lot = "0.12"
 serde = { version = "1.0", features = ["derive"] }
 serde_json = "1.0"
+tokio = { version = "1.51", features = ["full"] }
+tokio-tungstenite = { version = "0.26", default-features = false, features = ["connect", "handshake"] }
 tracing = { version = "0.1", features = ["log", "log-always"] }
 …
```

Then collapse the app's `[dependencies]` block to reference them as
`workspace`:

```diff
-async-trait = "0.1"
-futures-util = { version = "0.3", default-features = false, features = ["sink", "std"] }
-once_cell = "1"
-tokio = { version = "1.51", features = ["full"] }
-tokio-tungstenite = { version = "0.26", default-features = false, features = ["connect", "handshake"] }
+async-trait.workspace = true
+futures-util.workspace = true
+once_cell.workspace = true
+tokio.workspace = true
+tokio-tungstenite.workspace = true
```

This is the same housekeeping the migration-runtime PR did in step 1.

## 1.3 Create the crate skeleton

```
crates/gstpop-runtime/
├── Cargo.toml
└── src/
    └── lib.rs
```

`crates/gstpop-runtime/Cargo.toml`:

```toml
[package]
name = "gstpop-runtime"
version = "0.1.0"
edition = "2021"
publish = false
description = "In-process gst-pop daemon host + JSON-RPC client (extracted from android-sender)."

[dependencies]
anyhow.workspace = true
async-trait.workspace = true
futures-util.workspace = true
gstpop = { path = "../../vendor/gstpop" }
once_cell = { workspace = true }
parking_lot.workspace = true
serde.workspace = true
serde_json.workspace = true
tokio.workspace = true
tokio-tungstenite.workspace = true
tracing.workspace = true

[dev-dependencies]
tokio = { workspace = true, features = ["full", "test-util", "macros"] }
```

`crates/gstpop-runtime/src/lib.rs`:

```rust
//! In-process gst-pop daemon host + JSON-RPC client.
//!
//! Extracted from `android-sender`. See
//! `docs/gstpop-runtime-crate-extraction/` for the extraction plan and
//! `docs/gstpop-service-architecture.md` for the runtime architecture.

// Modules added by later steps.
```

## 1.4 Verify the workspace still builds

```bash
cargo metadata --no-deps --format-version=1 \
  | jq -r '.packages[].name' \
  | sort
# Expect: android-sender, gstpop, gstpop-runtime, migration-runtime
```

```bash
cargo build --target aarch64-linux-android       # full build still works
cargo build -p gstpop-runtime                    # empty crate builds
cargo test  -p gstpop-runtime                    # zero tests, exits 0
```

No app code moves in this step — the crate exposes nothing yet, so
nothing imports from it.

## 1.5 Commit message

```
build(gstpop-runtime): scaffold workspace crate

Empty crate. No symbols, no callers. Promotes async-trait /
futures-util / once_cell / tokio / tokio-tungstenite to
workspace deps so the app and the new crate share versions.

Next: move protocol + client (step 2).
```

Next: [02-move-protocol-and-client.md](./02-move-protocol-and-client.md).
