# Migration Runtime Service — Architecture

How the migration runtime starts, runs, and stops on Android.

---

## Layer map

```
┌──────────────────────────────────────────────────────────────────┐
│  Slint UI  (ui/pages/media_backend_page.slint)                  │
│  "Start service" button  →  Bridge.start-migration-runtime-service() │
└───────────────────────────────┬──────────────────────────────────┘
                                │ callback (Slint → Rust)
┌───────────────────────────────▼──────────────────────────────────┐
│  Rust lifecycle  (src/backend/lifecycle.rs)                      │
│  on_start_migration_runtime_service  →                           │
│      migration::service::request_service_start()                 │
└───────────────────────────────┬──────────────────────────────────┘
                                │ JNI reflection (Rust → Java)
┌───────────────────────────────▼──────────────────────────────────┐
│  Java bridge  (MigrationRuntimeServiceBridge.java)               │
│  start(context, "{}") → context.startForegroundService(intent)   │
└───────────────────────────────┬──────────────────────────────────┘
                                │ Android Intent (ACTION_START)
┌───────────────────────────────▼──────────────────────────────────┐
│  Android foreground service  (MigrationRuntimeService.java)      │
│  onStartCommand → startForeground() → nativeStart()              │
└───────────────────────────────┬──────────────────────────────────┘
                                │ JNI export (Java → Rust)
┌───────────────────────────────▼──────────────────────────────────┐
│  Rust JNI export  (src/lib.rs)                                   │
│  nativeStartMigrationRuntimeHost → runtime::start_graph_runtime()│
└───────────────────────────────┬──────────────────────────────────┘
                                │
┌───────────────────────────────▼──────────────────────────────────┐
│  Rust runtime  (src/migration/runtime.rs)                        │
│  Spawns refresh thread + optional HTTP command server            │
└──────────────────────────────────────────────────────────────────┘
```

---

## Step-by-step: Start

### 1 — User taps "Start service" in the UI

**File:** `ui/pages/media_backend_page.slint:233`

```slint
PrimaryButton {
    label: @tr("Start service");
    enabled: Bridge.migration-runtime-service-state == "stopped"
          || Bridge.migration-runtime-service-state == "error";
    clicked => { Bridge.start-migration-runtime-service(); }
}
```

The button fires a Slint callback into Rust.

---

### 2 — Rust lifecycle sets optimistic state and calls the service helper

**File:** `src/backend/lifecycle.rs:102–116`

```rust
bridge.on_start_migration_runtime_service(move || {
    // Optimistically mark "starting" so the button disables immediately.
    set_migration_runtime_service_state("starting");

    tokio::spawn(async move {
        if let Err(err) = crate::migration::service::request_service_start() {
            // Roll back to "error" if the OS rejected startForegroundService.
            set_migration_runtime_service_state("error");
        }
        // On success the 1 Hz poller will flip the state to "running".
    });
});
```

---

### 3 — Rust service helper: JNI reflection into Java

**File:** `src/migration/service.rs:48–71`

The helper attaches to the JVM, loads `MigrationRuntimeServiceBridge` via the
**activity's ClassLoader** (not the bootstrap loader — app classes aren't visible
there on non-JVM-spawned threads), and calls `start(Context, String)`.

```rust
pub fn request_service_start() -> anyhow::Result<()> {
    let ctx = crate::android_context()?;
    let mut env = ctx.vm.attach_current_thread()?;
    let bridge = android::load_app_class(&mut env, &ctx.activity,
        "org.fcast.android.sender.MigrationRuntimeServiceBridge")?;
    env.call_static_method(bridge, "start",
        "(Landroid/content/Context;Ljava/lang/String;)V",
        &[JValue::Object(&ctx.activity), JValue::Object(&jconfig)])?;
    Ok(())
}
```

---

### 4 — Java bridge: startForegroundService

**File:** `app/src/main/java/org/fcast/android/sender/MigrationRuntimeServiceBridge.java:27–37`

```java
public static void start(Context context, String configJson) {
    Intent intent = new Intent(context, MigrationRuntimeService.class)
        .setAction(MigrationRuntimeService.ACTION_START)
        .putExtra(MigrationRuntimeService.EXTRA_CONFIG_JSON,
                  configJson == null ? "{}" : configJson);
    context.startForegroundService(intent);   // → Android OS queue
}
```

`startForegroundService` queues the intent with the OS. Android gives the service
**5 seconds** to call `startForeground()` or ANR.

---

### 5 — Android delivers ACTION_START to the service

**File:** `app/src/main/java/org/fcast/android/sender/MigrationRuntimeService.java:36–57`

```java
public int onStartCommand(Intent intent, int flags, int startId) {
    if (ACTION_START.equals(action)) {
        // Must call startForeground before any blocking work (Android 14).
        startForeground(NOTIFICATION_ID,
            buildNotification("Starting migration runtime…"));

        String statusJson = MigrationRuntimeServiceBridge.nativeStart(config);
        updateNotification(statusJson);          // update tray text

        if (isErrorState(statusJson)) {
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
            return START_NOT_STICKY;
        }
        return START_STICKY;                     // OS will restart if killed
    }
    ...
}
```

`nativeStart` calls back into Rust via JNI.

---

### 6 — JNI export: hand off to the Rust runtime

**File:** `src/lib.rs:3054–3068`

