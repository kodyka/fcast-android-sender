# Promoting embedded gst-pop to an Android service

Step-by-step implementation guide for moving the embedded gst-pop
daemon out of `GstPopBackend::probe()` and into a dedicated Android
service, with clear ownership across Rust ↔ JNI ↔ Java ↔ Slint.

> **Status:** design + recipe only. No code changes are made by this
> document. Snippets are illustrative; copy-paste targets are noted.

---

## 0. Plan review

The 15-step plan you wrote is sound. A few deltas based on the current
code that you should bake into the work as you go:

1. **There is no Kotlin/Java orchestration layer today.** All glue
   lives in `app/src/main/java/org/fcast/android/sender/MainActivity.java`
   (`NativeActivity` subclass; ~1158 lines) and a single sibling service
   `ScreenCaptureService.java`. Slint runs entirely inside the Rust
   `cdylib` via `android-activity`. The "Java bridge class" idea is
   correct; just be aware you are *creating* that layer, not extending
   an existing controller.
2. **A foreground-service template already exists** — `ScreenCaptureService`
   (`mediaProjection` type) is the closest analogue. The new
   `GstPopService` should mirror its notification + `START_STICKY`
   shape, *not* invent a new pattern. See
   `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java`.
3. **JNI naming is load-bearing.** Every new native must be exported as
   `Java_org_fcast_android_sender_<Class>_<method>` to match the
   declared `package`. Don't free-form the symbol names.
4. **Implicit startup is *partially* gated already.** PR #8 added a
   pre-bind TCP probe in
   `src/backend/gstpop/embedded.rs:21-62`
   so `ensure_started` defers to an external listener if one is
   present. Your plan to "remove the implicit start from probe" is
   still correct — this guide just notes the current state is a soft
   defer, not a hard removal.
5. **Slint already exposes the state surface** the UI needs. The
   `Bridge` global has `media-backend-state` (enum
   `disconnected | probing | ready | error`),
   `media-backend-status-text`, and `media-backend-error-text` in
   `ui/bridge.slint:289-296`.
   We only need to enrich the existing state machine, not invent a new
   one. Adding a `Starting` variant to `MediaBackendState` is
   recommended (see §8 below).
6. **`BackendLifecycle` is the single funnel** for backend changes from
   Slint. The Apply/Save/Probe wiring is in
   `src/backend/lifecycle.rs:31-85`.
   That's where the service-start hook plugs in.
7. **`migration` backend is unrelated to this work.** Moving gst-pop
   into a service does not change `MigrationBackend`'s in-process
   runtime, and the *runtime command* path is still Java's
   `nativeGraphCommand` → Rust → `migration::runtime` regardless of the
   selected media backend. Your plan's "potential concerns" note is
   accurate and should stay out of scope here.

With those corrections folded in, the order of operations becomes:

```
A. Decide continuity contract (your steps 1–2)
B. Refactor Rust daemon control into start/stop/status APIs (steps 3, 8)
C. Add JNI entrypoints + Java bridge (steps 4–5)
D. Add GstPopService (steps 6, 7, 10, 11)
E. Rewire BackendLifecycle::apply to use the bridge (steps 8–9)
F. Tighten probe() to a pure connectivity check (step 15)
G. Slint UI state additions + propagation (steps 9, 13)
H. Tests + cleanup (steps 14, 15)
```

The rest of this document walks each milestone.

---

## A. Decide continuity contract

| Continuity goal | Mechanism |
|---|---|
| Survive config changes only (rotation, resize) | Already handled — `MainActivity` declares `configChanges="keyboardHidden|orientation|screenSize"`. **No service needed.** |
| Survive activity backgrounded but process alive | A plain in-process `Service` (started, not bound, no foreground) is sufficient. |
| Survive activity finish / task swipe-away | **Foreground service** required (Android 14+ enforces this for any long-running socket-holding work). |
| Survive process death | START_STICKY foreground + persisted config on disk; on relaunch, service restarts and re-binds 127.0.0.1:9000. |

**Recommendation:** target row 3 (foreground service). It is the only
configuration that lets a localhost gst-pop daemon keep serving while
the user is in another app. Anything weaker than that gets killed by
Android's background execution limits on a real device within minutes.

If you decide row 1 is enough, **stop reading**: the work in §B alone
(explicit Rust start/stop API + remove implicit probe-start) covers it,
and no Java/service code is needed.

---

