# 01 — Rust JNI bridge (`src/lib.rs`)

Three new `#[cfg(target_os = "android")]` JNI exports that map 1:1 to
the three `private static native` methods declared in
`MigrationRuntimeServiceBridge.java`
(see [02-java-bridge.md](./02-java-bridge.md)).

These functions are added immediately **after** the existing gst-pop
JNI block in `src/lib.rs:2988-3044`, before the `#[cfg(test)]` mod at
line 3046.

## 1.1 Three new exports

```rust
// ── migration runtime service host JNI bridge ────────────────────────────────
// Symbols match MigrationRuntimeServiceBridge in the
// `org.fcast.android.sender` package.

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    _config_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    // Migration runtime currently has no start-time config; the JString is
    // accepted for API symmetry with GstPopServiceBridge and ignored.
    let json = match crate::migration::runtime::start_graph_runtime() {
        Ok(()) => migration_runtime_status_json("running", None),
        Err(err) => migration_runtime_status_json("error", Some(&err.to_string())),
    };
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStopMigrationRuntimeHost<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    let json = match crate::migration::runtime::shutdown_graph_runtime() {
        Ok(()) => migration_runtime_status_json("stopped", None),
        Err(err) => migration_runtime_status_json("error", Some(&err.to_string())),
    };
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
#[allow(non_snake_case)]
#[unsafe(no_mangle)]
pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeGetMigrationRuntimeStatus<
    'local,
>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    // try_handle_command_json never panics and always returns a JSON string,
    // either {"id":null,"result":…} on success or {"id":null,"result":{"error":…}}.
    // We use it as a liveness probe.
    let probe = crate::migration::runtime::try_handle_command_json(r#"{"getinfo":{}}"#);
    let state = if probe.contains("\"result\"") && !probe.contains("\"error\"") {
        "running"
    } else {
        "stopped"
    };
    let json = migration_runtime_status_json(state, None);
    env.new_string(json).expect("new_string").into_raw()
}
```

## 1.2 Status JSON shape

The three exports above call one shared helper that emits the JSON
envelope the Java side expects. Insert it immediately below the three
exports, still gated on `target_os = "android"`:

```rust
#[cfg(target_os = "android")]
fn migration_runtime_status_json(state: &str, last_error: Option<&str>) -> String {
    let mut value = serde_json::json!({ "state": state });
    if let Some(err) = last_error {
        value["last_error"] = serde_json::Value::String(err.to_string());
    }
    serde_json::to_string(&value).unwrap_or_else(|_| {
        format!("{{\"state\":\"{}\"}}", state.replace('"', "'"))
    })
}
```

**Shape contract** — the Java side relies on:

| Field | Type | Always present | Notes |
|-------|------|----------------|-------|
| `state` | string | yes | One of `"running"`, `"stopped"`, `"starting"`, `"error"`. |
| `last_error` | string | only when `state == "error"` | Human-readable; surfaced in the notification. |

The shape is intentionally a **strict subset** of `EmbeddedStatus`, so
the same `describe(...)` style helper on the Java side can be reused
(without the `bind`/`port` fields).

## 1.3 Why no `HOST_RUNTIME.block_on(...)`?

The gst-pop variant uses it because `start_embedded` / `stop_embedded`
are `async fn`. The migration runtime equivalents are synchronous (they
internally spawn `std::thread::Builder` threads — see `runtime.rs:51-72`),
so dispatching them onto a separate async runtime adds no value.
Calling on the JNI binder thread is safe: the calls return in
milliseconds.

If a future change makes any of these async, switch the three exports
to use `crate::HOST_RUNTIME.block_on(async { … })`, matching the
gst-pop pattern in `src/lib.rs:3003-3005` and `3019-3020`.

## 1.4 Imports

No new `use` statements at the top of `src/lib.rs` are required — every
type referenced (`jni::JNIEnv`, `jni::objects::JClass`, `jni::objects::JString`,
`jni::sys::jstring`, `serde_json::json`) is already imported or named
fully-qualified in the gst-pop block. Keep that convention.

## 1.5 Where to insert (locator diff)

```diff
 #[cfg(target_os = "android")]
 fn parse_gstpop_config_port(json: &str) -> Option<u16> {
     let v: serde_json::Value = serde_json::from_str(json).ok()?;
     let url = v.get("gstpop_url")?.as_str()?;
     Some(crate::backend::gstpop::embedded::url_port(url))
 }

+// ── migration runtime service host JNI bridge ────────────────────────────────
+// Symbols match MigrationRuntimeServiceBridge in the
+// `org.fcast.android.sender` package.
+
+#[cfg(target_os = "android")]
+#[allow(non_snake_case)]
+#[unsafe(no_mangle)]
+pub extern "C" fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost<
+    'local,
+>(
+    mut env: jni::JNIEnv<'local>,
+    _class: jni::objects::JClass<'local>,
+    _config_json: jni::objects::JString<'local>,
+) -> jni::sys::jstring {
+    // … (see §1.1)
+}
+
+// … other two exports (see §1.1) …
+
+#[cfg(target_os = "android")]
+fn migration_runtime_status_json(state: &str, last_error: Option<&str>) -> String {
+    // … (see §1.2)
+}
+
 #[cfg(test)]
 mod phase9_dispatch_tests {
```
