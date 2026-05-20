# 9 · Race & recovery handling

Concrete cases the new design must defend. Each one has a defensive
code snippet you can drop into the relevant file from earlier steps.

## 9.1 Apply double-tap

User taps Apply twice in quick succession.

```
T0  UI Apply               → request_service_start(config)
T1  startForegroundService → ACTION_START
T2  UI Apply (again)       → request_service_start(config)
T3  startForegroundService → ACTION_START
T4  Service.onStartCommand × 2
```

- `startForegroundService` is idempotent: a second call while the
  service is already running just delivers a second `onStartCommand`.
- `nativeStart` re-enters `start_embedded`, hits the
  `READY.load + STATE.port == port` fast path, returns the existing
  `Running { externally_owned: false }` status.
- No duplicate server, no leaked handle. **No code change needed.**

## 9.2 Apply → Stop race

Covered in detail in `08-shutdown-policy.md` §8.3. The defensive
snippet for `stop_embedded`:

```rust
pub async fn stop_embedded() -> EmbeddedStatus {
    // Wait briefly for a Starting transition to finish so we don't drop
    // a half-bound ServerHandle.
    for _ in 0..10 {
        if !matches!(STATE.read().state, EmbeddedState::Starting) { break; }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    // …
}
```

## 9.3 Stale `READY` after activity recreate

The activity is destroyed and recreated (e.g., theme change, rotation
with `configChanges` removed). Statics in the cdylib live for the
**process** lifetime — so `READY` is still true and the daemon is
still listening. The new `MainActivity` instance's
`BackendLifecycle::new` runs again, but `autostart` should observe
the existing service via the status poller:

```rust
fn autostart(self: Arc<Self>, weak: Weak<MainWindow>) {
    use super::gstpop::embedded;

    // If the daemon is already running in-process, skip the
    // "Starting" pill — go straight to probe.
    let status = embedded::embedded_status();
    if matches!(status.state, EmbeddedState::Running { .. }) {
        push_state(&weak, crate::MediaBackendState::Probing);
        tokio::spawn(async move { /* probe loop */ });
        return;
    }

    // … rest of autostart from step 5.4 …
}
```

This avoids a flash of "Starting gst-pop…" on every activity
recreate.

## 9.4 External listener on 9000

Already handled in step 2.3 — `start_embedded` falls through to the
"adopt" branch and returns
`EmbeddedState::Running { externally_owned: true }`.

Make sure UI surfaces this distinction so debugging is sane:

```slint
if Bridge.gstpop-service-externally-owned: Text {
    text: @tr("(external daemon — Stop does not affect it)");
    color: Theme.text-secondary;
    font-size: Theme.font-size-label;
}
```

…and `stop_embedded` is a no-op (step 2.4):

```rust
let externally_owned = matches!(
    STATE.read().state,
    EmbeddedState::Running { externally_owned: true }
);
if externally_owned {
    tracing::info!("stop_embedded: listener is externally owned; no-op");
    return EmbeddedStatus::snapshot();
}
```

## 9.5 Bind fails (port held by a non-gst-pop process)

`start_embedded` returns `EmbeddedState::Error` with `last_error =
"failed to bind embedded gst-pop on 127.0.0.1:9000"`.

Step 4.1's `isErrorState` check in the service drops the foreground
state immediately so we don't display a persistent failed-state
notification:

```java
if (isErrorState(statusJson)) {
    stopForeground(STOP_FOREGROUND_REMOVE);
    stopSelf();
    return START_NOT_STICKY;
}
```

UI propagation: the status poller from step 7.6 picks up the error
state on the next tick and the Apply path's `push_error` from
step 5.3 surfaces the error string in `media-backend-error-text`.

To verify by hand:

```bash
$ adb shell "nc -l -p 9000 &"          # squat on the port
# Apply gst-pop in the UI → expect Error pill with bind failure text
$ adb shell pkill nc
# Apply again → expect Ready
```

## 9.6 Process death and revival

```
T0  app running, daemon on 9000
T1  OS kills process under pressure
T2  user relaunches app
T3  BackendLifecycle::new           → loads StoredBackendConfig
T4  autostart                       → request_service_start(config)
T5  Service.onStartCommand          → nativeStart
T6  start_embedded                  → fresh bind on 9000
```

The dead process took the previous `ServerHandle` with it. There is
no socket to clean up — the OS released it at T1. `probe_port_open`
at T6 returns false, the fresh bind succeeds.

If the OS is still holding the socket in TIME_WAIT (rare on
127.0.0.1, common after a `Ctrl+C`), the bind fails. Default
mitigation: bind with `SO_REUSEADDR`. Check `gstpop`'s `ServerHandle`
implementation; if it's not already set, add it upstream.

## 9.7 Two activities, one process

Not possible today (single `MainActivity` with `launchMode` defaulting
to `standard`, but the app is a singleton in practice). If you ever
support multi-window, the `READY`/`CLAIMED` atomics already serialise
correctly because they live in the cdylib — multiple activities just
become multiple JNI callers of the same statics.

## 9.8 Service revived with stale config

`START_STICKY` revives the service with a null intent. The null-intent
branch in step 4.1 self-stops, so stale config isn't a concern.

If you change that branch to auto-start, you also need to read the
last-known config from disk (the same `StoredBackendConfig::load`
that `BackendLifecycle::new` uses). The path is `<files_dir>/backend.json`
— see `src/backend/persistence.rs`.

## 9.9 Logging discipline

For the lifecycle invariants to be debuggable, every state transition
must log once. The current `embedded.rs` already does this for
start/adopt. Add for `stop_embedded`:

```rust
tracing::info!(
    "stop_embedded: dropping handle, previous_state={:?}",
    STATE.read().state
);
```

And in `GstPopService.onStartCommand`:

```java
Log.d(TAG, "onStartCommand action=" + action + " startId=" + startId);
```

After running the cases in 9.1–9.6 by hand, you should see one
distinct log per state change. If you see two consecutive
"Embedded gst-pop running on …" lines for the same port, something
in the lifecycle is double-starting.

Next: [10-test-plan.md](./10-test-plan.md).