## B. Refactor Rust daemon control into start/stop/status APIs

### B.1 New module: `src/backend/gstpop/embedded.rs`

Today `embedded.rs` exposes one public function:

```rust
pub async fn ensure_started(port: u16) -> Result<()>;
```

Split that into an explicit lifecycle. Keep `ensure_started` as a thin
compatibility shim for now (we delete its caller in §F), but add:

```rust
// src/backend/gstpop/embedded.rs

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EmbeddedState {
    Stopped,
    Starting,
    Running { externally_owned: bool },
    Error,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct EmbeddedStatus {
    pub state: EmbeddedState,
    pub bind: String,          // "127.0.0.1"
    pub port: u16,             // 9000
    pub last_error: Option<String>,
    pub started_at_unix_ms: Option<u64>,
}

/// Start the embedded gst-pop server (idempotent). Returns the resulting
/// status. `start_embedded` never panics — failures are reflected in
/// `EmbeddedStatus::state == Error` with `last_error` populated.
pub async fn start_embedded(port: u16) -> EmbeddedStatus { /* … */ }

/// Stop the embedded gst-pop server if we own it. No-op if it is
/// externally owned or already stopped. Always returns the current
/// status post-call.
pub async fn stop_embedded() -> EmbeddedStatus { /* … */ }

/// Cheap snapshot of current state. Does not perform any network I/O
/// beyond what is already cached.
pub fn embedded_status() -> EmbeddedStatus { /* … */ }
```

Implementation notes:

- Reuse the existing `CLAIMED` / `READY` atomics, but add a third
  `STATE: RwLock<EmbeddedState>` so transitions are observable. The
  atomics are still the synchronisation primitive; the enum is the
  read-only view.
- The "external listener already on the port" branch in
  `src/backend/gstpop/embedded.rs:31-43`
  becomes `EmbeddedState::Running { externally_owned: true }`. That
  flag is what tells `stop_embedded` to **not** try to kill someone
  else's server.
- `start_embedded` should be the only place that calls
  `ServerHandle::start`. Drop the `match start_server(port).await`
  block from `ensure_started` once §F removes the last in-tree caller.

### B.2 Don't forget shutdown

`ServerHandle` is currently held in a `static Mutex<Option<…>>` so it
lives for the process lifetime. For service-owned mode we want
`stop_embedded` to actually drop it. Move the handle storage from a
process-lifetime static into something `stop_embedded` can take and
drop:

```rust
static HANDLE: parking_lot::Mutex<Option<ServerHandle>> = parking_lot::const_mutex(None);

pub async fn stop_embedded() -> EmbeddedStatus {
    let externally_owned = matches!(*STATE.read(),
        EmbeddedState::Running { externally_owned: true });
    if externally_owned {
        // Not ours to stop.
        return embedded_status();
    }
    if let Some(handle) = HANDLE.lock().take() {
        // ServerHandle's Drop already aborts the task; this just makes
        // it explicit and synchronous for the JNI caller.
        drop(handle);
    }
    READY.store(false, Ordering::Release);
    CLAIMED.store(false, Ordering::Release);
    *STATE.write() = EmbeddedState::Stopped;
    embedded_status()
}
```

---

## C. JNI entrypoints + Java bridge

### C.1 New JNI exports

Add three exports at the bottom of `src/lib.rs`, next to the existing
`Java_org_fcast_android_sender_MainActivity_*` block
(see `src/lib.rs:2456-2483`
for the canonical pattern):

```rust
// src/lib.rs — add near the existing nativeGraphCommand block.

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn Java_org_fcast_android_sender_GstPopServiceBridge_nativeStartGstPopServiceHost<'local>(
    mut env: jni::JNIEnv<'local>,
    _class: jni::objects::JClass<'local>,
    config_json: jni::objects::JString<'local>,
) -> jni::sys::jstring {
    let config = jstring_to_string(&mut env, &config_json).unwrap_or_default();
    let port = parse_config_port(&config).unwrap_or(9000);

    // Drive the async API on the global tokio runtime that lib.rs already
    // owns (see `let runtime = tokio::runtime::Runtime::new().unwrap();`
    // in main_inner). We use a oneshot to block this JNI call until the
    // start attempt completes — JNI callers expect synchronous results.
    let status = futures::executor::block_on(async {
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
    let status = futures::executor::block_on(async {
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
```

