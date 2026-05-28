# gst-pop Android service — runtime architecture

How the gst-pop foreground service actually works in this codebase, end
to end. Read top-to-bottom on a first pass; jump to the section you're
debugging on subsequent reads.

For the *design rationale* behind these choices, see
[`gstpop-android-service-guide/`](./gstpop-android-service-guide/). For
the *crate extraction* that moved the daemon runtime out of
`src/backend/gstpop/` into `crates/gstpop-runtime`, see
[`gstpop-runtime-crate-extraction/`](./gstpop-runtime-crate-extraction/).
This document describes what's **in code today** (post-extraction).

---

## 1. Layers at a glance

```
┌─────────────────────────────────────────────────────────────┐
│ Slint UI    (ui/bridge.slint, ui/pages/media_backend_page)  │
│   Bridge.start-gstpop-service() / stop-gstpop-service()     │
│   Bridge.gstpop-service-state, gstpop-service-externally-…  │
└──────────────────────────┬──────────────────────────────────┘
                           │ callbacks + properties
┌──────────────────────────▼──────────────────────────────────┐
│ BackendLifecycle    (src/backend/lifecycle.rs)              │
│   apply / autostart / 1Hz poller / Start-Stop callbacks     │
└──────────────────────────┬──────────────────────────────────┘
                           │ Rust → Java JNI call
┌──────────────────────────▼──────────────────────────────────┐
│ gstpop_service    (src/gstpop_service.rs)                   │
│   request_service_start / request_service_stop              │
│   uses android_context() + activity ClassLoader             │
└──────────────────────────┬──────────────────────────────────┘
                           │ Java static method call
┌──────────────────────────▼──────────────────────────────────┐
│ GstPopServiceBridge.java                                    │
│   start(ctx, json) → startForegroundService(ACTION_START)   │
│   stop(ctx)        → startService(ACTION_STOP)              │
└──────────────────────────┬──────────────────────────────────┘
                           │ Intent
┌──────────────────────────▼──────────────────────────────────┐
│ GstPopService.java       (foreground, dataSync)             │
│   onStartCommand → startForeground + nativeStart/Stop       │
└──────────────────────────┬──────────────────────────────────┘
                           │ JNI back into Rust
┌──────────────────────────▼──────────────────────────────────┐
│ JNI exports          (src/lib.rs:3000-3045)                 │
│   nativeStartGstPopServiceHost / Stop / GetStatus           │
│   run on HOST_RUNTIME (separate from UI tokio runtime)      │
└──────────────────────────┬──────────────────────────────────┘
                           │ async fn call
┌──────────────────────────▼──────────────────────────────────┐
│ gstpop-runtime crate    (crates/gstpop-runtime/src/)        │
│   embedded.rs : start_embedded / stop_embedded / status     │
│   client.rs   : WebSocket JSON-RPC client                   │
│   protocol.rs : frame classifier + Request/Response/Event   │
│   owns ServerHandle for the in-process gst-pop daemon       │
└──────────────────────────┬──────────────────────────────────┘
                           │ binds 127.0.0.1:9000
┌──────────────────────────▼──────────────────────────────────┐
│ vendor/gstpop ServerHandle                                  │
│   WebSocket + JSON-RPC pipeline manager                     │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. Files involved

### Rust — `gstpop-runtime` crate (`crates/gstpop-runtime/`)

| Path | Role |
|---|---|
| `crates/gstpop-runtime/Cargo.toml` | Crate manifest; depends on `gstpop` (vendored), `tokio`, `tokio-tungstenite`, `serde`, etc. |
| `crates/gstpop-runtime/src/lib.rs` | Module declarations + public re-exports (`embedded_status`, `start_embedded`, `EmbeddedState`, `GstPopClient`, …). |
| `crates/gstpop-runtime/src/embedded.rs` | Statics + `start_embedded` / `stop_embedded` / `embedded_status`. Owns `ServerHandle`. |
| `crates/gstpop-runtime/src/client.rs` | `GstPopClient` — WS JSON-RPC client. |
| `crates/gstpop-runtime/src/protocol.rs` | Frame classifier + `Request` / `Response` / `Event` types. |
| `crates/gstpop-runtime/src/protocol_tests.rs` | Unit tests (gated `#[cfg(test)]`). |

