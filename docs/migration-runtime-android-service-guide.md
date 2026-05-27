# Migration Runtime → Android service implementation guide

Step-by-step recipe to host the existing Rust **migration runtime**
(`migration::runtime::start_graph_runtime` / `shutdown_graph_runtime`)
inside a dedicated Android foreground service, mirroring the existing
`GstPopService` pattern.

> **Status:** design + recipe only. **No source changes** are made by this
> document — it is the spec for the eventual implementation PR. All code
> blocks are illustrative; copy them into the listed files when implementing.

---

## 0. Plan review (cross-check against the actual code)

The submitted Version 1 / Version 2 plans are accurate at a high level
(mirror `GstPopService`, add 3 JNI exports, add a manifest entry). The
following deltas were found by reading the current source on `main`
and **MUST** be respected when implementing — they are the spots where
naïve copy-paste from `GstPopService` will not compile or will behave
incorrectly.

| # | Plan statement | Reality on `main` | Action |
|---|----------------|-------------------|--------|
| 1 | "Return JSON with at least `{"state":"running"\|"stopped"\|"error", ...}` to match the pattern GstPopService uses." | `migration::runtime::start_graph_runtime()` returns `Result<()>` — there is **no** `EmbeddedStatus`-equivalent struct that already serialises to that shape. Source: `src/migration/runtime.rs:302-320`. | The JNI layer must **synthesise** the JSON itself (see [§1.2](#12-status-json-shape)). |
| 2 | "Follow the same pattern as the existing GstPopServiceBridge JNI methods around line 2994-3037" — uses `HOST_RUNTIME.block_on(...)`. | `start_graph_runtime` / `shutdown_graph_runtime` are **synchronous** (they spawn their own background threads internally). There is no async future to drive. Source: `src/lib.rs:88-94` defines `HOST_RUNTIME`; `runtime.rs:302-320`. | Do **not** wrap calls in `HOST_RUNTIME.block_on(async { … })`. Call them directly. JNI binder threads are fine for the millisecond-scale work these functions do. |
| 3 | "Update notification text from 'gst-pop' to 'Migration Runtime'." Implicit: copy `describe(statusJson)` 1:1. | `GstPopService.describe(...)` reads `bind` and `port` fields from the status JSON. The migration runtime has no port concept exposed in its status. Source: `GstPopService.java:138-156`. | The `describe(...)` helper in `MigrationRuntimeService` must read only `state` and `last_error`. **Delete** the bind/port references. |
| 4 | "Use foregroundServiceType=\"dataSync\" (same as GstPopService)." | Correct — the matching permission `android.permission.FOREGROUND_SERVICE_DATA_SYNC` is already declared. Source: `AndroidManifest.xml:7`. | Use as planned. No new `<uses-permission>` needed. |
| 5 | "Notification ID 3 (GstPopService uses 2, ScreenCaptureService uses 1)." | Confirmed. Sources: `ScreenCaptureService.java:57` (`startForeground(1, …)`); `GstPopService.java:26` (`NOTIFICATION_ID = 2`). | Use `3` as planned. |
| 6 | Version 2: "After the existing GstPopService declaration (line 30)…" | The `GstPopService` block in `AndroidManifest.xml` actually spans lines **26-30**. | Insert the new `<service>` after line 30. |
| 7 | "MainActivity.java — may need to import/reference the new service (e.g., for starting it from UI)." | The existing `GstPopServiceBridge` is **not** called from `MainActivity` either. It is invoked from Rust via JNI reflection in `src/backend/gstpop/service.rs`. Source: `MainActivity.java` contains zero `GstPop*` references. | Do **not** add anything to `MainActivity.java` for the migration runtime service either. Wiring will be from Rust (see [§5](#5-optional-rust-side-helpers-to-trigger-the-service)). |
| 8 | "queryStatus() — calls nativeGetMigrationRuntimeStatus()" | The natural Rust implementation is `try_handle_command_json("{\"getinfo\":{}}")`. That function **always returns a String** (never errors). Source: `runtime.rs:349-366`. | Status JNI export wraps that call and re-shapes the response into the `{state, …}` envelope (see [§1.2](#12-status-json-shape)). |
| 9 | "start(Context, String) — starts the MigrationRuntimeService via Intent." | The migration runtime currently takes **no** start-time config. The `configJson` parameter is preserved purely for symmetry and forward-compatibility. | Plumb it through, but ignore it on the Rust side for now. Mark this in code comments. |
| 10 | Not mentioned in plan. | `GstPopService` performs an "externally_owned" check that triggers a 500 ms self-stop dance. Source: `GstPopService.java:126-133`. | The migration runtime is **always in-process** — there is no external owner. **Omit** the `isExternallyOwned` branch entirely from `MigrationRuntimeService`. |

The rest of the plan (file names, package, action constants, channel id,
private static native declarations) is correct as written.

---

## 1. Rust JNI exports — `src/lib.rs`

Three new `#[cfg(target_os = "android")]` JNI exports are added next to
the existing `Java_org_fcast_android_sender_GstPopServiceBridge_*`
functions (currently at `src/lib.rs:2988-3037`). They map 1:1 to the
three `private static native` methods declared in
`MigrationRuntimeServiceBridge.java`.

### 1.1 Three new exports

Insert immediately **after** the `parse_gstpop_config_port` helper at
`src/lib.rs:3040-3044` (i.e. after the existing gst-pop JNI block,
before `#[cfg(test)]`):

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

### 1.2 Status JSON shape

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

The shape is intentionally a **strict subset** of `EmbeddedStatus` so
the same `describe(...)` style helper on the Java side can be reused
(without the `bind`/`port` fields).

> **Why no `HOST_RUNTIME.block_on(...)`?** The gst-pop variant uses it
> because `start_embedded` / `stop_embedded` are `async`. The migration
> runtime equivalents are synchronous (they internally spawn
> `std::thread::Builder` threads), so dispatching them onto a separate
> async runtime adds no value. Calling on the JNI binder thread is
> safe — the calls return in milliseconds.

---

## 2. Java bridge — `app/src/main/java/org/fcast/android/sender/MigrationRuntimeServiceBridge.java`

Mirror of `GstPopServiceBridge.java`. Full file content:

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

**Visibility rationale (same as `GstPopServiceBridge`):**

* `start` / `stop` / `queryStatus` are `public static` — entry points
  for the rest of the app (Rust JNI reflection, future Slint callbacks).
* `nativeStart` / `nativeStop` are package-private (`static`, no
  modifier) so that only `MigrationRuntimeService` in the same package
  can invoke them, enforcing the rule that lifecycle bookkeeping
  always flows through the service.
* The three `nativeXxx*Host` methods are `private static native` —
  matched 1:1 by JNI symbols in [§1.1](#11-three-new-exports).

---

## 3. Android service — `app/src/main/java/org/fcast/android/sender/MigrationRuntimeService.java`

Mirror of `GstPopService.java`, with the gst-pop-specific deviations
called out in [§0](#0-plan-review-cross-check-against-the-actual-code).
Full file content:

```java
package org.fcast.android.sender;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;

import androidx.annotation.Nullable;

import org.json.JSONException;
import org.json.JSONObject;

public final class MigrationRuntimeService extends Service {
    private static final String TAG = "MigrationRuntimeService";

    public static final String ACTION_START      = "org.fcast.android.sender.MIGRATION_RUNTIME_START";
    public static final String ACTION_STOP       = "org.fcast.android.sender.MIGRATION_RUNTIME_STOP";
    public static final String EXTRA_CONFIG_JSON = "config_json";

    private static final int    NOTIFICATION_ID = 3; // 1=ScreenCapture, 2=GstPop
    private static final String CHANNEL_ID      = "org.fcast.android.sender.MigrationRuntimeService";

    @Override
    public void onCreate() {
        super.onCreate();
        ensureChannel();
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        String action = intent != null ? intent.getAction() : null;
        Log.d(TAG, "onStartCommand action=" + action);

        if (ACTION_START.equals(action)) {
            // Call startForeground before any blocking work — Android 14 enforces
            // this within 5s of startForegroundService().
            startForeground(NOTIFICATION_ID, buildNotification("Starting migration runtime\u2026"));

            String config = intent.getStringExtra(EXTRA_CONFIG_JSON);
            String statusJson = MigrationRuntimeServiceBridge.nativeStart(config != null ? config : "");
            Log.d(TAG, "nativeStart -> " + statusJson);
            updateNotification(statusJson);

            if (isErrorState(statusJson)) {
                stopForeground(STOP_FOREGROUND_REMOVE);
                stopSelf();
                return START_NOT_STICKY;
            }

            return START_STICKY;
        }

        if (ACTION_STOP.equals(action)) {
            String statusJson = MigrationRuntimeServiceBridge.nativeStop();
            Log.d(TAG, "nativeStop -> " + statusJson);
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
            return START_NOT_STICKY;
        }

        // Sticky-restart with null intent: don't auto-restart the runtime —
        // leave it to the UI/Rust caller to ask for ACTION_START again.
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        // Defensive: ensure Rust state matches reality if we're torn down
        // without an explicit ACTION_STOP (stopWithTask=false, but the process
        // can still be killed under memory pressure).
        MigrationRuntimeServiceBridge.nativeStop();
        super.onDestroy();
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        // No binder — callers poll via MigrationRuntimeServiceBridge.queryStatus().
        return null;
    }

    // ── Notification helpers ──────────────────────────────────────────────────

    private void ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return;
        NotificationChannel channel = new NotificationChannel(
            CHANNEL_ID, "Migration runtime", NotificationManager.IMPORTANCE_LOW);
        channel.setDescription("Embedded migration runtime hosting");
        channel.setShowBadge(false);
        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm != null) nm.createNotificationChannel(channel);
    }

    private Notification buildNotification(String text) {
        Intent open = new Intent(this, MainActivity.class)
            .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent openPi = PendingIntent.getActivity(this, 0, open,
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);

        Intent stop = new Intent(this, MigrationRuntimeService.class).setAction(ACTION_STOP);
        PendingIntent stopPi = PendingIntent.getService(this, 0, stop,
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);

        return new Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("FCast migration runtime")
            .setContentText(text)
            .setContentIntent(openPi)
            .addAction(new Notification.Action.Builder(0, "Stop", stopPi).build())
            .setOngoing(true)
            .setShowWhen(false)
            .build();
    }

    private void updateNotification(String statusJson) {
        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm == null) return;
        nm.notify(NOTIFICATION_ID, buildNotification(describe(statusJson)));
    }

    private static String describe(String statusJson) {
        try {
            JSONObject o = new JSONObject(statusJson);
            String state = o.optString("state", "unknown");
            switch (state) {
                case "running":  return "Migration runtime running";
                case "starting": return "Starting migration runtime\u2026";
                case "stopped":  return "Migration runtime stopped";
                case "error":
                    String e = o.optString("last_error", "unknown error");
                    return "Migration runtime error: " + e;
                default: return "Migration runtime " + state;
            }
        } catch (JSONException e) {
            return "Migration runtime";
        }
    }

    private static boolean isErrorState(String statusJson) {
        try {
            return "error".equals(new JSONObject(statusJson).optString("state"));
        } catch (JSONException e) {
            return false;
        }
    }
}
```

**Differences from `GstPopService` (intentional):**

1. `describe()` reads only `state` / `last_error` — no `bind` / `port`
   (see [§0 row 3](#0-plan-review-cross-check-against-the-actual-code)).
2. No `isExternallyOwned` branch or 500 ms self-stop dance — the
   migration runtime is always in-process (see [§0 row 10](#0-plan-review-cross-check-against-the-actual-code)).
3. Channel display name and notification title strings.
4. Action constants and channel id namespaced per service.

---

## 4. AndroidManifest.xml diff

Insert one new `<service>` block immediately after the existing
`GstPopService` declaration (after line 30):

```diff
         <service
             android:name=".GstPopService"
             android:exported="false"
             android:stopWithTask="false"
             android:foregroundServiceType="dataSync" />

+        <service
+            android:name=".MigrationRuntimeService"
+            android:exported="false"
+            android:stopWithTask="false"
+            android:foregroundServiceType="dataSync" />
+

         <activity
             android:name="com.journeyapps.barcodescanner.CaptureActivity"
```

**No new `<uses-permission>` entries are required.**
`FOREGROUND_SERVICE` (line 5) and `FOREGROUND_SERVICE_DATA_SYNC`
(line 7) are already declared and cover the new service.

---

## 5. (Optional) Rust-side helpers to trigger the service

Out of scope for this guide's "minimum viable" file list, but worth
calling out so future work matches the existing pattern. The gst-pop
service is invoked from Rust via JNI reflection in
`src/backend/gstpop/service.rs:38-92` — that file:

* Calls `crate::android_context()` to get a VM + activity reference.
* Resolves `org.fcast.android.sender.GstPopServiceBridge` via the
  **activity's** ClassLoader (NOT `env.find_class`, which uses the
  bootstrap loader and cannot see app classes — see the doc comment at
  `service.rs:11-12`).
* Invokes `start(Context, String)` / `stop(Context)` reflectively.

When a Rust caller eventually needs to drive the migration runtime
through the service, create an analogous file (e.g.
`src/migration/service.rs`) that:

1. Reuses the `load_app_class` helper currently inside
   `src/backend/gstpop/service.rs` (consider promoting it to a shared
   `crate::android` module rather than duplicating).
2. Calls
   `org.fcast.android.sender.MigrationRuntimeServiceBridge.start(Context, String)`
   / `.stop(Context)`.

This is **deferred** — none of the files listed in the original plan
require this helper to exist. Direct in-process calls to
`migration::runtime::start_graph_runtime()` still work everywhere they
already do. The service simply gives the OS a foreground-process anchor
once a UI surface starts driving migration commands.

> **Don't add `MigrationRuntimeService`/`Bridge` imports to `MainActivity.java`.**
> The existing gst-pop service is also not referenced from `MainActivity`
> (verified: zero matches). Follow the same pattern.

---

## 6. Build / package gotchas

* **No `build.gradle` changes.** The new files live in the existing
  `org.fcast.android.sender` package; the cdylib (`libfcastsender.so`)
  picks up the new JNI exports because they are conditional on
  `cfg(target_os = "android")` and share the existing crate build.
* **Symbol matching.** The JNI symbol must be byte-for-byte:
  ```
  Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost
  Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStopMigrationRuntimeHost
  Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeGetMigrationRuntimeStatus
  ```
  If you rename either side, rename both.
* **`#[unsafe(no_mangle)]`**, **`#[allow(non_snake_case)]`**, and
  **`pub extern "C" fn`** are required exactly as shown — these match
  the existing gst-pop exports at `src/lib.rs:2991-3037`.
* **ProGuard / R8.** If/when shrinking is enabled later, the bridge
  classes will need `-keep` rules (`-keep class org.fcast.android.sender.MigrationRuntimeServiceBridge { *; }`).
  Not required for the current debug build, but flagged so it's not
  forgotten alongside the existing `GstPopServiceBridge`.

---

## 7. Test plan (manual, since this is guide-only today)

When the implementation PR lands, the following acceptance checks
verify it end-to-end. They mirror the gst-pop guide's
[`10-test-plan.md`](./gstpop-android-service-guide/10-test-plan.md).

1. **`cargo check --target aarch64-linux-android`** — compiles the
   three new JNI exports against the migration-runtime functions.
2. **`./gradlew assembleDebug`** — verifies the two new Java files and
   the manifest entry are picked up.
3. **`adb shell dumpsys activity services org.fcast.android.sender`** —
   after triggering start, confirms `MigrationRuntimeService` is in
   `foreground=true` state with `foregroundServiceType=dataSync` and
   notification ID 3.
4. **Notification surface** — check that the title reads "FCast
   migration runtime" and the text cycles `Starting…` → `Migration
   runtime running` after start completes.
5. **`MigrationRuntimeServiceBridge.queryStatus()`** — call from a
   debug surface (e.g. ADB shell via a maintenance activity, or a
   future Slint debug button) and confirm it returns
   `{"state":"running"}` while the service is up and
   `{"state":"stopped"}` after stop.
6. **`adb shell stop org.fcast.android.sender`** — kill the process
   and confirm `nativeStop()` is invoked via `onDestroy()` (visible
   in `logcat -s MigrationRuntimeService MigrationRuntimeServiceBridge`).
7. **Task removal** — swipe away the app from Recents. Because
   `android:stopWithTask="false"`, the service must **survive** the
   task-removal (unlike `ScreenCaptureService`, which uses
   `stopWithTask="true"`).

> Run the pre-commit hooks (`pre-commit run --all-files`) before
> opening the PR. Note: the existing hooks check `.slint` files only,
> so this docs-only PR will be unaffected, but the same hook config
> will also run on the eventual implementation PR.

---

## 8. File change list (for the eventual implementation PR)

| # | File | Action | LOC (approx) |
|---|------|--------|-------|
| 1 | `app/src/main/java/org/fcast/android/sender/MigrationRuntimeService.java` | **CREATE** | ~150 |
| 2 | `app/src/main/java/org/fcast/android/sender/MigrationRuntimeServiceBridge.java` | **CREATE** | ~70 |
| 3 | `app/src/main/AndroidManifest.xml` | **MODIFY** — 5-line insert after current line 30 | +5 |
| 4 | `src/lib.rs` | **MODIFY** — three JNI exports + status helper after current line 3044 | +60 |

Everything else (build.gradle, `MainActivity.java`,
`src/migration/runtime.rs`, `src/migration/mod.rs`) is **untouched** by
the minimum viable implementation.

---

## 9. References (line-pinned)

| Topic | File | Lines |
|-------|------|-------|
| `GstPopService` template (full lifecycle) | `app/src/main/java/org/fcast/android/sender/GstPopService.java` | 1-174 |
| `GstPopServiceBridge` template | `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java` | 1-70 |
| Notification ID precedent (`= 1`) | `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java` | 57 |
| Manifest insertion point | `app/src/main/AndroidManifest.xml` | 26-30 |
| Existing GstPop JNI exports (template) | `src/lib.rs` | 2988-3037 |
| `HOST_RUNTIME` lazy_static (NOT needed here) | `src/lib.rs` | 82-95 |
| `jstring_to_string` helper | `src/lib.rs` | 2588 |
| `migration::runtime::start_graph_runtime` | `src/migration/runtime.rs` | 302-310 |
| `migration::runtime::shutdown_graph_runtime` | `src/migration/runtime.rs` | 312-320 |
| `migration::runtime::try_handle_command_json` | `src/migration/runtime.rs` | 349-366 |
| Rust → Java reflection precedent | `src/backend/gstpop/service.rs` | 1-92 |
| Related service-abstraction guide (different angle) | `docs/service-abstraction-refactor-guide/STEP-03-migration-service-wrapper.md` | 1-138 |
| Related Android-service guide for gst-pop | `docs/gstpop-android-service-guide/README.md` | 1-62 |