Important: do **not** use `futures::executor::block_on` if your tokio
runtime is multi-threaded *and* the JNI thread is the same one running
tokio tasks. The current `main_inner` builds a single-runtime model
(see `src/lib.rs:1750-1764`),
so JNI calls from the Java side arrive on the JVM's binder thread and
are safe to block. If you ever move to a per-task spawn model, switch
to a `tokio::runtime::Handle::block_on` against a stashed handle
instead.

### C.2 New Java bridge class

Create `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java`:

```java
package org.fcast.android.sender;

import android.content.Context;
import android.content.Intent;
import android.util.Log;

/**
 * Thin wrapper around the native gst-pop daemon lifecycle and the
 * Android service that hosts it. UI/Activity code MUST go through this
 * class — direct startService / native calls bypass the lifecycle
 * bookkeeping.
 */
public final class GstPopServiceBridge {
    private static final String TAG = "GstPopServiceBridge";

    private GstPopServiceBridge() {}

    /** Request the service to start; returns immediately. UI polls
     *  {@link #queryStatus()} for the resulting state. */
    public static void start(Context context, String configJson) {
        Intent intent = new Intent(context, GstPopService.class)
            .setAction(GstPopService.ACTION_START)
            .putExtra(GstPopService.EXTRA_CONFIG_JSON, configJson);
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

    /** Synchronous status query. Returns the JSON serialised
     *  EmbeddedStatus from Rust. Safe to call from any thread. */
    public static String queryStatus() {
        return nativeGetGstPopServiceStatus();
    }

    // Called only from GstPopService — never from UI code.
    static String nativeStart(String configJson) {
        return nativeStartGstPopServiceHost(configJson);
    }
    static String nativeStop() {
        return nativeStopGstPopServiceHost();
    }

    // Native exports (see Java_org_fcast_android_sender_GstPopServiceBridge_*
    // in src/lib.rs).
    private static native String nativeStartGstPopServiceHost(String configJson);
    private static native String nativeStopGstPopServiceHost();
    private static native String nativeGetGstPopServiceStatus();
}
```

The native methods live on `GstPopServiceBridge` rather than
`MainActivity` so the service can call them after the activity is
gone. They are package-private (`static native String …`) — only the
bridge class is the JNI entrypoint.

---

## D. The Android service

### D.1 GstPopService

Model on `ScreenCaptureService`. Create
`app/src/main/java/org/fcast/android/sender/GstPopService.java`:

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

public final class GstPopService extends Service {
    private static final String TAG = "GstPopService";

    public static final String ACTION_START = "org.fcast.android.sender.GSTPOP_START";
    public static final String ACTION_STOP  = "org.fcast.android.sender.GSTPOP_STOP";
    public static final String EXTRA_CONFIG_JSON = "config_json";

    private static final int NOTIFICATION_ID = 2; // ScreenCaptureService uses 1.
    private static final String CHANNEL_ID = "org.fcast.android.sender.GstPopService";

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
            startForeground(NOTIFICATION_ID, buildNotification("Starting gst-pop…"));
            String config = intent.getStringExtra(EXTRA_CONFIG_JSON);
            String statusJson = GstPopServiceBridge.nativeStart(config != null ? config : "");
            Log.d(TAG, "nativeStart -> " + statusJson);
            // Refresh notification text with the actual state.
            updateNotification(statusJson);
            // START_STICKY: if the OS kills us under memory pressure, it
            // restarts us with a null Intent — onStartCommand will get an
            // ACTION_START shape on the *next* explicit start instead.
            return START_STICKY;
        }

        if (ACTION_STOP.equals(action)) {
            String statusJson = GstPopServiceBridge.nativeStop();
            Log.d(TAG, "nativeStop -> " + statusJson);
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
            return START_NOT_STICKY;
        }

        // Sticky-restart with null intent: don't auto-restart the daemon —
        // leave it to the UI to ask for ACTION_START again. This avoids a
        // surprise foreground service after the user explicitly stopped it.
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        // Defensive: if onDestroy fires without ACTION_STOP (e.g. task
        // removal, forced stop), make sure Rust knows.
        GstPopServiceBridge.nativeStop();
        super.onDestroy();
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        // No binder API for now. UI polls via GstPopServiceBridge.queryStatus().
        return null;
    }

    private void ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return;
        NotificationChannel channel = new NotificationChannel(
            CHANNEL_ID, "gst-pop backend", NotificationManager.IMPORTANCE_LOW);
        channel.setDescription("Embedded gst-pop daemon hosting");
        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm != null) nm.createNotificationChannel(channel);
    }

    private Notification buildNotification(String text) {
        Intent open = new Intent(this, MainActivity.class)
            .setFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP);
        PendingIntent openPi = PendingIntent.getActivity(this, 0, open,
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);
        Intent stop = new Intent(this, GstPopService.class).setAction(ACTION_STOP);
        PendingIntent stopPi = PendingIntent.getService(this, 0, stop,
            PendingIntent.FLAG_IMMUTABLE | PendingIntent.FLAG_UPDATE_CURRENT);

        Notification.Builder b = new Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("FCast gst-pop backend")
            .setContentText(text)
            .setContentIntent(openPi)
            .addAction(new Notification.Action.Builder(0, "Stop", stopPi).build())
            .setOngoing(true);
        return b.build();
    }

    private void updateNotification(String statusJson) {
        NotificationManager nm = getSystemService(NotificationManager.class);
        if (nm == null) return;
        String text = "gst-pop running on 127.0.0.1:9000"; // parse statusJson for real values
        nm.notify(NOTIFICATION_ID, buildNotification(text));
    }
}
```

### D.2 AndroidManifest changes

Edit `app/src/main/AndroidManifest.xml`:

```xml
<!-- Already present, keep as-is: -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_MEDIA_PROJECTION" />
<uses-permission android:name="android.permission.INTERNET" />
<uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />

<!-- New for the daemon-hosting service. Android 14+ requires picking a
     concrete foregroundServiceType; "dataSync" is the closest match
     for a long-running localhost server that is *not* user-initiated. -->
<uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />

<application … >

    <!-- existing -->
    <service
        android:name=".ScreenCaptureService"
        android:exported="false"
        android:stopWithTask="true"
        android:foregroundServiceType="mediaProjection" />

    <!-- NEW -->
    <service
        android:name=".GstPopService"
        android:exported="false"
        android:stopWithTask="false"
        android:foregroundServiceType="dataSync" />

    …
</application>
```

`android:stopWithTask="false"` is intentional — task removal should
**not** auto-kill the daemon. The user explicitly stopping the backend
(or switching away from gst-pop) is the only path that should tear it
down. See §H for the shutdown matrix.

---

## E. Rewire `BackendLifecycle::apply` to use the bridge

Today `BackendLifecycle::apply` (see
`src/backend/lifecycle.rs:88-99`)
just calls `install(build_backend(&config))` and probes. Insert the
service start/stop between persist and probe:

```rust
// src/backend/lifecycle.rs

async fn apply(&self, config: StoredBackendConfig, weak: Weak<MainWindow>) -> Result<()> {
    config.save(&self.files_dir)?;

    // Service lifecycle hooks. These are no-ops on non-Android targets.
    match config.kind {
        BackendKind::GstPop if super::gstpop::embedded::is_localhost(&config.gstpop_url) => {
            push_state(&weak, crate::MediaBackendState::Starting);
            request_service_start(&config)?;
        }
        BackendKind::GstPop => {
            // Remote gst-pop URL — service not needed, just a connectivity probe.
        }
        BackendKind::Migration => {
            // Switching away from gst-pop: tear down the service if we own it.
            request_service_stop();
        }
    }

    install(build_backend(&config));
    push_state(&weak, crate::MediaBackendState::Probing);
    match current().probe().await {
        Ok(status) => push_status(&weak, status),
        Err(err) => push_error(&weak, &err.to_string()),
    }
    Ok(())
}
```

The two helpers live in a new `src/backend/gstpop/service.rs`:

```rust
// src/backend/gstpop/service.rs

#[cfg(target_os = "android")]
pub fn request_service_start(config: &super::super::persistence::StoredBackendConfig) -> anyhow::Result<()> {
    use jni::objects::{JObject, JValue};
    let ctx = crate::android_context()?; // see §E.1
    let mut env = ctx.vm.attach_current_thread()?;

    let config_json = serde_json::to_string(config)?;
    let jconfig: JObject = env.new_string(&config_json)?.into();

    let bridge_class = env.find_class("org/fcast/android/sender/GstPopServiceBridge")?;
    env.call_static_method(
        bridge_class,
        "start",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[JValue::Object(&ctx.activity), JValue::Object(&jconfig)],
    )?;
    Ok(())
}