### Rust — app crate (`src/`)

| Path | Role |
|---|---|
| `src/backend/gstpop_backend.rs` | `GstPopBackend` implements `MediaBackend`; `probe()` is connectivity-only. Imports `GstPopClient` from `gstpop_runtime`. |
| `src/gstpop_service.rs` | Rust → Java bridge (`request_service_start` / `_stop`). Pairs with `src/migration_service.rs`. |
| `src/backend/lifecycle.rs` | Apply / autostart, Start-Stop callbacks, 1Hz status poller. Imports `gstpop_runtime::*` directly. |
| `src/lib.rs` (≈75-113) | `HOST_RUNTIME`, `AndroidCtx`, `android_context()`. |
| `src/lib.rs` (≈3000-3052) | Three JNI exports for `GstPopServiceBridge` (call `gstpop_runtime::*`). |

### Android (`app/src/main/`)

| Path | Role |
|---|---|
| `app/src/main/java/.../GstPopServiceBridge.java` | Sole Java entrypoint for daemon control. |
| `app/src/main/java/.../GstPopService.java` | Foreground service hosting the daemon. |
| `app/src/main/AndroidManifest.xml` | `FOREGROUND_SERVICE_DATA_SYNC`, `<service android:foregroundServiceType="dataSync" stopWithTask="false">`. |

### Slint (`ui/`)

| Path | Role |
|---|---|
| `ui/bridge.slint` | `MediaBackendState::starting`, `start/stop-gstpop-service` callbacks, `gstpop-service-state` / `…externally-owned` properties. |
| `ui/pages/media_backend_page.slint` | Renders the status pill + "Service" section. |

---

## 3. Process boundaries

- **Same process.** `GstPopService` has no `android:process` attribute,
  so the service runs in the same Linux process as `MainActivity`. This
  lets `ServerHandle` survive activity destruction without IPC.
- **Two tokio runtimes.**
  - The Slint UI runtime, built in `main_inner`.
  - `HOST_RUNTIME` (`lib.rs:87-93`) — a dedicated 2-thread runtime used
    only by the JNI exports so binder-thread calls never block the UI.
- **One native library.** `MainActivity.java` calls
  `System.loadLibrary("fcastsender")`. Because the service is in the
  same process, the JNI symbols are already resolved when
  `nativeStart` is first invoked.

---

## 4. Startup paths

### 4.1 Cold start (app launch with persisted gst-pop config)

```
1. Android starts MainActivity → cdylib main_inner() runs.
2. ANDROID_APP is installed; HOST_RUNTIME is built lazily.
3. BackendLifecycle::new(files_dir)
     loads StoredBackendConfig from <files_dir>/backend.json
     install(build_backend(&initial_config))            // GstPopBackend
4. lifecycle.register(ui)
     - wires Bridge callbacks (apply / save / probe / start / stop)
     - spawns 1Hz gst-pop status poller
     - spawns 1Hz migration runtime poller
     - calls autostart(weak)
5. autostart:
     if initial_config.kind == GstPop && is_localhost(url):
       service::request_service_start(&initial_config)
       push_state(Starting)
     else:
       push_state(Probing)
6. Probe retry loop (25 × 200ms):
     current().probe().await
       → raw_call("get_version") → raw_call("get_pipeline_count")
     on first success: push_status(Ready), exit loop.
```

`request_service_start` (`src/gstpop_service.rs:40-64`) does:

1. `android_context()` returns cached VM + activity (`lib.rs:103-113`).
2. `vm.attach_current_thread()` attaches the current OS thread to the
   JVM (this runs from a tokio worker).
3. `load_app_class(env, activity, "org.fcast.android.sender.GstPopServiceBridge")`
   uses the **activity's ClassLoader** to find the class. Plain
   `env.find_class()` would fail on non-JVM-spawned threads because
   the bootstrap ClassLoader can't see app classes.
