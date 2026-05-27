# 03 — Move `node_manager.rs` and `runtime.rs`

Goal: move the two largest runtime modules into the crate. Both have
**zero** `crate::` couplings to app code — they only reach into
`crate::migration::*` siblings, which now live in `migration-runtime`.

## 3.1 What moves

| From | To | LOC |
|---|---|---|
| `src/migration/node_manager.rs` | `crates/migration-runtime/src/node_manager.rs` | ~1100 |
| `src/migration/runtime.rs` | `crates/migration-runtime/src/runtime.rs` | ~650 |

Combined: ~1750 LOC moved. Pure code motion + import rewrites.

## 3.2 Import rewrites inside the crate

`node_manager.rs` opens with (today):

```rust
use crate::migration::{
    media_bridge::*,
    messages::*,
    nodes::{
        control::*, destination::*, mixer::*, screen_capture::*, source::*,
        video_generator::*,
    },
    protocol::*,
};
```

Becomes:

```rust
use crate::{
    media_bridge::*,
    messages::*,
    nodes::{
        control::*, destination::*, mixer::*, screen_capture::*, source::*,
        video_generator::*,
    },
    protocol::*,
};
```

But note: `destination::*` and `screen_capture::*` are **not yet in the
crate** (they move in steps 04/05). To keep this step buildable, add
*temporary* `pub use` shims **inside the crate** that re-export from the
app:

```rust
// crates/migration-runtime/src/nodes/mod.rs — during step 03 only
pub mod control;
pub mod mixer;
pub mod source;
pub mod video_generator;

// Temporary back-references to the app side while extraction is in flight.
// Removed in step 04 (screen_capture) and step 05 (destination).
//
// NOTE: this only works because the app `cdylib` and the crate are linked
// into the same final binary. Cyclic crate deps are not introduced; the
// shims live in the app's `mod.rs` (step 02) and re-export *into* the
// crate via the trait-object boundary in node_manager.rs.
```

Reality check: **circular crate deps are forbidden**. The crate cannot
literally re-export app types. So step 03's actual move strategy is:

**Option A (recommended) — temporarily inline placeholders in the crate.**
Add stub `pub struct DestinationNode;` and `pub struct ScreenCaptureNode;`
in the crate that satisfy the `node_manager` type signatures but `panic!()`
when used. Steps 04/05 replace the stubs with real implementations.

**Option B — defer step 03 until after 04 and 05.** Move
`screen_capture.rs` (step 04) and `destination.rs` (step 05) first while
they still live alongside `node_manager.rs` in the app, then move
`node_manager.rs` + `runtime.rs` together. This avoids stubs entirely but
makes the intermediate state larger.

**Recommendation:** use **Option B**. Reorder so the sequence is:

```
01  workspace skeleton
02  pure modules                          (protocol, messages, media_bridge, simple nodes)
04  decouple framepair → move screen_capture
05  move whep + move destination
03  move node_manager + runtime          ← reordered to last move
06  app import rewrite
07  verification
```

Update [00-overview.md](./00-overview.md) accordingly when implementing.

## 3.3 Final layout after step 03 (under Option B ordering)

`crates/migration-runtime/src/`:

```
lib.rs
protocol.rs
messages.rs
media_bridge.rs
node_manager.rs          ← moved in this step
runtime.rs               ← moved in this step
whep_signaller_compat.rs (moved in step 05)
nodes/
  mod.rs
  control.rs
  source.rs
  mixer.rs
  video_generator.rs
  screen_capture.rs      (moved in step 04)
  destination.rs         (moved in step 05)
```

`src/migration/` is now empty except for `service.rs` (renamed/moved in
step 06).

## 3.4 Update the crate's `lib.rs`

```rust
//! migration-runtime — extracted from `android-sender`.

pub mod media_bridge;
pub mod messages;
pub mod node_manager;
pub mod nodes;
pub mod protocol;
pub mod runtime;

// `whep_signaller_compat` is added unconditionally in step 05 — Android
// uses WHEP too (see DestinationFamily::Whep in the app's cast flow).
pub mod whep_signaller_compat;

// Existing re-exports preserved …
pub use protocol::{
    Command, CommandResult, ControlMode, ControlPoint, DestinationFamily,
    DestinationInfo, MixerInfo, MixerSlotInfo, NodeInfo, ServerMessage,
    SourceInfo, State,
};
```

## 3.5 Tests come with `runtime.rs`

`runtime.rs` carries its own `#[cfg(test)] mod tests` block. After the
move:

```bash
cargo test -p migration-runtime runtime::tests
```

…runs the existing roundtrip / HTTP / shutdown-idempotency tests on the
host (no emulator needed). This is the **first** observable win of the
refactor.

## 3.6 Verify

```bash
cargo build -p migration-runtime
cargo test  -p migration-runtime
nix develop .#android -c bash scripts/build-deploy.sh
```

On-device smoke: tap Start service → status flips to "running" → notification
appears → Stop service → notification clears. Behaviour identical to before
the extraction.

## 3.7 Files changed

| File | Change |
|---|---|
| `src/migration/node_manager.rs` | **deleted** |
| `src/migration/runtime.rs` | **deleted** |
| `crates/migration-runtime/src/node_manager.rs` | **new** (moved + imports rewritten) |
| `crates/migration-runtime/src/runtime.rs` | **new** (moved + imports rewritten) |
| `crates/migration-runtime/src/lib.rs` | + `pub mod node_manager; pub mod runtime;` |
| `src/migration/mod.rs` | update shims: drop `pub mod node_manager; pub mod runtime;`, add `pub use migration_runtime::{node_manager, runtime};` |
