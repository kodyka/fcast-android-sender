# 01 — Harden `ScreenCaptureService`

**Priority:** Highest · **Effort:** Low · **Estimated PR size:** ~30 LOC

## Goal

Make the `ScreenCaptureService` start contract null-safe and non-sticky. The service
is a one-shot hand-off layer for `MediaProjection` permission results — it must not
auto-restart with a null intent, and it must validate every extra before use.

## Report finding

> "`ScreenCaptureService.onStartCommand()` reads extras from `intent` without a
> null check and returns `START_STICKY`. That combination is unsafe, because sticky
> services can be restarted with a null intent. The service also only acts as a
> hand-off layer and does not look like something that should be sticky in the
> first place."

— `deep-research-report-3.md`, "Detailed findings".

## Pre-state on `main`

`app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java:44-65`:

```java
@Override
public int onStartCommand(Intent intent, int flags, int startId) {
    Log.d(TAG, "onStartCommand intent=" + intent);

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
        int resultCode = intent.getIntExtra("resultCode", -1);           // ← unsafe on sticky restart
        Intent data = intent.getParcelableExtra("data");                 // ← unsafe on sticky restart

        Intent broadcastIntent = new Intent(this, MainActivity.CaptureBroadcastReceiver.class);
        broadcastIntent.setAction(ACTION_MEDIA_PROJECTION_STARTED);
        broadcastIntent.putExtra("resultCode", resultCode);
        broadcastIntent.putExtra("data", data);

        startForeground(1, notification);

        Log.d(TAG, "Started foreground");

        LocalBroadcastManager.getInstance(this).sendBroadcast(broadcastIntent);
    }

    return START_STICKY;
}
```

Three concrete problems:

1. If Android restarts the service with `intent == null` (sticky semantics),
   line 49's `intent.getIntExtra(...)` throws `NullPointerException`.
2. The service has no `Action` qualifier. Any caller targeting the same class
   triggers the broadcast path.
3. `START_STICKY` is wrong: the service exists only to ferry one `Intent` from the
   permission UI back to the activity.

## Target state

```java
public static final String ACTION_RESULT = "org.fcast.android.sender.SCREENCAP_RESULT";

@Override
public int onStartCommand(Intent intent, int flags, int startId) {
    Log.d(TAG, "onStartCommand intent=" + intent);

    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
        stopSelfResult(startId);
        return START_NOT_STICKY;
    }
    if (intent == null || !ACTION_RESULT.equals(intent.getAction())) {
        Log.w(TAG, "Ignoring sticky/null restart or unexpected action");
        stopSelfResult(startId);
        return START_NOT_STICKY;
    }

    int resultCode = intent.getIntExtra("resultCode", Activity.RESULT_CANCELED);
    Intent data = intent.getParcelableExtra("data");
    if (resultCode != Activity.RESULT_OK || data == null) {
        Log.w(TAG, "Missing screen-capture result payload");
        stopSelfResult(startId);
        return START_NOT_STICKY;
    }

    Intent broadcastIntent = new Intent(this, MainActivity.CaptureBroadcastReceiver.class);
    broadcastIntent.setAction(ACTION_MEDIA_PROJECTION_STARTED);
    broadcastIntent.putExtra("resultCode", resultCode);
    broadcastIntent.putExtra("data", data);

    startForeground(1, notification);
    LocalBroadcastManager.getInstance(this).sendBroadcast(broadcastIntent);   // step 02 replaces this
    return START_NOT_STICKY;
}
```

> Step 02 replaces the `LocalBroadcastManager` call. This step keeps that line so the
> diff is genuinely "low" effort and can land as its own PR.

## Diff

