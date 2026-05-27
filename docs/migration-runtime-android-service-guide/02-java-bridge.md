# 02 — Java bridge (`MigrationRuntimeServiceBridge.java`)

Mirror of `GstPopServiceBridge.java` (see
`app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java:1-70`).
Full source for the new file follows.

## 2.1 Full file content

```java
package org.fcast.android.sender;

import android.content.Context;
import android.content.Intent;
import android.util.Log;

/**
 * Thin wrapper around the native migration-runtime lifecycle and the Android
 * service that hosts it. All UI/Activity/Rust code MUST go through this class —
 * direct startService / native calls bypass the lifecycle bookkeeping.
 *
 * Mirrors {@link GstPopServiceBridge}; the migration runtime currently takes
 * no start-time config, but a configJson parameter is preserved for symmetry.
 */
public final class MigrationRuntimeServiceBridge {
    private static final String TAG = "MigrationRuntimeServiceBridge";

    private MigrationRuntimeServiceBridge() {}

    // ── Public API ────────────────────────────────────────────────────────────

    /**
     * Request the service to start. Returns immediately — the service drives
     * the native start on its onStartCommand thread. UI polls
     * {@link #queryStatus()} for the resulting state.
     */
    public static void start(Context context, String configJson) {
        Intent intent = new Intent(context, MigrationRuntimeService.class)
            .setAction(MigrationRuntimeService.ACTION_START)
            .putExtra(MigrationRuntimeService.EXTRA_CONFIG_JSON,
                      configJson == null ? "{}" : configJson);
        try {
            context.startForegroundService(intent);
        } catch (Exception e) {
            Log.e(TAG, "startForegroundService failed: " + e);
        }
    }

    /** Request graceful shutdown. */
    public static void stop(Context context) {
        Intent intent = new Intent(context, MigrationRuntimeService.class)
            .setAction(MigrationRuntimeService.ACTION_STOP);
        try {
            context.startService(intent);
        } catch (Exception e) {
            Log.e(TAG, "stopService failed: " + e);
        }
    }

    /**
     * Synchronous status query. Returns the JSON-serialised status from Rust.
     * Safe to call from any thread.
     */
    public static String queryStatus() {
        return nativeGetMigrationRuntimeStatus();
    }

    // ── Called only from MigrationRuntimeService — not from UI code ──────────

    static String nativeStart(String configJson) {
        return nativeStartMigrationRuntimeHost(configJson);
    }

    static String nativeStop() {
        return nativeStopMigrationRuntimeHost();
    }

    // ── Native exports (Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_* in lib.rs) ──

    private static native String nativeStartMigrationRuntimeHost(String configJson);
    private static native String nativeStopMigrationRuntimeHost();
    private static native String nativeGetMigrationRuntimeStatus();
}
```

## 2.2 Visibility rationale (same as `GstPopServiceBridge`)

| Visibility | Members | Rationale |
|---|---|---|
| `public static` | `start`, `stop`, `queryStatus` | Entry points for the rest of the app — Rust JNI reflection, Slint callbacks via the Rust caller helper in step 5. |
| package-private (default) `static` | `nativeStart`, `nativeStop` | Only `MigrationRuntimeService` in the same package can invoke them, enforcing the rule that lifecycle bookkeeping always flows through the service. |
| `private static native` | `nativeStartMigrationRuntimeHost`, `nativeStopMigrationRuntimeHost`, `nativeGetMigrationRuntimeStatus` | Matched 1:1 by JNI symbols in [01-rust-jni-bridge.md §1.1](./01-rust-jni-bridge.md#11-three-new-exports). |

## 2.3 Class signature

`public final class` — matches `GstPopServiceBridge`. `final` because
the bridge has zero state and no inheritance hooks; making it `final`
prevents accidental subclassing.

`private` constructor — purely static API; the class is a namespace.

## 2.4 Why the `configJson` parameter is kept despite being unused

The migration runtime has no start-time configuration today. Even so,
keeping the parameter in `start(Context, String)` means:

1. **Matching call-site signature** with `GstPopServiceBridge.start(Context, String)`.
   The Rust reflection helper in step 5 invokes both with
   `(Landroid/content/Context;Ljava/lang/String;)V`, so a uniform
   helper can call either bridge.
2. **Forward compatibility.** When the runtime grows config knobs
   (bind address, log filter, …), the wire stays the same; only the
   Rust JNI export starts honouring the JSON payload.

The JNI export discards the JString today
(see [01-rust-jni-bridge.md §1.1](./01-rust-jni-bridge.md#11-three-new-exports)),
which is the only place that should ever change.

## 2.5 Threading notes

* `nativeGetMigrationRuntimeStatus()` is called from any thread — the
  Rust side does no blocking I/O (it's a one-shot
  `try_handle_command_json("getinfo")` call).
* `start(Context, …)` / `stop(Context)` MUST be called on a thread
  that holds a reference to a `Context`. The Rust caller helper in
  step 5 attaches the JNI VM to its calling thread before invoking
  these.
* `nativeStart` / `nativeStop` should only ever be called from
  `MigrationRuntimeService.onStartCommand(...)` — they run on the
  binder thread (the system delivers `onStartCommand` on the main
  thread by default, but it's safe either way since the underlying
  Rust calls block for at most a few milliseconds).
