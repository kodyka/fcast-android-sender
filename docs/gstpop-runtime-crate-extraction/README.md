# gstpop-runtime crate extraction plan

Plan for hoisting `src/backend/gstpop/*` into a workspace-local crate
`crates/gstpop-runtime`. Mirrors the migration-runtime extraction
recipe in [`../migration-runtime-crate-extraction/`](../migration-runtime-crate-extraction/).

> **Status:** **implemented** on branch `gstpop-runtime-extraction`.
> See [`IMPLEMENTATION-NOTES.md`](./IMPLEMENTATION-NOTES.md) for the
> as-shipped diff against this plan and deviations worth knowing.
> The per-step files below remain as the recipe-of-record.

Pair with [`../gstpop-service-architecture.md`](../gstpop-service-architecture.md)
(runtime architecture — updated to reflect the post-extraction
layout).

## Read order

| # | File | What it covers |
|---|---|---|
| 0 | [00-overview.md](./00-overview.md) | Why, target shape, risks, naming choice. |
| 1 | [01-workspace-skeleton.md](./01-workspace-skeleton.md) | Add `crates/gstpop-runtime` to the workspace; promote shared deps. |
| 2 | [02-move-protocol-and-client.md](./02-move-protocol-and-client.md) | Move `protocol.rs`, `protocol_tests.rs`, `client.rs`. |
| 3 | [03-move-embedded.md](./03-move-embedded.md) | Move `embedded.rs` (daemon lifecycle + statics). |
| 4 | [04-relocate-backend-impl.md](./04-relocate-backend-impl.md) | Promote `gstpop/backend.rs` → `src/backend/gstpop_backend.rs`. |
| 5 | [05-relocate-service.md](./05-relocate-service.md) | Promote `gstpop/service.rs` → `src/gstpop_service.rs`; delete `src/backend/gstpop/`. |
| 6 | [06-rewrite-app-imports.md](./06-rewrite-app-imports.md) | Rewrite remaining `crate::backend::gstpop::*` paths; drop the app's direct `gstpop` dep. |
| 7 | [07-verification.md](./07-verification.md) | Build matrix, on-device smoke, rollback plan. |

## Target layout

```
crates/gstpop-runtime/
├── Cargo.toml
└── src/
    ├── lib.rs              # re-exports
    ├── embedded.rs         # daemon lifecycle (was backend/gstpop/embedded.rs)
    ├── client.rs           # WS JSON-RPC client (was backend/gstpop/client.rs)
    ├── protocol.rs         # classifier + types (was backend/gstpop/protocol.rs)
    └── protocol_tests.rs   # (was backend/gstpop/protocol_tests.rs)

src/
├── backend/
│   ├── gstpop_backend.rs   # NEW — MediaBackend impl only (was backend/gstpop/backend.rs)
│   ├── lifecycle.rs        # imports gstpop_runtime::* directly
│   └── mod.rs              # `pub mod gstpop_backend;` (no more `pub mod gstpop;`)
├── gstpop_service.rs       # NEW — Rust → Java bridge (was backend/gstpop/service.rs)
└── lib.rs                  # JNI exports call gstpop_runtime::*
```

## Milestone breakdown

```
M1 — Workspace skeleton             → step 1   (~80 LOC)
M2 — Move protocol + client         → step 2   (~250 LOC moved)
M3 — Move embedded daemon control   → step 3   (~285 LOC moved)
M4 — Relocate backend trait impl    → step 4   (~250 LOC moved)
M5 — Relocate service dispatch      → step 5   (~100 LOC moved + dir delete)
M6 — Rewrite app imports + cleanup  → step 6   (~10 LOC edited, app deps tightened)
```

Six PRs, each independently mergeable, each leaves the tree green.