#[cfg(target_os = "android")]
pub fn request_service_stop() {
    if let Ok(ctx) = crate::android_context() {
        let _ = (|| -> anyhow::Result<()> {
            let mut env = ctx.vm.attach_current_thread()?;
            let class = env.find_class("org/fcast/android/sender/GstPopServiceBridge")?;
            env.call_static_method(
                class,
                "stop",
                "(Landroid/content/Context;)V",
                &[jni::objects::JValue::Object(&ctx.activity)],
            )?;
            Ok(())
        })();
    }
}

#[cfg(not(target_os = "android"))]
pub fn request_service_start(_config: &super::super::persistence::StoredBackendConfig) -> anyhow::Result<()> { Ok(()) }
#[cfg(not(target_os = "android"))]
pub fn request_service_stop() {}
```

### E.1 Reusable Android context

`lib.rs` already converts `vm_as_ptr` / `activity_as_ptr` on demand
(see `src/lib.rs:591-610`).
Wrap that into a single helper so the new code in §E doesn't
duplicate the dance:

```rust
// src/lib.rs

#[cfg(target_os = "android")]
pub(crate) struct AndroidCtx {
    pub vm: jni::JavaVM,
    pub activity: jni::objects::JObject<'static>,
}

#[cfg(target_os = "android")]
pub(crate) fn android_context() -> anyhow::Result<AndroidCtx> {
    let app = /* the stored PlatformApp clone, e.g. via OnceCell */;
    let vm_ptr = app.vm_as_ptr() as *mut jni::sys::JavaVM;
    let activity_ptr = app.activity_as_ptr() as *mut jni::sys::_jobject;
    let vm = unsafe { jni::JavaVM::from_raw(vm_ptr)? };
    let activity = unsafe { jni::objects::JObject::from_raw(activity_ptr) };
    Ok(AndroidCtx { vm, activity })
}
```

(You may already have a per-thread VM cache elsewhere — reuse it
rather than re-binding `from_raw` repeatedly.)

---

## F. Tighten `probe()` to a pure connectivity check

In `src/backend/gstpop/backend.rs`, remove the implicit start. The
service is now solely responsible for daemon lifetime:

```rust
// BEFORE — src/backend/gstpop/backend.rs (current behaviour)
async fn probe(&self) -> Result<BackendStatus> {
    if super::embedded::is_localhost(&self.url) {
        super::embedded::ensure_started(super::embedded::url_port(&self.url))
            .await
            .context("start embedded gst-pop")?;
    }
    let info = self.raw_call("get_version", json!({})).await…;
    …
}

// AFTER
async fn probe(&self) -> Result<BackendStatus> {
    // Probe is connectivity-only. Daemon lifetime is owned by
    // GstPopService. If the daemon is not yet up, surface a clean
    // error and let the UI ask for ACTION_START again.
    let info = self.raw_call("get_version", json!({})).await
        .context("probe: get_version (is the gst-pop service running?)")?;
    …
}
```

Side effect: the `ensure_started` shim from §B.1 now has no in-tree
callers and can be deleted (or marked `#[cfg(test)]` if tests still
use it). The smoke test
`src/backend/gstpop/backend.rs:158-164`
already accepts an externally-managed daemon (the dockerised one in
CI), so it stays green.

---

## G. Slint UI state additions

### G.1 Add a `Starting` state

Edit
`ui/bridge.slint:55-60`:

```slint
// ui/bridge.slint
export enum MediaBackendState {
    disconnected,
    starting,   // NEW — service-start requested, daemon not yet listening
    probing,    // service running, validating get_version / get_pipeline_count
    ready,
    error,
}
```

Then add the matching arm in
`ui/pages/media_backend_page.slint:56-77`:

```slint
// ui/pages/media_backend_page.slint — inside the status pill block
background:
    Bridge.media-backend-state == MediaBackendState.ready    ? Theme.success :
    Bridge.media-backend-state == MediaBackendState.probing  ? Theme.warning :
    Bridge.media-backend-state == MediaBackendState.starting ? Theme.warning :
    Bridge.media-backend-state == MediaBackendState.error    ? Theme.error-fg :
    Theme.text-disabled;
…
text:
    Bridge.media-backend-state == MediaBackendState.ready    ? @tr("Ready") :
    Bridge.media-backend-state == MediaBackendState.probing  ? @tr("Probing…") :
    Bridge.media-backend-state == MediaBackendState.starting ? @tr("Starting gst-pop service…") :
    Bridge.media-backend-state == MediaBackendState.error    ? @tr("Error") :
    @tr("Disconnected");
```

