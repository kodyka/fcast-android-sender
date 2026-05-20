# 4 · The Android service

`GstPopService` is a foreground service that owns the daemon lifetime
for the process. It is modelled on the existing
`ScreenCaptureService.java` (~84 lines) — same notification dance,
same `START_STICKY` shape, different `foregroundServiceType`.

## 4.1 `GstPopService.java`

New file:
`app/src/main/java/org/fcast/android/sender/GstPopService.java`.

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

public final class GstPopService extends Service {
    private static final String TAG = "GstPopService";

    public static final String ACTION_START = "org.fcast.android.sender.GSTPOP_START";
    public static final String ACTION_STOP  = "org.fcast.android.sender.GSTPOP_STOP";
    public static final String EXTRA_CONFIG_JSON = "config_json";

    private static final int    NOTIFICATION_ID = 2; // ScreenCaptureService uses 1.
    private static final String CHANNEL_ID      = "org.fcast.android.sender.GstPopService";

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
            // Foreground *before* anything that could block — Android 14
            // forces this within 5s of startForegroundService().
            startForeground(NOTIFICATION_ID, buildNotification("Starting gst-pop…"));

            String config = intent.getStringExtra(EXTRA_CONFIG_JSON);
            String statusJson = GstPopServiceBridge.nativeStart(config != null ? config : "");
            Log.d(TAG, "nativeStart -> " + statusJson);
            updateNotification(statusJson);

            // If the native start failed outright, don't sit around as a
            // permanent foreground "Error" — drop the notification.
            if (isErrorState(statusJson)) {
                stopForeground(STOP_FOREGROUND_REMOVE);
                stopSelf();
                return START_NOT_STICKY;
            }

            // START_STICKY: if killed under memory pressure, the OS
            // restarts the service with a null intent. See the null-intent
            // branch below for why we choose not to auto-restart the daemon.
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
        // surprise foreground service after the user explicitly stopped it
        // or the process died.
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        // Defensive: if onDestroy fires without ACTION_STOP (e.g. task
        // removal with stopWithTask=true, or forced stop), make sure
        // Rust state matches reality.
        GstPopServiceBridge.nativeStop();
        super.onDestroy();
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        // No binder API. UI polls via GstPopServiceBridge.queryStatus().
        // Promote to a Messenger / AIDL if you want push updates later.
        return null;
    }

    // ── Notification helpers ─────────────────────────────────────────

    private void ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) return;
        NotificationChannel channel = new NotificationChannel(
            CHANNEL_ID, "gst-pop backend", NotificationManager.IMPORTANCE_LOW);
        channel.setDescription("Embedded gst-pop daemon hosting");
        channel.setShowBadge(false);
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

        return new Notification.Builder(this, CHANNEL_ID)
            .setSmallIcon(android.R.drawable.ic_media_play)
            .setContentTitle("FCast gst-pop backend")
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
            String bind  = o.optString("bind", "127.0.0.1");
            int port     = o.optInt("port", 9000);
            switch (state) {
                case "running":  return "gst-pop running on " + bind + ":" + port;
                case "starting": return "Starting gst-pop on " + bind + ":" + port + "…";
                case "stopped":  return "gst-pop stopped";
                case "error":
                    String e = o.optString("last_error", "unknown error");
                    return "gst-pop error: " + e;
                default: return "gst-pop " + state;
            }
        } catch (JSONException e) {
            return "gst-pop";
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

## 4.2 `AndroidManifest.xml` diff

Edit `app/src/main/AndroidManifest.xml`:

```diff
 <?xml version="1.0" encoding="utf-8"?>
 <manifest xmlns:android="http://schemas.android.com/apk/res/android"
     xmlns:tools="http://schemas.android.com/tools">

     <uses-permission android:name="android.permission.FOREGROUND_SERVICE" />
     <uses-permission android:name="android.permission.FOREGROUND_SERVICE_MEDIA_PROJECTION" />
+    <uses-permission android:name="android.permission.FOREGROUND_SERVICE_DATA_SYNC" />
     <uses-permission android:name="android.permission.INTERNET" />
     <uses-permission android:name="android.permission.ACCESS_NETWORK_STATE" />

     <application
         android:icon="@mipmap/ic_launcher"
         android:label="FCast Sender"
         …>

         <service
             android:name=".ScreenCaptureService"
             android:exported="false"
             android:stopWithTask="true"
             android:foregroundServiceType="mediaProjection" />

+        <service
+            android:name=".GstPopService"
+            android:exported="false"
+            android:stopWithTask="false"
+            android:foregroundServiceType="dataSync" />

         <activity
             android:name="com.journeyapps.barcodescanner.CaptureActivity"
             …
         </activity>
     </application>
 </manifest>
```

Why these specific attributes:

- **`android:exported="false"`** — only this app starts the service.
  Required for FGS launched by the app itself.
- **`android:stopWithTask="false"`** — task removal (swipe-away)
  **does not** kill the daemon. The user explicitly stopping the
  backend, or switching away from gst-pop, is the only path that
  tears it down. See `08-shutdown-policy.md`.
- **`android:foregroundServiceType="dataSync"`** — the most defensible
  match for a long-running localhost server that isn't user-initiated.
  See `12-open-decisions.md` for why this beats `mediaPlayback`.

## 4.3 Process & class loader

The service runs in the same process as `MainActivity` (no
`android:process` attribute). This is what makes the bridge cheap:
`nativeStart` from `GstPopService.onStartCommand` lands in the same
`HOST_RUNTIME` as the activity, and `ServerHandle` survives across
activity finish.

Do **not** put the service in a separate process. You'd need a second
`System.loadLibrary("fcastsender")` and a second tokio runtime, and
the activity would be unable to read `embedded_status()` directly —
all status queries would have to round-trip through the bridge IPC.

## 4.4 Optional: stop notification when daemon is externally owned

When the user has their own gst-pop running, `start_embedded` sets
`externally_owned: true`. Hosting a foreground notification in that
case is misleading. Detect it in `updateNotification`:

```java
private void updateNotification(String statusJson) {
    NotificationManager nm = getSystemService(NotificationManager.class);
    if (nm == null) return;
    if (isExternallyOwned(statusJson)) {
        // We're not actually hosting anything — let the user's own
        // daemon do the work. Self-stop after a short delay so the
        // FGS-start contract is satisfied.
        nm.notify(NOTIFICATION_ID, buildNotification("Using external gst-pop"));
        new android.os.Handler(getMainLooper()).postDelayed(() -> {
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
        }, 500);
        return;
    }
    nm.notify(NOTIFICATION_ID, buildNotification(describe(statusJson)));
}

private static boolean isExternallyOwned(String statusJson) {
    try {
        JSONObject o = new JSONObject(statusJson);
        return "running".equals(o.optString("state")) && o.optBoolean("externally_owned", false);
    } catch (JSONException e) {
        return false;
    }
}
```

This keeps the service from showing a "we're hosting gst-pop" lie
when in fact it has nothing to host.

Next: [05-rewire-lifecycle.md](./05-rewire-lifecycle.md).
