# 02 — Remove `LocalBroadcastManager`

**Priority:** Highest · **Effort:** Medium · **Estimated PR size:** ~150 LOC across 3 files.

## Goal

Replace the deprecated `LocalBroadcastManager` channel between `ScreenCaptureService`
and `MainActivity` with an explicit in-process callback. The service should not need
to know about `MainActivity` at all — and certainly not by name.

## Report finding

> "`ScreenCaptureService` uses `LocalBroadcastManager` to send the capture result
> back to `MainActivity`, and `MainActivity` registers a matching receiver. AndroidX
> deprecated `LocalBroadcastManager` specifically because it behaves like an
> application-wide event bus, encourages layer violations, and forces in-process
> communication through `Intent`s."

— `deep-research-report-3.md`, "Detailed findings".

The refactor target proposed in the report is an explicit interface, not another
broadcast bus:

> "Replace with an explicit coordinator contract: Activity handles permission
> result, service only owns ongoing notification/runtime; if a callback is still
> needed, use a typed in-process channel rather than intents."

## Pre-state on `main`

`ScreenCaptureService.java:61`:

```java
LocalBroadcastManager.getInstance(this).sendBroadcast(broadcastIntent);
```

`MainActivity.java:323-347`:

```java
public class CaptureBroadcastReceiver extends BroadcastReceiver {
    @Override
    public void onReceive(Context context, Intent intent) {
        Log.d(TAG, "Broadcast event intent=" + intent);
        if (ACTION_MEDIA_PROJECTION_STARTED.equals(intent.getAction())) {
            int resultCode = intent.getIntExtra("resultCode", Activity.RESULT_CANCELED);
            Intent data = intent.getParcelableExtra("data");
            initializeCapture(resultCode, data);
        }
    }
}

private final CaptureBroadcastReceiver receiver = new CaptureBroadcastReceiver();

@Override
protected void onCreate(Bundle savedInstanceState) {
    // …
    IntentFilter filter = new IntentFilter(ACTION_MEDIA_PROJECTION_STARTED);
    filter.addCategory(Intent.CATEGORY_DEFAULT);
    LocalBroadcastManager.getInstance(this).registerReceiver(receiver, filter);
    // …
}
```

## Target state

The activity already owns the permission flow (it calls `startActivityForResult`
on the `MediaProjectionManager` intent). There is no architectural reason for the
service to send the result back through an event bus — the activity already has it
by the time the service is started.

### Cleanest target (recommended)

Stop sending the result through the service at all. The activity:

1. Receives the projection result in `onActivityResult` (already does).
2. Calls `startForegroundService(...)` to satisfy the API requirement that screen
   capture be performed from a foreground service.
3. Passes the result to its own capture pipeline via a direct call once
   `onServiceConnected` (or `onStartCommand` confirmation via a typed bound
   service) fires.

### Minimal target (also acceptable)

Keep the service receiving the result, but expose a typed callback on a
single-application singleton — registered by the activity, cleared on
`onDestroy`. No `Intent`, no `BroadcastReceiver`.

```java
// app/src/main/java/org/fcast/android/sender/CaptureResultBus.java   (NEW)
package org.fcast.android.sender;

import android.content.Intent;

import java.util.concurrent.atomic.AtomicReference;

/**
 * Single-shot, typed in-process channel between {@link ScreenCaptureService}
 * and the activity. Replaces LocalBroadcastManager.
 *
 * Lifetime contract:
 *   - Activity calls {@link #setListener} in onResume, {@link #clearListener} in onPause.
 *   - Service calls {@link #deliver} from its main thread.
 */
public final class CaptureResultBus {
    public interface Listener {
        void onCaptureResult(int resultCode, Intent data);
    }

    private static final AtomicReference<Listener> LISTENER = new AtomicReference<>();

    private CaptureResultBus() {}

    public static void setListener(Listener listener) {
        LISTENER.set(listener);
    }

    public static void clearListener(Listener expected) {
        LISTENER.compareAndSet(expected, null);
    }

    public static boolean deliver(int resultCode, Intent data) {
        Listener l = LISTENER.get();
        if (l == null) return false;
        l.onCaptureResult(resultCode, data);
        return true;
    }
}
```