4. `env.call_static_method(bridge, "start", "(Context;String;)V", …)`
   passes the activity as `Context` and the serialized
   `StoredBackendConfig` as JSON.

`GstPopServiceBridge.start` (`GstPopServiceBridge.java:24-34`) builds
an Intent with `ACTION_START` + `EXTRA_CONFIG_JSON` and calls
`context.startForegroundService(intent)`.

### 4.2 Service onStartCommand (ACTION_START)

`GstPopService.java:36-72`:

```
1. startForeground(NOTIFICATION_ID, "Starting gst-pop…")
   Android 14 requires this within 5s of startForegroundService.
2. config = intent.getStringExtra(EXTRA_CONFIG_JSON)
3. statusJson = GstPopServiceBridge.nativeStart(config)
4. updateNotification(statusJson):
     - externally_owned → "Using external gst-pop", self-stop in 500ms
     - error            → drop foreground, stopSelf, START_NOT_STICKY
     - otherwise        → show "gst-pop running on 127.0.0.1:9000"
5. return START_STICKY
```

### 4.3 JNI → Rust daemon start

`src/lib.rs:3002-3016`:

```rust
Java_…_nativeStartGstPopServiceHost(env, _class, config_json):
  let config = jstring_to_string(config_json)
  let port   = parse_gstpop_config_port(&config).unwrap_or(9000)
  let status = HOST_RUNTIME.block_on(
                 gstpop_runtime::start_embedded(port)   // async
               )
  return env.new_string(json(status))
```

`block_on` is safe here because the calling thread is a JVM binder
thread, not a tokio worker.

### 4.4 `start_embedded` state machine

`embedded.rs:74-135` — five distinct paths:

| Entry condition | Path | Final state |
|---|---|---|
| `READY` && `STATE.port == port` | Fast path — return snapshot | unchanged |
| `probe_port_open(port)` returns true | Adopt external listener | `Running { externally_owned: true }` |
| `CLAIMED.compare_exchange` succeeds | Race winner — bind | `Starting` → `Running` or `Error` |
| `CLAIMED.compare_exchange` fails | Race loser — wait for port | snapshot after `wait_for_port` |
| `start_server` returns Err | Bind failed | `Error` with `last_error` populated |

`start_server` (`crates/gstpop-runtime/src/embedded.rs:172-188`)
builds a `gstpop::server::ServerConfig { bind: "127.0.0.1", port,
no_dbus: true, no_websocket: false, api_key: None, allowed_origins:
[] }` and calls `ServerHandle::start(…)`. `wait_for_port` (≤2s)
confirms the listener is accepting before `start_server` returns
`Ok`.

---

## 5. Status surfacing

### 5.1 1Hz Rust poller → Slint

`src/backend/lifecycle.rs:130-153`:

```rust
loop {
    ticker.tick().await;
    let status = gstpop_runtime::embedded_status();  // RwLock::read + clone
    let state_str = match status.state {
        gstpop_runtime::EmbeddedState::Stopped  => "stopped",
        gstpop_runtime::EmbeddedState::Starting => "starting",
        gstpop_runtime::EmbeddedState::Running  => "running",
        gstpop_runtime::EmbeddedState::Error    => "error",
    };
    let externally = status.externally_owned;
    poll_weak.upgrade_in_event_loop(move |ui| {
        if PanelBridge.get_active() != Panel::MediaBackend { return; }
        Bridge.set_gstpop_service_state(state_str);
        Bridge.set_gstpop_service_externally_owned(externally);
    });
}
```

The off-panel guard runs **inside** the event-loop closure so the
read-the-status side is unconditional, but the Slint property writes
are skipped when the user is on another page.

### 5.2 Slint rendering

`ui/bridge.slint:349-353` exposes:

```slint
callback start-gstpop-service();
callback stop-gstpop-service();
in-out property <string> gstpop-service-state: "stopped";
in-out property <bool>   gstpop-service-externally-owned: false;
```

`ui/pages/media_backend_page.slint` renders the indicator dot + label
from `gstpop-service-state`, plus the externally-owned hint when set.

