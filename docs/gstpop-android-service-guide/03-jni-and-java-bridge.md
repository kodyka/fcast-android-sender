# 3 · JNI entrypoints + Java bridge

This step adds three JNI symbols and a small Java class to call them.
The Java class is the **only** entrypoint that the rest of the app
(and the service in step 4) is allowed to use.

## 3.1 JNI exports in `src/lib.rs`

Add at the end of the file, alongside the existing
`Java_org_fcast_android_sender_MainActivity_nativeGraphCommand`
block (current pattern: `src/lib.rs:2453-2485`).

```rust
// src/lib.rs

// ──────────────────────────────────────────────────────────────────
// gst-pop service host JNI bridge
// Symbols are named to match GstPopServiceBridge in the
// `org.fcast.android.sender` package — do not rename either side.
// ──────────────────────────────────────────────────────────────────

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    config_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    let config = jstring_to_string(&mut env, &config_json).unwrap_or_default();
    let port = parse_config_port(&config).unwrap_or(9000);

    let status = HOST_RUNTIME.block_on(async {
        crate::backend::gstpop::embedded::start_embedded(port).await
    });
    let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStopGstPopServiceHost<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    let status = HOST_RUNTIME.block_on(async {
        crate::backend::gstpop::embedded::stop_embedded().await
    });
    let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeGetGstPopServiceStatus<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
) -> jni::sys::jstring {
    let status = crate::backend::gstpop::embedded::embedded_status();
    let json = serde_json::to_string(&status).unwrap_or_else(|_| "{}".into());
    env.new_string(json).expect("new_string").into_raw()
}

#[cfg(target_os = "android")]
fn parse_config_port(json: &str) -> Option<u16> {
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let url = v.get("gstpop_url")?.as_str()?;
    Some(crate::backend::gstpop::embedded::url_port(url))
}
```

## 3.2 Where does the tokio runtime come from?

`main_inner` already builds a runtime
(`src/lib.rs:1750-1754`). To make it reachable from the JNI calls
above without threading it through every callsite, stash a `Handle`:

```rust
// src/lib.rs — near the top, alongside other lazy_static!.

#[cfg(target_os = "android")]
lazy_static::lazy_static! {
    /// Dedicated multi-thread runtime for host (service) JNI calls.
    /// Separate from the slint event-loop runtime so JNI calls from a
    /// binder thread never block the UI thread.
    pub(crate) static ref HOST_RUNTIME: tokio::runtime::Runtime =
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("gstpop-host")
            .build()
            .expect("build HOST_RUNTIME");
}
```

`HOST_RUNTIME.block_on(...)` is safe to call from a JVM binder thread
because that thread is **not** owned by tokio. Don't call it from a
thread that's already running on the UI runtime — there are no such
callers in this design.

## 3.3 `GstPopServiceBridge.java`

New file: `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java`.

```java
package org.fcast.android.sender;

import android.content.Context;
import android.content.Intent;
import android.util.Log;

/**
 * Thin wrapper around the native gst-pop daemon lifecycle and the
 * Android service that hosts it. All UI/Activity code MUST go
 * through this class — direct startService / native calls bypass the
 * lifecycle bookkeeping.
 */
public final class GstPopServiceBridge {
    private static final String TAG = "GstPopServiceBridge";

    private GstPopServiceBridge() {}

    // ── Public API (from UI / Rust / wherever) ────────────────────────

    /**
     * Request the service to start. Returns immediately — the service
     * itself drives the native start on its onStartCommand thread.
     * UI polls {@link #queryStatus()} (or listens to a binder, if you
     * add one later) for the resulting state.
     */
    public static void start(Context context, String configJson) {
        Intent intent = new Intent(context, GstPopService.class)
            .setAction(GstPopService.ACTION_START)
            .putExtra(GstPopService.EXTRA_CONFIG_JSON,
                      configJson == null ? "{}" : configJson);
        try {
            context.startForegroundService(intent);
        } catch (Exception e) {
            Log.e(TAG, "startForegroundService failed: " + e);
        }
    }

    /** Request graceful shutdown. */
    public static void stop(Context context) {
        Intent intent = new Intent(context, GstPopService.class)
            .setAction(GstPopService.ACTION_STOP);
        try {
            context.startService(intent);
        } catch (Exception e) {
            Log.e(TAG, "stopService failed: " + e);
        }
    }

    /**
     * Synchronous status query. Returns the JSON-serialised
     * EmbeddedStatus from Rust. Safe to call from any thread.
     *
     *   {"state":"running","bind":"127.0.0.1","port":9000,
     *    "last_error":null,"started_at_unix_ms":1731234567890}
     */
    public static String queryStatus() {
        return nativeGetGstPopServiceStatus();
    }

    // ── Called only from GstPopService — never from UI code ──────────

    static String nativeStart(String configJson) {
        return nativeStartGstPopServiceHost(configJson);
    }
    static String nativeStop() {
        return nativeStopGstPopServiceHost();
    }

    // ── Native exports (see Java_org_fcast_android_sender_GstPopServiceBridge_* in src/lib.rs) ──

    private static native String nativeStartGstPopServiceHost(String configJson);
    private static native String nativeStopGstPopServiceHost();
    private static native String nativeGetGstPopServiceStatus();
}
```

The native methods live on `GstPopServiceBridge` rather than
`MainActivity` so the service can call them after the activity is
gone. They are package-private (`static native String …`) — only the
bridge class is the JNI entrypoint.

## 3.4 Loading the native library

`MainActivity.java` already calls `System.loadLibrary("fcastsender")`
(line 213). That covers all symbols in the same `cdylib`, including
the ones added in 3.1, **provided the service is in the same process
as the activity** (which it is — no `android:process="…"` attribute
on the `<service>` block).

If you ever decide to give `GstPopService` its own process, you'd
need a duplicate `loadLibrary` call from a static initialiser in
`GstPopServiceBridge` — keep that in mind, but don't do it now.

## 3.5 Sanity test

After building the cdylib once, you can hand-verify the symbols are
exported:

```bash
$ aarch64-linux-android-nm --defined-only \
    app/src/main/jniLibs/arm64-v8a/libfcastsender.so | \
    grep GstPopServiceBridge
0000000000abcdef T Java_org_fcast_android_sender_GstPopServiceBridge_nativeGetGstPopServiceStatus
0000000000abcdf0 T Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost
0000000000abcdf1 T Java_org_fcast_android_sender_GstPopServiceBridge_nativeStopGstPopServiceHost
```

If any of those symbols is missing or has the wrong name, JNI lookup
at runtime throws `UnsatisfiedLinkError` — there is no graceful
fallback.

Next: [04-android-service.md](./04-android-service.md).
