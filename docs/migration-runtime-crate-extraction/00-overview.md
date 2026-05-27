# 00 — Overview

Per-step recipe for extracting `src/migration/` into a workspace-local crate
`crates/migration-runtime`.

> **Status:** plan only. No source changes are made by these documents — they
> are the spec for the implementation PR(s). All code blocks are illustrative;
> copy them into the listed files when implementing.

## How to read

Read 00 → 07 once for context, then implement in the listed order. Each step
keeps the tree green (`cargo build --target aarch64-linux-android` passes).

| # | File | What it covers |
|---|------|---|
| 0 | [00-overview.md](./00-overview.md) | This file. |
| 1 | [01-workspace-skeleton.md](./01-workspace-skeleton.md) | Convert root `Cargo.toml` to a workspace; create empty `migration-runtime` crate. |
| 2 | [02-move-pure-modules.md](./02-move-pure-modules.md) | Move `protocol.rs`, `messages.rs`, `media_bridge.rs`, and the un-coupled `nodes/*`. |
| 3 | [03-move-node-manager-and-runtime.md](./03-move-node-manager-and-runtime.md) | Move `node_manager.rs` and `runtime.rs` into the crate. Implement after steps 4 and 5. |
| 4 | [04-decouple-framepair.md](./04-decouple-framepair.md) | Replace `crate::FRAME_PAIR` static with constructor injection; move `screen_capture.rs`. |
| 5 | [05-move-whep-and-destination.md](./05-move-whep-and-destination.md) | Move `whep_signaller_compat` into the crate; move `destination.rs`. |
| 6 | [06-rewrite-app-imports.md](./06-rewrite-app-imports.md) | Replace `crate::migration::*` with `migration_runtime::*` across the app; rename `migration/service.rs` to `migration_service.rs`. |
| 7 | [07-verification.md](./07-verification.md) | Build matrix, smoke checklist, rollback plan. |

## Milestone mapping

```
M1 — Workspace skeleton        → step 1
M2 — Move pure modules         → step 2
M3 — Decouple                  → steps 4, 5
M4 — Move node/runtime         → step 3
M5 — App import rewrite        → step 6
M? — Verification + rollback   → step 7
```

Ship each milestone as its own PR (≤400 LOC each). They are individually
mergeable, reviewable, and revertible.

## Out of scope

* Moving the JNI exports themselves (`Java_org_fcast_..._nativeStart*`) — the
  symbol name encodes the Java package, so they stay in the app `cdylib`. A
  follow-up could extract them into `crates/jni-glue`.
* Splitting `src/backend/` or `src/lib.rs` further.
* Replacing existing `vendor/gstpop` — it is already a path dep and just
  becomes a workspace member.