```rust
pub extern "C"
fn Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_nativeStartMigrationRuntimeHost(
    mut env: JNIEnv,
    _class: JClass,
    _config_json: JString,
) -> jstring {
    let json = match crate::migration::runtime::start_graph_runtime() {
        Ok(()) => migration_runtime_status_json("running", None),
        Err(err) => migration_runtime_status_json("error", Some(&err.to_string())),
    };
    env.new_string(json).expect("new_string").into_raw()
}
```

Returns `{"state":"running"}` or `{"state":"error","last_error":"…"}` as a JSON
string which flows back up to `MigrationRuntimeService.onStartCommand`.

---

### 7 — Rust runtime: spawn worker threads

**File:** `src/migration/runtime.rs:302–310`

```rust
pub fn start_graph_runtime() -> Result<()> {
    {
        let mut manager = GRAPH_NODE_MANAGER.lock();
        manager.start();                       // initialise the node graph
    }
    ensure_refresh_thread_running()?;          // 100 ms tick loop
    ensure_command_server_running()?;          // optional HTTP command endpoint
    Ok(())
}
```

`start_graph_runtime` is **idempotent** — calling it twice is safe.

Two background threads start:

| Thread | Purpose | Interval |
|---|---|---|
| `graph-runtime-refresh` | Calls `NodeManager::tick()` | 100 ms |
| `graph-command-endpoint` | HTTP server for JSON commands | on-demand (env: `MIGRATION_COMMAND_BIND`) |

---

## Status reporting

After start, a **1 Hz poller** in the Rust lifecycle layer drives all UI state.

**File:** `src/backend/lifecycle.rs:155–195`

```
Every 1 second:
  migration::service::query_status()
    → JNI → MigrationRuntimeServiceBridge.queryStatus()
    → JNI → nativeGetMigrationRuntimeStatus()
    → runtime::is_running()   (reads GRAPH_REFRESH_RUNNING atomic)
    → {"state":"running"} or {"state":"stopped"}

  If migration backend is active:
    set_migration_runtime_service_state("running" | "stopped" | …)
    set_media_backend_state(Ready | Disconnected | …)
    set_media_backend_status_text("Migration runtime running" | …)
```

The Slint UI re-renders reactively from these properties.

---

## Step-by-step: Stop

| Step | Location | What happens |
|---|---|---|
| 1 | `media_backend_page.slint` | User taps "Stop service" |
| 2 | `lifecycle.rs:120–127` | `migration::service::request_service_stop()` called; state set to `"stopping"` |
| 3 | `service.rs:77–100` | JNI: calls `MigrationRuntimeServiceBridge.stop(context)` |
| 4 | `MigrationRuntimeServiceBridge.java:40–48` | `context.startService(ACTION_STOP intent)` |
| 5 | `MigrationRuntimeService.java:59–65` | `nativeStop()` → `stopForeground()` → `stopSelf()` |
| 6 | `lib.rs:3073–3083` | `runtime::shutdown_graph_runtime()` |
| 7 | `runtime.rs:312–320` | Sets `GRAPH_REFRESH_RUNNING=false`, joins threads, calls `manager.shutdown()` |
| 8 | 1 Hz poller | Detects `is_running()==false`, flips UI to Disconnected / "stopped" |

`onDestroy` also calls `nativeStop()` as a defensive teardown if the service is
killed by the OS under memory pressure without receiving `ACTION_STOP`.

---

## Key files

| File | Role |
|---|---|
| `ui/pages/media_backend_page.slint` | Start/Stop buttons, service status dot |
| `ui/components/status_pill.slint` | Top-level status pill (driven by `media-backend-state`) |
| `src/backend/lifecycle.rs` | Slint callbacks, 1 Hz poller |
| `src/migration/service.rs` | Rust → Java JNI reflection helpers |
| `src/lib.rs` | JNI exports (`nativeStart`, `nativeStop`, `nativeGetMigrationRuntimeStatus`) |
| `app/.../MigrationRuntimeServiceBridge.java` | Java → OS + Java → JNI thin wrapper |
| `app/.../MigrationRuntimeService.java` | Android foreground service |
| `src/migration/runtime.rs` | Worker threads, `start_graph_runtime`, `shutdown_graph_runtime` |
| `src/migration/node_manager.rs` | Graph node lifecycle, `tick()` |

---

## Design decisions

**Why a foreground service?**  
Android throttles and kills background work aggressively. A foreground service
with a persistent notification gives the OS a signal to keep the process at a
higher priority and prevents the runtime from being torn down mid-cast.

**Why JNI reflection instead of a direct call?**  
`startForegroundService` requires a `Context` object. Rust does not hold a
`Context` directly; the activity reference stored in `ANDROID_CTX` is used to
call Java's `startForegroundService` via JNI.

**Why is `start_graph_runtime` idempotent?**  
`ensure_refresh_thread_running` checks `thread_slot.is_some()` before spawning.
This means the JNI export can be called by the service AND by other callers
(e.g. screen-capture flow in `Event::CaptureStarted`) without double-spawning threads.

**Why `START_STICKY`?**  
If the process is killed under memory pressure, Android will restart it with a
`null` intent. `onStartCommand` handles `null` by calling `stopSelf()` — the
runtime does not restart automatically; the user or the cast flow must trigger it
explicitly.