### 5.3 Notification text

`GstPopService.describe()` (java:138-156) parses the JSON snapshot and
produces strings like:

- `gst-pop running on 127.0.0.1:9000`
- `Starting gst-pop on 127.0.0.1:9000…`
- `gst-pop error: failed to bind embedded gst-pop on 127.0.0.1:9000`
- `Using external gst-pop`

---

## 6. Apply (backend switch / settings change)

`src/backend/lifecycle.rs:233-272`:

```rust
async fn apply(...) {
    use crate::gstpop_service as service;
    use gstpop_runtime as embedded;

    let previous = current();
    config.save(files_dir)?;
    match config.kind {
        GstPop if embedded::is_localhost(&url) =>
            push_state(Starting); service::request_service_start(&config)
        GstPop                                  => service::request_service_stop()
        Migration                               => service::request_service_stop()
    }
    if previous.kind() != config.kind {
        previous.shutdown().await   // GstPopBackend drops WS client
    }
    install(build_backend(&config));
    push_state(Probing);
    match current().probe().await {
        Ok(s)  => push_status(weak, s)   // Ready
        Err(e) => push_error(weak, e)    // Error
    }
}
```

Notes:
- `previous.shutdown` only runs on **kind change**. Same-kind config
  changes (e.g. different gst-pop URL) leave the cached WebSocket
  client live — the next `raw_call` reconnects on demand.
- `push_status` decides Ready vs Disconnected from
  `BackendStatus::is_connected` (`lifecycle.rs:334-338`).

---

## 7. Stop / shutdown paths

| Trigger | What runs |
|---|---|
| UI **Stop** button | `Bridge::on_stop_gstpop_service` → `service::request_service_stop` → `GstPopServiceBridge.stop(ctx)` → `Intent(ACTION_STOP)` → service `onStartCommand` |
| Notification **Stop** action | PendingIntent → `Intent(ACTION_STOP)` → service `onStartCommand` |
| Backend switch to Migration | `apply()` calls `crate::gstpop_service::request_service_stop()` directly |
| OS kill under pressure | `START_STICKY` re-creates service with `intent=null` → null-intent branch stops cleanly (no auto-restart of daemon) |
| Task swipe-away | `stopWithTask="false"` ignores it → daemon keeps running |
| `onDestroy` without ACTION_STOP | Defensive `GstPopServiceBridge.nativeStop()` |

`stop_embedded` (`crates/gstpop-runtime/src/embedded.rs:138-165`):

```
1. Up to 500ms grace window if STATE == Starting
2. if STATE.externally_owned: log + return snapshot (no-op)
3. HANDLE.lock().take() → drop(ServerHandle)   // releases bind
4. READY = false, CLAIMED = false, STATE = Stopped
```

The 500ms grace window prevents the
"`Apply` then immediate notification `Stop`" race from leaving a
half-bound listener behind.

---

## 8. Race / recovery cases

### 8.1 Double-tap Apply (same config)

- `startForegroundService` is idempotent for an already-running service
  (delivers another `onStartCommand`).
- `nativeStart` re-enters `start_embedded`; `READY` is set and
  `STATE.port` matches → fast-path returns the existing snapshot.
- No second bind, no leaked handle.

### 8.2 External daemon on port 9000

- `probe_port_open(9000)` returns true before the bind attempt.
- `start_embedded` sets `externally_owned = true` and returns
  `Running`.
- `stop_embedded` is a no-op for this flag.
- `GstPopService.updateNotification` shows "Using external gst-pop"
  and schedules `stopSelf` in 500ms — we don't want a foreground
  notification claiming to host something we don't actually own.

### 8.3 Bind fails (port held)

- `start_server` returns `Err`.
- `STATE = Error`, `last_error = "failed to bind embedded gst-pop on …"`.
- `GstPopService.isErrorState(statusJson)` returns true →
  `stopForeground(STOP_FOREGROUND_REMOVE)` + `stopSelf`. No persistent
  error notification.
