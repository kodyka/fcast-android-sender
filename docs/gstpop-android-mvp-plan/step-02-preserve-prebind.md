# Step 2 — Preserve vendored `TcpListener` pre-bind behavior

**Phase:** 1 — Android MVP
**Priority:** highest (cheap, prevents regression)
**Depends on:** nothing
**Unblocks:** future upstream-sync work

## Goal

The vendored daemon (`vendor/gstpop/src/server.rs`,
`vendor/gstpop/src/websocket/server.rs`) already diverges from upstream by
**binding the `TcpListener` before spawning the WebSocket task**. This is
critical for Android: bind failures surface synchronously and propagate
through `EmbeddedStatus.last_error`. Without this, a port conflict only shows
up later as a silent task crash.

This step does **not change code**; it marks the divergence so a future
upstream sync does not silently undo it.

## Files touched

- `vendor/gstpop/src/server.rs` (comment only)
- `vendor/gstpop/src/websocket/server.rs` (comment only)

## Verified current divergence

```diff
# vendor/gstpop/src/server.rs (Android version)
+ // Pre-bind the listener before spawning so bind errors surface immediately.
+ let listener = match tokio::net::TcpListener::bind(addr).await {
+     Ok(l) => l,
+     Err(e) => {
+         error!("Failed to bind WebSocket server on {}: {}", addr, e);
+         return Err(());
+     }
+ };
  ...
- if let Err(e) = ws_server.run(ws_event_rx).await {
+ if let Err(e) = ws_server.run(listener, ws_event_rx).await {
```

```diff
# vendor/gstpop/src/websocket/server.rs (Android version)
- pub async fn run(self, mut event_rx: EventReceiver) -> Result<()> {
-     let listener = TcpListener::bind(&self.addr).await?;
+ pub async fn run(self, listener: TcpListener, mut event_rx: EventReceiver) -> Result<()> {
```

## Implementation

Add a load-bearing comment block above each modified region. Example for
`vendor/gstpop/src/server.rs`, immediately before the pre-bind:

```rust
// === ANDROID DIVERGENCE FROM UPSTREAM ===
// Do NOT remove during upstream sync without updating
// crates/gstpop-runtime/src/embedded.rs and docs/gstpop-android-mvp-plan/.
//
// Upstream `daemon/src/server.rs` lets `WebSocketServer::run` bind internally,
// which means a port-in-use error only manifests after the server task is
// already spawned and detached. On Android we need bind errors to bubble up
// synchronously to `EmbeddedStatus.last_error`, so we bind here and pass the
// listener into `run()`. See docs/deep-research-gstpop-demon.md §"What is
// already good in the target tree" for rationale.
let listener = match tokio::net::TcpListener::bind(addr).await {
    Ok(l) => l,
    Err(e) => {
        error!("Failed to bind WebSocket server on {}: {}", addr, e);
        return Err(());
    }
};
```

Mirror in `vendor/gstpop/src/websocket/server.rs` above the modified `run`
signature:

```rust
// === ANDROID DIVERGENCE FROM UPSTREAM ===
// Listener is bound by the caller (server.rs) so bind failures surface
// synchronously. Upstream signature is `run(self, event_rx)`; do not revert.
pub async fn run(self, listener: TcpListener, mut event_rx: EventReceiver) -> Result<()> {
```

## Optional: divergence ledger

Create `vendor/gstpop/DIVERGENCE.md` to make the next upstream sync trivial:

```markdown
# vendor/gstpop divergences from upstream `dabrain34/gstpop/daemon`

Tracked so an upstream sync does not silently revert Android-relevant changes.

| File | Change | Reason |
|---|---|---|
| `src/server.rs` | Pre-binds `TcpListener` before spawn | Surface bind errors synchronously for Android `EmbeddedStatus.last_error` |
| `src/websocket/server.rs` | `run(self, listener, event_rx)` takes pre-bound listener | Pair of the above |
| `Cargo.toml` | `gstreamer`/`gstreamer-pbutils` bumped to `0.25` | Workspace dependency alignment |
| `src/lib.rs` | License header trimmed, `signal` module removed | Library-only, no CLI |
| (removed) `src/main.rs`, `src/cmd/`, `src/signal.rs`, `src/cli_tests.rs` | CLI surface intentionally absent | Library-only build for embedded use |
```

## Verification

```bash
cargo build -p gstpop
cargo build -p gstpop-runtime
```

Then grep for the marker so future contributors find it:

```bash
grep -rn "ANDROID DIVERGENCE FROM UPSTREAM" vendor/gstpop/src
```

## Done when

- Both divergence comments are present.
- `vendor/gstpop/DIVERGENCE.md` exists (optional but recommended).
- `cargo build` succeeds for both `gstpop` and `gstpop-runtime`.
