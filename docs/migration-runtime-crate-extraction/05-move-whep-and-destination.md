# 05 — Move `whep_signaller_compat` + `destination.rs`

Goal: move the WHEP-signaller shim and the destination node into the
crate.

> **Correction (vs. earlier draft):** `whep_signaller_compat` is **not**
> host-only in this repo. The Android cast flow creates
> `DestinationFamily::Whep` (`src/lib.rs:1107`) and the existing
> `src/whep_signaller_compat.rs` is unconditional
> (`pub use mcore::whep_signaller::*;`). Both files move into the crate
> **without** any `cfg(not(target_os = "android"))` gating, and the crate
> picks up an unconditional `mcore` dependency.
>
> APK impact: zero — `mcore` is already an unconditional dep of the app
> (`src/lib.rs:16`).

## 5.1 What couples today

`src/migration/nodes/destination.rs` reaches into the app at 4 sites:

```
:855   crate::whep_signaller_compat::WhepServerSignaller::default()
:860   crate::whep_signaller_compat::ON_SERVER_STARTED_SIGNAL_NAME
:1489  Option<crate::whep_signaller_compat::WhepServerSignaller>
:1490  crate::whep_signaller_compat::ON_SERVER_STARTED_SIGNAL_NAME
```

All 4 references go to the same module: `src/whep_signaller_compat.rs`,
which is a one-line re-export of `mcore::whep_signaller`.

## 5.2 Add `mcore` to the crate

Edit `crates/migration-runtime/Cargo.toml`. Pin to the **same git rev**
the app uses (see root `Cargo.toml`):

```toml
[dependencies]
# … existing deps from step 01 …
mcore = { git = "https://github.com/kodyka/fcast", rev = "63980e6736e65adbd15588d21903d0c02223c15c" }
```

Optionally hoist to `[workspace.dependencies]` to keep the app and the
crate in lock-step:

```toml
# Cargo.toml (root)
[workspace.dependencies]
# …
mcore = { git = "https://github.com/kodyka/fcast", rev = "63980e6736e65adbd15588d21903d0c02223c15c" }
```

```toml
# Cargo.toml (root) and crates/migration-runtime/Cargo.toml
[dependencies]
mcore.workspace = true
```

## 5.3 Move the shim

```
mv src/whep_signaller_compat.rs \
   crates/migration-runtime/src/whep_signaller_compat.rs
```

Contents unchanged — it's still:

```rust
//! crates/migration-runtime/src/whep_signaller_compat.rs
pub use mcore::whep_signaller::*;
```

Update `crates/migration-runtime/src/lib.rs`:

```rust
pub mod whep_signaller_compat;
```

Drop the matching `mod whep_signaller_compat;` line from `src/lib.rs`.

## 5.4 Move `destination.rs`

```
mv src/migration/nodes/destination.rs \
   crates/migration-runtime/src/nodes/destination.rs
```

Rewrite the imports inside the moved file. Every
`crate::migration::…` and `crate::whep_signaller_compat::…` becomes
`crate::…` because we're now **inside** the crate:

```diff
-use crate::migration::protocol::{DestinationFamily, DestinationInfo, NodeInfo, State};
+use crate::protocol::{DestinationFamily, DestinationInfo, NodeInfo, State};
```

And the 4 WHEP sites — keep them unconditional, just shorten the path:

```diff
-let signaller = crate::whep_signaller_compat::WhepServerSignaller::default();
+let signaller = crate::whep_signaller_compat::WhepServerSignaller::default();
```

(The string `crate::whep_signaller_compat::…` is correct in both cases —
the *meaning* of `crate::` changes from "the app" to "the crate", but
the path text is identical because the module name was preserved.)

The test reference at line ~1489 stays as-is:

```rust
let _signaller: Option<crate::whep_signaller_compat::WhepServerSignaller> = None;
assert!(!crate::whep_signaller_compat::ON_SERVER_STARTED_SIGNAL_NAME.is_empty());
```

Update the crate's `nodes/mod.rs`:

```rust
pub mod control;
pub mod destination;     // ← added
pub mod mixer;
pub mod screen_capture;
pub mod source;
pub mod video_generator;
```

Drop the matching `pub mod destination;` from the app-side
`src/migration/nodes/mod.rs` shim.

## 5.5 Verify

```bash
# Host build of the crate
cargo build -p migration-runtime
cargo test  -p migration-runtime

# App host build
cargo build

# Android — the failure mode of step 5 lives here
nix develop .#android -c bash scripts/build-deploy.sh
```

**On-device smoke (Android — WHEP path):**

1. Start service → status "running".
2. Connect to a receiver.
3. Start a screen-capture cast (uses `DestinationFamily::Whep`).
4. Verify the receiver renders the stream for 30+ seconds.
5. Stop cast → notification stays, runtime still running.
6. Stop service → notification clears.

Step 3 is the canary for this milestone. If the WHEP destination fails
to create, look for:

* **Unresolved `crate::whep_signaller_compat::*`** inside the moved
  `destination.rs` — likely a leftover `crate::migration::` prefix
  before "whep" that confused the rewrite.
* **`mcore` version mismatch** between the app and the crate — `cargo
  tree -i mcore` must show one version. Pin via
  `[workspace.dependencies]` if it doesn't.

## 5.6 Files changed

| File | Change |
|---|---|
| `Cargo.toml` (root) | + `mcore` in `[workspace.dependencies]` (optional but recommended) |
| `crates/migration-runtime/Cargo.toml` | + `mcore` dep |
| `src/whep_signaller_compat.rs` | **deleted** |
| `crates/migration-runtime/src/whep_signaller_compat.rs` | **new** (moved verbatim — 1 line) |
| `src/migration/nodes/destination.rs` | **deleted** |
| `crates/migration-runtime/src/nodes/destination.rs` | **new** (moved + protocol imports rewritten) |
| `crates/migration-runtime/src/lib.rs` | + `pub mod whep_signaller_compat;` |
| `crates/migration-runtime/src/nodes/mod.rs` | + `pub mod destination;` |
| `src/lib.rs` | drop `mod whep_signaller_compat;` |
| `src/migration/nodes/mod.rs` | drop `pub mod destination;` |

After this step the app's `src/migration/nodes/mod.rs` shim is empty
(everything re-exported from the crate). Step 06 deletes the entire
shim layer.