Slint-generated Rust enum: the new variant becomes
`crate::MediaBackendState::Starting`. Reference it in
`push_state(&weak, crate::MediaBackendState::Starting)` from §E.

### G.2 Status text propagation

`embedded_status()` returns `EmbeddedStatus`. Have the lifecycle code
write a friendly summary into `media-backend-status-text`:

```rust
// src/backend/lifecycle.rs — extend push_status for the new flow.
fn push_starting(weak: &Weak<MainWindow>, status: &EmbeddedStatus) {
    let text = match status.state {
        EmbeddedState::Stopped       => "gst-pop service stopped".into(),
        EmbeddedState::Starting      => "Starting gst-pop service…".into(),
        EmbeddedState::Running { externally_owned: true }  => "Using external gst-pop daemon".into(),
        EmbeddedState::Running { externally_owned: false } => format!("gst-pop running on {}:{}", status.bind, status.port),
        EmbeddedState::Error         => status.last_error.clone().unwrap_or_else(|| "gst-pop failed".into()),
    };
    let weak = weak.clone();
    let _ = weak.upgrade_in_event_loop(move |ui| {
        ui.global::<crate::Bridge>().set_media_backend_status_text(text.into());
    });
}
```

The "Apply" flow on the Slint side stays unchanged — it still fires
`Bridge.apply-media-backend()` and the Rust side does the rest. The
*only* user-visible difference is the extra status pill text.

### G.3 (Optional) Add a poller for status

If you skip the binder API (§D.1 returns `null`), add a 1Hz tokio
task that calls `GstPopServiceBridge.queryStatus()` while the panel is
visible and pushes the latest text into `media-backend-status-text`.
Keep it short-lived; deregister when the panel closes.

---

## H. Shutdown policy matrix

| Trigger | Action |
|---|---|
| User switches backend to `migration` in Apply | `request_service_stop()` from §E. |
| User switches backend to remote gst-pop URL | `request_service_stop()` (no localhost daemon needed). |
| User explicit Stop in notification | `GstPopService.ACTION_STOP` → `nativeStop` → `stopSelf`. |
| User swipes the task away | Service keeps running (`stopWithTask="false"`). |
| OS kills under memory pressure | `START_STICKY` restarts the service with a null Intent; we choose not to auto-restart the daemon (see comment in §D.1). |
| Process death + restart | On next `BackendLifecycle::new`, if persisted config is `gst-pop` + localhost, fire `request_service_start` from the `autostart` path. |
| Reboot | `START_STICKY` is *not* honoured across reboots. Same handling as fresh launch. |

The autostart hook is the only new behaviour worth pointing out:
`BackendLifecycle::autostart` currently only probes
(`src/backend/lifecycle.rs:80-87`).
Extend it:

```rust
fn autostart(self: Arc<Self>, weak: Weak<MainWindow>) {
    if self.initial_config.kind == BackendKind::GstPop
        && super::gstpop::embedded::is_localhost(&self.initial_config.gstpop_url)
    {
        let _ = super::gstpop::service::request_service_start(&self.initial_config);
    }
    push_state(&weak, crate::MediaBackendState::Probing);
    tokio::spawn(async move {
        match current().probe().await { … }
    });
}
```

---

## I. Race & recovery handling

Concrete cases to defend:

1. **Apply double-tap.** UI Apply button → two `request_service_start`
   calls in flight. `startForegroundService` is idempotent for an
   already-running service (delivers a second `ACTION_START`).
   `nativeStart` re-enters `start_embedded` which returns the existing
   `Running` status because of the READY fast path. Outcome: no
   duplicate server.
2. **Apply → Stop race.** UI Apply, then user hits Stop in notification
   before bind completes. Order can be:
   `ACTION_START → nativeStart (still starting) → ACTION_STOP → nativeStop`.
   `nativeStop` sees `STATE == Starting`, waits ~200ms for the bind to
   complete (or fail), then drops the handle. Implement that in
   `stop_embedded`:
   ```rust
   pub async fn stop_embedded() -> EmbeddedStatus {
       for _ in 0..10 {
           if !matches!(*STATE.read(), EmbeddedState::Starting) { break; }
           tokio::time::sleep(std::time::Duration::from_millis(50)).await;
       }
       // … existing teardown …
   }
   ```