```diff
diff --git a/app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java b/app/src/main/java/org/fcast/android/sender/ScreenCaptureService.java
@@
 public class ScreenCaptureService extends Service {
     private static final String TAG = "ScreenCaptureService";
+    public static final String ACTION_RESULT = "org.fcast.android.sender.SCREENCAP_RESULT";
@@
     @Override
     public int onStartCommand(Intent intent, int flags, int startId) {
         Log.d(TAG, "onStartCommand intent=" + intent);

-        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
-            int resultCode = intent.getIntExtra("resultCode", -1);
-            Intent data = intent.getParcelableExtra("data");
-
-            Intent broadcastIntent = new Intent(this, MainActivity.CaptureBroadcastReceiver.class);
-            broadcastIntent.setAction(ACTION_MEDIA_PROJECTION_STARTED);
-            broadcastIntent.putExtra("resultCode", resultCode);
-            broadcastIntent.putExtra("data", data);
-
-            startForeground(1, notification);
-
-            Log.d(TAG, "Started foreground");
-
-            LocalBroadcastManager.getInstance(this).sendBroadcast(broadcastIntent);
-        }
-
-        return START_STICKY;
+        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
+            stopSelfResult(startId);
+            return START_NOT_STICKY;
+        }
+        if (intent == null || !ACTION_RESULT.equals(intent.getAction())) {
+            Log.w(TAG, "Ignoring sticky/null restart or unexpected action");
+            stopSelfResult(startId);
+            return START_NOT_STICKY;
+        }
+        int resultCode = intent.getIntExtra("resultCode", Activity.RESULT_CANCELED);
+        Intent data = intent.getParcelableExtra("data");
+        if (resultCode != Activity.RESULT_OK || data == null) {
+            Log.w(TAG, "Missing screen-capture result payload");
+            stopSelfResult(startId);
+            return START_NOT_STICKY;
+        }
+
+        Intent broadcastIntent = new Intent(this, MainActivity.CaptureBroadcastReceiver.class);
+        broadcastIntent.setAction(ACTION_MEDIA_PROJECTION_STARTED);
+        broadcastIntent.putExtra("resultCode", resultCode);
+        broadcastIntent.putExtra("data", data);
+
+        startForeground(1, notification);
+        LocalBroadcastManager.getInstance(this).sendBroadcast(broadcastIntent);
+        return START_NOT_STICKY;
     }
```

`Activity` and `START_NOT_STICKY` are already imported transitively via
`android.app.Service`; if the lint job flags the `Activity.RESULT_*` references,
add `import android.app.Activity;`.

## Update the caller

`MainActivity.java` already builds the `Intent` that starts this service. Find the
call site (search for `new Intent(this, ScreenCaptureService.class)`) and qualify it:

```diff
 Intent svc = new Intent(this, ScreenCaptureService.class)
+    .setAction(ScreenCaptureService.ACTION_RESULT)
     .putExtra("resultCode", resultCode)
     .putExtra("data", data);
 startForegroundService(svc);
```

## Testing

| Test                                                            | How                                                       |
|-----------------------------------------------------------------|-----------------------------------------------------------|
| Permission flow still works                                      | Manual: grant projection, confirm capture starts.        |
| Service is not restarted by Android after process death          | `adb shell am kill org.fcast.android.sender` then `adb logcat | grep ScreenCaptureService` — should not see a new `onStartCommand` until the user re-triggers capture. |
| Null intent path no longer crashes                              | `adb shell am startservice --user 0 -n org.fcast.android.sender/.ScreenCaptureService` (no extras) — service must log "Ignoring sticky/null restart" and stop. |
| Headless Slint UI tests                                          | `cargo test -p fcastsender --test ui_snapshots` — unchanged. |
| Android Lint                                                     | `./gradlew :app:lint` — should not regress. |

## Rollback

Revert the file. No data, no state migration, no manifest change.

If a regression appears only on cold-start after process death, restore the
`START_STICKY` line but keep the null guard. That preserves the original auto-
restart behaviour while removing the NPE risk.

## Follow-ups (not in this PR)

- Replace `LocalBroadcastManager.sendBroadcast` with a typed callback — **Step 02**.
- Move the entire capture coordination out of `MainActivity` — **Step 04**.
