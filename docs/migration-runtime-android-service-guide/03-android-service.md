# 03 — Android service (`MigrationRuntimeService.java`)

Mirror of `GstPopService.java`, with the gst-pop-specific deviations
called out in [00-plan-review.md](./00-plan-review.md). Full source
follows.

## 3.1 Full file content

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

## 3.2 Differences from `GstPopService` (intentional)

| # | `GstPopService` | `MigrationRuntimeService` | Why |
|---|----------------|---------------------------|-----|
| 1 | `describe()` reads `bind`, `port`, `state`, `last_error` from JSON. | `describe()` reads only `state`, `last_error`. | Migration runtime has no port concept exposed in its status — see [00 row 3](./00-plan-review.md). |
| 2 | `updateNotification()` has an `isExternallyOwned` branch that triggers a 500 ms self-stop. | No such branch. | Migration runtime is always in-process; there is no external owner — see [00 row 10](./00-plan-review.md). |
| 3 | Channel display name: `"gst-pop backend"`. | `"Migration runtime"`. | User-visible label. |
| 4 | Channel description: `"Embedded gst-pop daemon hosting"`. | `"Embedded migration runtime hosting"`. | User-visible label. |
| 5 | Content title: `"FCast gst-pop backend"`. | `"FCast migration runtime"`. | User-visible label. |
| 6 | Action constants: `GSTPOP_START`/`GSTPOP_STOP`. | `MIGRATION_RUNTIME_START`/`MIGRATION_RUNTIME_STOP`. | Avoids intent-action collisions when both services are running. |
| 7 | `NOTIFICATION_ID = 2`. | `NOTIFICATION_ID = 3`. | Coexists with `ScreenCaptureService` (1) and `GstPopService` (2). |
| 8 | `CHANNEL_ID = "org.fcast.android.sender.GstPopService"`. | `CHANNEL_ID = "org.fcast.android.sender.MigrationRuntimeService"`. | Each service owns its own notification channel — required to surface independent state pills. |

## 3.3 Lifecycle invariants

* `onStartCommand` calls `startForeground(NOTIFICATION_ID, …)` **before**
  doing any work that might block. Android 14 enforces the
  startForeground-within-5s rule and will kill the process otherwise.
* On `ACTION_START` failure (`isErrorState(...)` returns true), the
  service removes its notification and stops itself. The UI/Rust caller
  is expected to retry by re-issuing `ACTION_START`.
* On `ACTION_STOP`, the service stops itself synchronously — there is
  no asynchronous shutdown path because `shutdown_graph_runtime()` is
  itself synchronous and joins the refresh + command-server threads
  before returning (`runtime.rs:312-320`).
* `onDestroy` defensively calls `nativeStop()` so the Rust state matches
  reality if the OS tears down the service without an explicit
  `ACTION_STOP` (e.g. process-killed while `stopWithTask=false` keeps
  the service alive across task removal but a low-memory kill still
  fires `onDestroy`).
* `onBind` returns `null` — there is no IPC interface; status is polled
  via `MigrationRuntimeServiceBridge.queryStatus()`.

## 3.4 Why `START_NOT_STICKY` after a null-intent restart

Android may redeliver `onStartCommand(null, …)` to a sticky service
after the OS killed and re-created it. For both `GstPopService` and the
migration runtime service we explicitly **do not** auto-restart in that
case — the UI/Rust caller must re-request via `ACTION_START`. This
keeps service start tied to an explicit user/business decision and
avoids resurrection of a runtime that the user just stopped.
