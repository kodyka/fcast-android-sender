# Step 7 — Optional JNI bridge in `gstpop-runtime`

**Phase:** 2 — Android polish
**Priority:** medium (skip if app crate already owns JNI cleanly)
**Depends on:** Steps 1, 3, 6
**Unblocks:** nothing critical

## Decision gate — read first

If the existing `android-sender` crate already exposes the calls Kotlin needs
(start/stop/status) via its own JNI module, **skip this step**. Two JNI
boundaries for the same runtime is worse than one.

Pursue this step only if at least one is true:

- The app crate's JNI surface is tangled with UI logic and should not own
  runtime lifecycle.
- You want `gstpop-runtime` itself to be reusable from another Android app
  without re-implementing JNI.

## Goal

Expose **three** JNI methods, returning JSON strings so Kotlin can evolve
without recompiling the cdylib:

- `nativeStartEmbedded(port: jint) -> jstring`
- `nativeStopEmbedded() -> jstring`
- `nativeStatus() -> jstring`

Keep per-RPC calls out of JNI. Kotlin should connect to the WebSocket
directly (via a Kotlin JSON-RPC client) for `play`/`pause`/etc.

## Files touched

- `crates/gstpop-runtime/Cargo.toml` (add `android-jni` feature + deps)
- `crates/gstpop-runtime/src/android_jni.rs` (new)
- `crates/gstpop-runtime/src/lib.rs` (cfg-gated module)

## Implementation

### 1. Manifest

```toml
# crates/gstpop-runtime/Cargo.toml
[features]
default = []
typed-client = []
media-tools = []
android-jni = ["dep:jni", "dep:ndk-context"]

[target.'cfg(target_os = "android")'.dependencies]
jni = { version = "0.21", optional = true }
ndk-context = { version = "0.1.1", optional = true }
```

### 2. The bridge

Create `crates/gstpop-runtime/src/android_jni.rs`:

```rust
//! Minimal JNI bridge for `gstpop-runtime` embedded lifecycle.
//!
//! Returns JSON strings so the Kotlin side can be schema-flexible. Per-RPC
//! calls (`play`, `pause`, ...) are intentionally NOT exposed here — Kotlin
//! should talk to the embedded WebSocket on 127.0.0.1 directly.

#![cfg(all(target_os = "android", feature = "android-jni"))]

use jni::objects::JClass;
use jni::sys::{jint, jstring};
use jni::JNIEnv;
use once_cell::sync::Lazy;
use std::sync::Mutex;
use tokio::runtime::{Builder, Runtime};

use crate::embedded::{embedded_status, start_embedded, stop_embedded, EmbeddedStatus};

/// Single shared tokio runtime for the JNI bridge. Built once on first call.
/// We use a `Mutex<Runtime>` rather than a `OnceCell<Runtime>` so the
/// runtime can be torn down on `nativeStopEmbedded` if you ever decide to.
static RUNTIME: Lazy<Mutex<Runtime>> = Lazy::new(|| {
    Mutex::new(
        Builder::new_multi_thread()
            .enable_all()
            .thread_name("gstpop-jni")
            .build()
            .expect("build tokio runtime"),
    )
});

fn json_status(status: &EmbeddedStatus) -> String {
    serde_json::to_string(status)
        .unwrap_or_else(|e| format!(r#"{{"state":"error","last_error":"{e}"}}"#))
}

fn make_jstring(env: &mut JNIEnv, s: &str) -> jstring {
    env.new_string(s)
        .expect("JNI new_string failed")
        .into_raw()
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_fcast_sender_GstPopBridge_nativeStartEmbedded(
    mut env: JNIEnv,
    _class: JClass,
    port: jint,
) -> jstring {
    let rt = RUNTIME.lock().expect("runtime mutex poisoned");
    let status = rt.block_on(start_embedded(port as u16));
    drop(rt);
    make_jstring(&mut env, &json_status(&status))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_fcast_sender_GstPopBridge_nativeStopEmbedded(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let rt = RUNTIME.lock().expect("runtime mutex poisoned");
    let status = rt.block_on(stop_embedded());
    drop(rt);
    make_jstring(&mut env, &json_status(&status))
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_org_fcast_sender_GstPopBridge_nativeStatus(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let status = embedded_status();
    make_jstring(&mut env, &json_status(&status))
}
```

### 3. Wire into `lib.rs`

```rust
#[cfg(all(target_os = "android", feature = "android-jni"))]
pub mod android_jni;
```

## Kotlin side

```kotlin
// app/src/main/java/org/fcast/sender/GstPopBridge.kt
package org.fcast.sender

import org.json.JSONObject

object GstPopBridge {
    init { System.loadLibrary("android_sender") } // or whatever cdylib name

    external fun nativeStartEmbedded(port: Int): String
    external fun nativeStopEmbedded(): String
    external fun nativeStatus(): String

    fun start(port: Int = 9000): Status = Status.parse(nativeStartEmbedded(port))
    fun stop(): Status = Status.parse(nativeStopEmbedded())
    fun status(): Status = Status.parse(nativeStatus())

    data class Status(
        val state: String,
        val bind: String,
        val port: Int,
        val externallyOwned: Boolean,
        val lastError: String?,
        val startedAtUnixMs: Long?,
    ) {
        companion object {
            fun parse(json: String): Status {
                val j = JSONObject(json)
                return Status(
                    state = j.optString("state", "error"),
                    bind = j.optString("bind", ""),
                    port = j.optInt("port", 0),
                    externallyOwned = j.optBoolean("externally_owned", false),
                    lastError = if (j.isNull("last_error")) null else j.optString("last_error"),
                    startedAtUnixMs = if (j.isNull("started_at_unix_ms")) null
                                      else j.optLong("started_at_unix_ms"),
                )
            }
        }
    }
}
```

## Gotchas

- **Method symbol naming** is package-sensitive: `Java_org_fcast_sender_GstPopBridge_nativeStartEmbedded`
  must match the Kotlin class's fully qualified name. Rename if your package
  is different.
- **Runtime once per process.** Do not create a new tokio runtime per JNI
  call; this would leak threads and slow start_embedded.
- **`block_on` inside JNI is acceptable here** because the called futures are
  short and idempotent. Do not call this for per-RPC paths — they should run
  through Kotlin's own WebSocket client.
- **Process death and re-launch.** `RUNTIME` is a `Lazy` static; Android
  reuses the process across activity restarts, so the runtime persists. After
  full process death it is rebuilt fresh.
- **`#[unsafe(no_mangle)]`** is the 2024 edition spelling; if your edition is
  2021, write `#[no_mangle]`.

## Verification

```bash
cargo ndk -t arm64-v8a build \
    -p gstpop-runtime \
    --features "typed-client media-tools android-jni" \
    --release

# Confirm symbols are exported:
nm -D --defined-only target/aarch64-linux-android/release/libgstpop_runtime.so \
    | grep nativeStartEmbedded
```

Then rebuild the cdylib that includes `gstpop-runtime` and re-run the
[Step 6 device smoke test](./step-06-android-arm64-build.md).

## Done when

- arm64 build succeeds with `android-jni` feature.
- `nm` shows all three `Java_org_fcast_sender_GstPopBridge_*` symbols.
- Kotlin `GstPopBridge.start(...)` returns a parsed `Status(state="running")`.
- App can stop and restart embedded server without leaking ports.
