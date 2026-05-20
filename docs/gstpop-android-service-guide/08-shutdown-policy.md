# 8 · Shutdown policy

Decide *exactly* when the daemon dies. Implicit "depends on
Activity/Service interaction" is what causes confusing lifecycle
bugs.

## 8.1 Per-trigger matrix

| Trigger | Service action | Daemon action |
|---|---|---|
| User switches backend to `migration` (Apply) | `request_service_stop()` from step 5 | `stop_embedded` drops `ServerHandle`. |
| User switches to remote gst-pop URL | `request_service_stop()` | Same — local daemon not needed for a remote URL. |
| User taps **Stop** in Media Backend panel | `Bridge.stop-gstpop-service()` → `request_service_stop()` | Same. |
| User taps **Stop** in foreground notification | `GstPopService.ACTION_STOP` → `nativeStop` → `stopSelf()` | `stop_embedded`; state → `Stopped`. |
| User backgrounds the app (Home button) | None — service keeps running. | Daemon keeps running. |
| User finishes activity (back from root) | None — service keeps running. | Daemon keeps running. |
| User swipes the task away | `stopWithTask="false"` → none. | Daemon keeps running. |
| OS kills under memory pressure | `START_STICKY` restarts service with null intent. | Null-intent branch in `onStartCommand` calls `stopSelf()` (see step 4.1). Daemon does **not** auto-restart. |
| Process death | Service gone. | Daemon gone (same process). |
| App relaunch after process death | If persisted config is `gst-pop` + localhost, `BackendLifecycle::autostart` fires `request_service_start`. | Daemon restarts. |
| Reboot | Service gone (START_STICKY does not survive reboot). | Same as fresh launch. |

## 8.2 Why the null-intent branch is a no-op

When Android restarts a `START_STICKY` service after killing it for
memory, it delivers `onStartCommand(intent = null, …)`. The intuitive
move is "auto-start the daemon" — but that means a foreground
notification can pop up out of nowhere on a user who explicitly
stopped the backend, or whose app was killed in the background. We
choose the safer option: drop the foreground state and rely on
`BackendLifecycle::autostart` (which runs on activity recreate) to
ask for a fresh start if the config still wants one.

If you want stickiness across OS kills, change the null-intent
branch to call `nativeStart("")` and `updateNotification(…)`. Document
that decision; the surprise-notification cost is real.

## 8.3 Stop-during-start race

User clicks Apply (localhost gst-pop), then immediately clicks Stop
in the notification before the bind finishes:

```
T0  UI Apply              → startForegroundService(ACTION_START)
T1  Service onStartCommand → nativeStart(config) [blocking on bind]
T2  User taps Stop         → startService(ACTION_STOP)
T3  Service onStartCommand → nativeStop()
```

`stop_embedded` (step 2.4) handles this with the 500ms grace window:

```rust
for _ in 0..10 {
    if !matches!(STATE.read().state, EmbeddedState::Starting) { break; }
    tokio::time::sleep(Duration::from_millis(50)).await;
}
```

After the grace window, whatever state the start landed in is the
state we stop from. Worst case: bind succeeded just before stop ran,
and we drop the handle a few ms later — clean.

## 8.4 Apply during a previous Apply

User clicks Apply, then re-clicks Apply with different settings:

```
T0  UI Apply A           → startForegroundService(ACTION_START, configA)
T1  Service nativeStart  → starts daemon on portA
T2  UI Apply B           → startForegroundService(ACTION_START, configB)
T3  Service nativeStart  → start_embedded(portB)
```

If `portA == portB`, step T3 hits the `READY.load + STATE.port match`
fast path in `start_embedded` and returns the existing status — no
restart. If `portA != portB`, the fast path misses, the `probe_port_open`
call sees no listener on `portB`, and a fresh bind happens — but the
old `ServerHandle` for `portA` is leaked because `stop_embedded` was
never called.

Fix: have `start_embedded` detect a port change and call
`stop_embedded` internally first. Add this at the top of
`start_embedded`:

```rust
{
    let st = STATE.read();
    if st.port != 0 && st.port != port && matches!(
        st.state,
        EmbeddedState::Running { externally_owned: false }
    ) {
        drop(st);
        let _ = stop_embedded().await;
    }
}
```

## 8.5 Foreground-service grace period

Android 14 requires `startForeground()` within ~5 seconds of
`startForegroundService()`. Step 4.1's `onStartCommand` calls
`startForeground(NOTIFICATION_ID, buildNotification("Starting…"))`
*before* `nativeStart`, so the grace window is satisfied immediately
and `nativeStart` can take as long as it needs.

If you ever move `nativeStart` *before* `startForeground`, you will
get `ForegroundServiceDidNotStartInTimeException` and Android will
ANR the app. Don't.

## 8.6 Notification persistence

`updateNotification` after `nativeStart` returns updates the text to
either "gst-pop running on 127.0.0.1:9000" or the error string. The
notification stays foregrounded until `stopForeground` + `stopSelf`
run. In the error case, step 4.1's `isErrorState` check drops the
foreground state immediately so the user doesn't see a permanent
"gst-pop error: …" pill in the shade.

## 8.7 What about `MediaBackend::shutdown()`?

`apply` in step 5.5 calls `previous.shutdown()` on the outgoing
backend. For `GstPopBackend::shutdown` that closes the cached
WebSocket connection. It does **not** stop the daemon — that's
`request_service_stop`'s job. Keeping these two concerns separate is
intentional: a future "switch ports without restarting daemon"
feature only needs to drop the client connection, not the daemon.

Next: [09-race-recovery.md](./09-race-recovery.md).