### `ScreenCaptureService` change

```diff
-import androidx.localbroadcastmanager.content.LocalBroadcastManager;
-
@@
-    Intent broadcastIntent = new Intent(this, MainActivity.CaptureBroadcastReceiver.class);
-    broadcastIntent.setAction(ACTION_MEDIA_PROJECTION_STARTED);
-    broadcastIntent.putExtra("resultCode", resultCode);
-    broadcastIntent.putExtra("data", data);
-
-    startForeground(1, notification);
-    LocalBroadcastManager.getInstance(this).sendBroadcast(broadcastIntent);
+    startForeground(1, notification);
+    boolean delivered = CaptureResultBus.deliver(resultCode, data);
+    if (!delivered) {
+        Log.w(TAG, "No active listener — dropping capture result");
+    }
```

### `MainActivity` change

```diff
-import androidx.localbroadcastmanager.content.LocalBroadcastManager;
-
@@
-    public class CaptureBroadcastReceiver extends BroadcastReceiver {
-        @Override
-        public void onReceive(Context context, Intent intent) {
-            Log.d(TAG, "Broadcast event intent=" + intent);
-            if (ACTION_MEDIA_PROJECTION_STARTED.equals(intent.getAction())) {
-                int resultCode = intent.getIntExtra("resultCode", Activity.RESULT_CANCELED);
-                Intent data = intent.getParcelableExtra("data");
-                initializeCapture(resultCode, data);
-            }
-        }
-    }
-
-    private final CaptureBroadcastReceiver receiver = new CaptureBroadcastReceiver();
+    private final CaptureResultBus.Listener captureListener =
+        (resultCode, data) -> initializeCapture(resultCode, data);
@@
-        IntentFilter filter = new IntentFilter(ACTION_MEDIA_PROJECTION_STARTED);
-        filter.addCategory(Intent.CATEGORY_DEFAULT);
-        LocalBroadcastManager.getInstance(this).registerReceiver(receiver, filter);
+        CaptureResultBus.setListener(captureListener);
@@
+    @Override
+    protected void onDestroy() {
+        CaptureResultBus.clearListener(captureListener);
+        super.onDestroy();
+    }
```

> Step 03 expands `onDestroy` with the remaining cleanup (HandlerThread, display
> listener, etc.). Here we add only the listener-clear.

## Drop the dependency

`app/build.gradle` no longer needs `androidx.localbroadcastmanager:localbroadcastmanager`.
Search for it:

```bash
rg -n 'localbroadcastmanager' app/build.gradle
```

If it appears as a direct dependency, remove it. If it only arrives transitively,
no change is required.

The `androidx.localbroadcastmanager.content.LocalBroadcastManager` import in
`ScreenCaptureService.java` and `MainActivity.java` must be deleted in the same PR.

## Testing

| Test                                                              | How                                                              |
|-------------------------------------------------------------------|------------------------------------------------------------------|
| Capture still starts on first permission grant                    | Manual.                                                          |
| Capture starts on subsequent permission grants                    | Stop, restart capture from UI; confirm activity gets the result. |
| Activity destroyed mid-flow does not crash                        | Rotate device while permission dialog is up.                     |
| `LocalBroadcastManager` import truly gone                         | `rg -n 'LocalBroadcastManager' app/src/main` returns no hits.    |
| Service no longer references `MainActivity.CaptureBroadcastReceiver` | `rg -n 'CaptureBroadcastReceiver' app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java` returns no hits. |
| Lint                                                              | `./gradlew :app:lint`.                                            |

## Rollback

Revert the three files. Re-add the import. The `CaptureResultBus` class can stay
in the codebase (unused) without affecting behaviour, which makes a forward-roll
much faster the next time.

## Follow-ups (not in this PR)

- Activity-side cleanup beyond clearing the listener — **Step 03**.
- Move capture orchestration out of the activity entirely — **Step 04**.