3. **Stale `READY` after process restart.** Statics live in the cdylib
   for the lifetime of the *process*. If `MainActivity` is destroyed
   and recreated without process death, the statics survive — that is
   intentional and correct (the daemon is also alive). If the process
   is killed, the statics reset, but so does the daemon (it lives in
   the same process via `ServerHandle`). No drift possible.
4. **External listener on 9000.** Already handled by the §B.1
   `externally_owned: true` branch. `stop_embedded` is a no-op in this
   case so we never accidentally kill a user-managed daemon.
5. **Bind fails (port held by another, non-gst-pop, process).**
   `start_embedded` returns `EmbeddedState::Error` with `last_error =
   "failed to bind…"`. The notification text reflects this. The
   service stays foregrounded just long enough for `nativeStart` to
   return; you can choose to `stopSelf()` on `Error` to avoid showing
   a persistent failed-state notification.

---

## J. Test plan

Tier the verification — don't skip levels:

### J.1 Rust unit / integration

Add to `src/backend/gstpop/embedded.rs` `#[cfg(test)]`:

- `start_then_stop_is_idempotent`: call `start_embedded(0)` twice,
  assert `Running { externally_owned: false }` both times, assert a
  single port is bound.
- `stop_after_failed_start_resets_state`: monkey-patch a bind failure
  (or pre-bind 127.0.0.1:9000 in the test), assert
  `EmbeddedState::Error` then `Stopped` after `stop_embedded`.
- `externally_owned_stop_is_noop`: pre-bind a fake listener, call
  `start_embedded`, assert `externally_owned: true`, call
  `stop_embedded`, assert the fake listener still accepts.

Reuse the pattern from `round_trip_against_echo_server`
(`src/backend/gstpop/backend.rs:180-204`).

### J.2 Slint integration

Reuse the harness in
`src/backend/lifecycle.rs:200-220`
(`test_switch_media_backend_to_gstpop_integration`). Extend it to
assert:

- `media-backend-state == Starting` is observed between Apply and
  Ready.
- Switching back to `migration` calls `request_service_stop` (mock the
  trait — see §J.4).

### J.3 JNI / Java unit

Add `app/src/test/java/.../GstPopServiceBridgeTest.java` using
Robolectric. Verify:

- `start(context, "{}")` issues an Intent with action `ACTION_START`
  and the config extra.
- `stop(context)` issues an Intent with action `ACTION_STOP`.
- (No native calls in the JVM unit tier — those are exercised on-device.)

### J.4 Mocking the bridge in Rust tests

Wrap §E in a trait so tests inject a fake:

```rust
trait ServiceController: Send + Sync {
    fn start(&self, config: &StoredBackendConfig) -> anyhow::Result<()>;
    fn stop(&self);
}
```

Production wires `AndroidJniController`; tests wire `MockController`
with `Vec<…>` captures.

### J.5 Device / manual

In order:

1. Select gst-pop, Apply → status pill goes Starting → Ready,
   notification appears.
2. Rotate device → notification stays, state stays Ready.
3. Background app (home) → daemon still serving (verify with
   `adb shell curl 127.0.0.1:9000` or by reconnecting from a remote).
4. Finish activity (`adb shell am force-stop` won't help — use the
   back gesture from root). → service alive, notification alive.
5. Task swipe-away → with `stopWithTask="false"`, service alive.
6. Relaunch app from launcher → status pill reflects already-running
   daemon (`externally_owned: false` because the same process owns it).
7. Switch to Migration in the Backend page → notification disappears,
   daemon torn down.
8. Switch back to gst-pop → daemon restarts cleanly.
9. `adb shell am kill org.fcast.android.sender` to simulate process
   death → service is restarted by `START_STICKY` with null intent →
   under the policy in §D.1 it stops cleanly; verify the next Apply
   restarts it.

### J.6 Log assertions

Grep `adb logcat` for exactly one of:

- `Embedded gst-pop server running on 127.0.0.1:9000`
- `External gst-pop server already listening on 127.0.0.1:9000`

Per process lifetime. More than one of either means the lifecycle is
double-starting.

---

## K. Cleanup checklist (after the service path works)

- [ ] Remove `ensure_started`'s self-bind branch — it should only be a
      thin wrapper that calls `start_embedded`. Then delete the
      wrapper entirely from `src/backend/gstpop/embedded.rs` and from
      `src/backend/gstpop/backend.rs::probe`.
