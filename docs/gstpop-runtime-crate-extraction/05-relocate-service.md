# 05 — Relocate `gstpop/service.rs` → `src/gstpop_service.rs`

`service.rs` is the Rust → Java dispatch layer. It cannot move into
the runtime crate because it depends on:

- `crate::android_context()` (`src/lib.rs:103-113`) — needs the
  Android platform handle stashed by `main_inner`.
- `crate::backend::persistence::StoredBackendConfig` — the app's
  Serde config struct.

Both are app-scoped, so we promote `service.rs` to a sibling of
`src/migration_service.rs` instead.

## 5.1 Move

| Move from | Move to |
|---|---|
| `src/backend/gstpop/service.rs` | `src/gstpop_service.rs` |

No logic changes; the file moves verbatim.

## 5.2 `src/lib.rs` — declare the module

Near the top, alongside the existing `pub mod migration_service;` (if
present) or the other top-level modules:

```diff
 #[cfg(target_os = "android")]
 pub mod migration_service;
+#[cfg(target_os = "android")]
+pub mod gstpop_service;
```

(The migration extraction PR introduced `migration_service` at the
crate root for the same reason — keep gstpop adjacent to it.)

## 5.3 Caller updates

Two call sites in `src/backend/lifecycle.rs`:

```diff
         bridge.on_start_gstpop_service(move || {
             let config = read_config_from_bridge(&start_weak);
             let weak = start_weak.clone();
             tokio::spawn(async move {
                 push_state(&weak, crate::MediaBackendState::Starting);
-                if let Err(err) = super::gstpop::service::request_service_start(&config) {
+                if let Err(err) = crate::gstpop_service::request_service_start(&config) {
                     push_error(&weak, &format!("service start failed: {err}"));
                 }
             });
         });
```

```diff
         bridge.on_stop_gstpop_service(move || {
-            super::gstpop::service::request_service_stop();
+            crate::gstpop_service::request_service_stop();
             …
         });
```

And in `apply` (`lifecycle.rs:233-272`):

```diff
     async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
-        use super::gstpop::{embedded, service};
+        use gstpop_runtime as embedded;       // is_localhost still here for now
+        use crate::gstpop_service as service;

         let previous = current();
         config.save(&self.files_dir)?;

         match config.kind {
             BackendKind::GstPop if embedded::is_localhost(&config.gstpop_url) => {
                 push_state(&weak, crate::MediaBackendState::Starting);
                 if let Err(err) = service::request_service_start(&config) {
                     …
                 }
             }
             BackendKind::GstPop => {
                 service::request_service_stop();
             }
             BackendKind::Migration => {
                 service::request_service_stop();
             }
         }
         …
     }
```

The `autostart` function has the same `use super::gstpop::{embedded,
service};` block — apply the same rewrite there.

## 5.4 Interim module shape

```rust
// src/backend/gstpop/mod.rs — after M5, the directory holds nothing.

// (Entire file deleted; mod.rs disappears with the directory.)
```

Wait — `backend.rs` already moved out in step 4. `service.rs` is
moving out now. The compatibility re-exports for `embedded`,
`client`, `protocol` were placeholders until step 6, but step 6 is
the rewrite. So at the end of M5 the directory is empty except for
the `mod.rs` re-exports.

To avoid an awkward transient state, do these together at the end of
M5:

1. Move `service.rs` out (as above).
2. Update all `super::gstpop::*` callers in `lifecycle.rs` to point
   at the new locations (`crate::gstpop_service::*`,
   `gstpop_runtime::*`).
3. Delete the entire `src/backend/gstpop/` directory in one go.
4. Remove `pub mod gstpop;` from `src/backend/mod.rs`.

This collapses what would be M5 + M6 into a single PR, but it's the
honest unit of change: the in-app `gstpop` module disappears in this
step. Re-label step 6 to "verification + cleanup" if you prefer
that framing.

## 5.5 Optional: simplify `request_service_*` signatures

While we're touching `service.rs`, consider a small ergonomic cleanup
that the migration-runtime PR didn't apply:

- Today: `request_service_start(config: &StoredBackendConfig)` serialises
  the whole config to JSON and passes it via the Intent extra.
- The service-side only needs the port (`parse_gstpop_config_port`
  pulls it out again).

**Don't change this in step 5.** Two reasons:

1. The full JSON survives a future where the daemon needs more
   start-time params (API key passthrough was already discussed in
   the guide §12.3).
2. Changing the signature here couples to step 4's `backend.rs` move
   — keep the PR scope tight.

Document it as a follow-up if you care.

## 5.6 Build verification

```bash
cargo build --target aarch64-linux-android
cargo test  --lib
cargo test  -p gstpop-runtime
```

Manual smoke (per the architecture doc §12):

```bash
adb logcat -s GstPopService:D GstPopServiceBridge:D | \
  grep -E "onStartCommand|nativeStart|nativeStop"
```

Then Apply gst-pop in the UI and confirm the state transitions still
fire exactly once each.

## 5.7 JNI Java-side: nothing changes

`GstPopServiceBridge.java` calls
`Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost`
— that symbol still lives in `src/lib.rs:3002`. The Java side is
oblivious to the Rust-side reorg.

## 5.8 Commit message

```
refactor(gstpop): promote service dispatch to src/gstpop_service.rs

Moves the Rust → Java bridge out of src/backend/gstpop/. Pairs with
src/migration_service.rs. Removes the now-empty
src/backend/gstpop/ directory; src/backend/mod.rs no longer declares
`pub mod gstpop;`. All callers in lifecycle.rs migrated to the new
paths (gstpop_runtime::* for the runtime, crate::gstpop_service::*
for the Java bridge).

Next: rewrite remaining app imports + cleanup (step 6).
```

Next: [06-rewrite-app-imports.md](./06-rewrite-app-imports.md).