- Status poller picks the error state up on the next tick; UI shows
  the error pill.

### 8.4 Process death

- Process dies → `ServerHandle` is gone, OS releases the bind.
- `START_STICKY` re-creates `GstPopService` with `intent=null`.
- Null-intent branch calls `stopSelf`. No surprise foreground.
- On next app launch, `autostart` re-issues `request_service_start`
  if the persisted config still wants gst-pop.

### 8.5 Activity recreate (process still alive)

- Statics in `crates/gstpop-runtime/src/embedded.rs` live as long as the process. `READY` is
  still true, `HANDLE` still holds the bind.
- New activity instance runs `autostart` again; the probe loop hits
  the existing listener on the first iteration → `Ready` with no
  visible "Starting" flash.

---

## 9. Manifest contract

`AndroidManifest.xml:27-31`:

```xml
<service
    android:name=".GstPopService"
    android:exported="false"
    android:stopWithTask="false"
    android:foregroundServiceType="dataSync" />
```

Plus the matching `<uses-permission
android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />`
on line 7 and `POST_NOTIFICATIONS` on line 10 (required on Android
13+ for the FGS notification).

- `exported="false"` — only this app can start the service.
- `stopWithTask="false"` — daemon survives task swipe.
- `foregroundServiceType="dataSync"` — most defensible match for a
  long-running localhost server (see guide §12.1).

---

## 10. Key invariants

- **Single daemon per process.** The atomics + `Option<ServerHandle>`
  in the `gstpop-runtime` crate enforce this. Calling
  `start_embedded(p2)` while running on `p1` currently leaks `p1`'s
  handle — there is *no* in-tree port-switch fix today (the guide
  proposes one in §8.4 but it's not implemented; with the runtime in
  its own crate the fix is now contained to `embedded.rs`).
- **Probe never starts.** `GstPopBackend::probe` is connectivity-only.
  The daemon is started exclusively through the service path.
- **Foreground before blocking work.** `startForeground` runs before
  `nativeStart` in `onStartCommand` so the Android-14 5s grace window
  is always satisfied.
- **Null-intent self-stops.** `START_STICKY` revival doesn't
  auto-start the daemon; only the explicit autostart path does.
- **HOST_RUNTIME != UI runtime.** JNI calls never deadlock the UI.

---

## 11. Related: MigrationRuntimeService

A parallel pair exists for the migration runtime:

- `app/src/main/java/.../MigrationRuntimeService.java`
- `app/src/main/java/.../MigrationRuntimeServiceBridge.java`
- `Java_…_MigrationRuntimeServiceBridge_*` JNI exports in
  `src/lib.rs:3061+`
- `crate::migration_service::{request_service_start, request_service_stop,
  query_status}`
- Separate 1Hz poller in `lifecycle.rs:156-194` that also drives the
  top-level Media Backend pill when `media-backend == Migration`.

The pattern is the same; the migration runtime has no port to bind, so
its status is `running`/`error`/`stopped` only.

---

## 12. Debugging cheatsheet

```bash
# Symbols are exported?
nm --defined-only app/src/main/jniLibs/arm64-v8a/libfcastsender.so | \
  grep GstPopServiceBridge
# Expect exactly 3 (Start, Stop, GetStatus).

# Daemon up?
adb shell "curl -s http://127.0.0.1:9000 -o /dev/null && echo UP"

# Force-hold the port to test bind-failure path
adb shell "nc -l -p 9000 &"  # then Apply gst-pop → expect Error pill
adb shell pkill nc           # then Apply again → expect Ready

# Service alive?
adb shell dumpsys activity services org.fcast.android.sender | \
  grep -A2 GstPopService

# State transitions in logcat
adb logcat -s GstPopService:D GstPopServiceBridge:D | \
  grep -E "onStartCommand|nativeStart|nativeStop|gst-pop"
```

You should see exactly one `Embedded gst-pop running on …` (or
`External gst-pop already on …; adopting`) per process lifetime, and
one `stop_embedded: dropping handle` per explicit teardown. More than
one of either means the lifecycle is double-starting.