- [ ] Remove the static `CLAIMED` / `READY` atomics in
      `embedded.rs` once `STATE` is the sole source of truth (atomics
      stay if you want a lock-free fast path — fine either way).
- [ ] Drop `is_localhost` from `embedded.rs` `pub` surface if it
      becomes a private detail of `service.rs`.
- [ ] Delete the unused `embedded::ensure_started` test (none today,
      but if you add one in §J.1 keep it on the new API).
- [ ] Audit `migration_backend.rs` — no change expected, but confirm
      `MigrationBackend::shutdown` is wired into `apply` when
      switching away from gst-pop. The current
      `BackendLifecycle::apply` does **not** call `shutdown` on the
      outgoing backend. If you want symmetry with `request_service_stop`,
      add it here.

---

## L. Open decision points

1. **Foreground service type.** `dataSync` is the most defensible
   match for a localhost daemon. If the daemon will *only* ever serve
   media playback on behalf of a casting session you can argue for
   `mediaPlayback`, but the manifest would then need
   `FOREGROUND_SERVICE_MEDIA_PLAYBACK` and Android plays UI tricks
   (media controls in the notification shade) that the daemon does not
   support. Sticking with `dataSync` is simpler.
2. **Binder vs poll.** §D.1 returns `null` from `onBind`. If you later
   want push-based UI updates, expose a Messenger or a small AIDL
   interface that the UI registers a callback against. Not needed for
   the initial milestone.
3. **API key in config JSON.** `StoredBackendConfig` already round-trips
   `gstpop_api_key`. Passing it through `EXTRA_CONFIG_JSON` is fine
   *if* you accept that `dumpsys activity services` will surface the
   key. If that matters, use a private file in `getFilesDir()` instead
   of an Intent extra.
4. **Restart-on-failure.** `START_STICKY` is the recommended setting,
   but you can swap to `START_REDELIVER_INTENT` if you want the OS to
   redeliver the last `ACTION_START` Intent after a kill. Be wary:
   that re-runs `nativeStart` *without* the user re-requesting it.

---

## M. File-by-file change list (no code committed by this guide)

| File | Change |
|---|---|
| `src/backend/gstpop/embedded.rs` | Add `EmbeddedState`, `EmbeddedStatus`, `start_embedded`, `stop_embedded`, `embedded_status`. Delete `ensure_started` after callers migrate. |
| `src/backend/gstpop/service.rs` (new) | `request_service_start`, `request_service_stop` JNI shims. |
| `src/backend/gstpop/mod.rs` | `pub mod service;` |
| `src/backend/gstpop/backend.rs` | Drop the implicit `ensure_started` call in `probe`. |
| `src/backend/lifecycle.rs` | Service hooks in `apply` + `autostart`. |
| `src/lib.rs` | Add 3 `Java_org_fcast_android_sender_GstPopServiceBridge_native…` exports; add `android_context()` helper. |
| `ui/bridge.slint` | Add `MediaBackendState::Starting`. |
| `ui/pages/media_backend_page.slint` | Render the Starting state in the status pill. |
| `app/src/main/java/org/fcast/android/sender/GstPopServiceBridge.java` (new) | Static helper + native declarations. |
| `app/src/main/java/org/fcast/android/sender/GstPopService.java` (new) | Foreground service. |
| `app/src/main/AndroidManifest.xml` | Add `FOREGROUND_SERVICE_DATA_SYNC` permission + `<service android:name=".GstPopService" …>`. |
| `app/build.gradle` | No change expected (no new deps). |

Total: 3 new files (`service.rs`, `GstPopServiceBridge.java`,
`GstPopService.java`), 8 edited files, 0 generated files.

---

## N. References (current state)

- Implicit start today:
  `src/backend/gstpop/backend.rs:62-66`
- Embedded server bookkeeping:
  `src/backend/gstpop/embedded.rs:11-62`
- BackendLifecycle apply / probe wiring:
  `src/backend/lifecycle.rs:31-99`
- JNI export template:
  `src/lib.rs:2453-2485`
- Existing foreground service template:
  `app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java`
- Slint state surface:
  `ui/bridge.slint:55-60`,
  `ui/bridge.slint:289-296`
- Smoke-test that already tolerates externally-managed daemons:
  `src/backend/gstpop/backend.rs:156-164`
