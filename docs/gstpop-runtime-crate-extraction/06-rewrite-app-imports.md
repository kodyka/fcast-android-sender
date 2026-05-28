# 06 — Rewrite remaining app imports + final cleanup

After step 5, the `src/backend/gstpop/` directory is gone. The only
thing keeping the build green is the chain of re-exports that's been
in flight since step 2. This step removes the re-exports and points
each callsite at its final home.

## 6.1 Inventory of remaining `crate::backend::gstpop::*` paths

Grep before:

```bash
$ rg -n 'crate::backend::gstpop|super::gstpop|backend::gstpop' src/
```

After steps 2–5 the only residual references should be in
`src/lib.rs` (JNI exports). Specifically:

| File | Line | Path used |
|---|---|---|
| `src/lib.rs` | 3012 | `crate::backend::gstpop::embedded::start_embedded(port)` |
| `src/lib.rs` | 3028 | `crate::backend::gstpop::embedded::stop_embedded()` |
| `src/lib.rs` | 3042 | `crate::backend::gstpop::embedded::embedded_status()` |
| `src/lib.rs` | 3051 | `crate::backend::gstpop::embedded::url_port(url)` |

## 6.2 Rewrite

```diff
 // src/lib.rs

 pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost<'local>(
     mut env: jni::JNIEnv<'local>,
     _class: jni::objects::JClass<'local>,
     config_json: jni::objects::JString<'local>,
 ) -> jni::sys::jstring {
     let config = jstring_to_string(&mut env, &config_json).unwrap_or_default();
     let port = parse_gstpop_config_port(&config).unwrap_or(9000);
     let status = HOST_RUNTIME.block_on(async {
-        crate::backend::gstpop::embedded::start_embedded(port).await
+        gstpop_runtime::start_embedded(port).await
     });
     …
 }

 pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStopGstPopServiceHost<'local>(…) {
     let status = HOST_RUNTIME
-        .block_on(async { crate::backend::gstpop::embedded::stop_embedded().await });
+        .block_on(async { gstpop_runtime::stop_embedded().await });
     …
 }

 pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeGetGstPopServiceStatus<'local>(…) {
-    let status = crate::backend::gstpop::embedded::embedded_status();
+    let status = gstpop_runtime::embedded_status();
     …
 }

 fn parse_gstpop_config_port(json: &str) -> Option<u16> {
     let v: serde_json::Value = serde_json::from_str(json).ok()?;
     let url = v.get("gstpop_url")?.as_str()?;
-    Some(crate::backend::gstpop::embedded::url_port(url))
+    Some(gstpop_runtime::url_port(url))
 }
```

## 6.3 `src/backend/lifecycle.rs` final cleanup

After step 5 the `use` blocks at the top of `apply`/`autostart`
already point at `gstpop_runtime` and `crate::gstpop_service`. The
remaining references to `super::gstpop::embedded::EmbeddedState` in
the 1Hz poller (`lifecycle.rs:138-141`) need updating:

```diff
                 let state_str: &'static str = match status.state {
-                    super::gstpop::embedded::EmbeddedState::Stopped => "stopped",
-                    super::gstpop::embedded::EmbeddedState::Starting => "starting",
-                    super::gstpop::embedded::EmbeddedState::Running => "running",
-                    super::gstpop::embedded::EmbeddedState::Error => "error",
+                    gstpop_runtime::EmbeddedState::Stopped  => "stopped",
+                    gstpop_runtime::EmbeddedState::Starting => "starting",
+                    gstpop_runtime::EmbeddedState::Running  => "running",
+                    gstpop_runtime::EmbeddedState::Error    => "error",
                 };
```

Same for the `status` query just above:

```diff
-                let status = super::gstpop::embedded::embedded_status();
+                let status = gstpop_runtime::embedded_status();
```

And the `is_localhost` check inside `apply`/`autostart`:

```diff
-            BackendKind::GstPop if embedded::is_localhost(&config.gstpop_url) => {
+            BackendKind::GstPop if gstpop_runtime::is_localhost(&config.gstpop_url) => {
```

If `apply` had `use gstpop_runtime as embedded;` from step 5, you can
drop the alias and write the full path inline — pick one style and
be consistent.

## 6.4 Verify nothing still imports the old path

```bash
$ rg -n 'crate::backend::gstpop|super::gstpop|backend::gstpop' src/
# Expect: no matches.

$ rg -n 'use gstpop_runtime' src/
# Expect: 3-5 hits (lifecycle.rs, gstpop_service.rs, gstpop_backend.rs, lib.rs).
```

## 6.5 Drop the app's direct dep on `gstpop`

The app's `Cargo.toml` still has:

```toml
gstpop = { path = "vendor/gstpop" }
```

After step 3 nothing in the app crate uses `gstpop::*` directly —
only the new `gstpop-runtime` crate does. Drop the app dep:

```diff
-gstpop = { path = "vendor/gstpop" }
 migration-runtime = { path = "crates/migration-runtime" }
 gstpop-runtime    = { path = "crates/gstpop-runtime" }
```

Verify:

```bash
$ rg -n '^use gstpop::|^use gstpop\b' src/
# Expect: no matches.
```

If a stray import shows up, it's almost certainly something
embedded.rs used (e.g. `gstpop::server::ServerConfig`) — that import
already lives in the crate now, so the grep should come up empty.

## 6.6 Optional: tighten visibility

After the move, `EmbeddedStatus::serialize` is the only thing the JNI
layer actually consumes (via `serde_json::to_string`). The
`StoredBackendConfig`-shaped JSON parsing in `parse_gstpop_config_port`
could move into the crate as a helper:

```rust
// crates/gstpop-runtime/src/lib.rs
pub fn url_port_from_config_json(json: &str) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let url = v.get("gstpop_url")?.as_str()?;
    Some(url_port(url))
}
```

…and `src/lib.rs:3048` becomes:

```rust
fn parse_gstpop_config_port(json: &str) -> Option<u16> {
    gstpop_runtime::url_port_from_config_json(json)
}
```

Tiny win; skip if you don't care.

## 6.7 Final build matrix

```bash
cargo build --target aarch64-linux-android       # device target
cargo build                                      # host target
cargo test  --lib                                # app tests
cargo test  -p gstpop-runtime                    # crate tests
cargo test  -p gstpop-runtime -- --ignored --test-threads=1
cargo test  -p migration-runtime                 # neighbour crate (sanity)
```

All green expected.

## 6.8 Commit message

```
refactor(gstpop): rewrite remaining imports to gstpop_runtime

Last step of the extraction. JNI exports in lib.rs and the 1Hz
status poller in lifecycle.rs now import from gstpop_runtime::*
directly. The app crate no longer depends on `gstpop` (vendor) —
only gstpop-runtime does.

The `src/backend/gstpop/` directory and all `crate::backend::gstpop::*`
paths are now gone.

Next: verification + rollback (step 7).
```

Next: [07-verification.md](./07-verification.md).
