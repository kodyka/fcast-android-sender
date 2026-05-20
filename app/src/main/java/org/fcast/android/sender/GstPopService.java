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

    public static final String ACTION_START      = "org.fcast.android.sender.GSTPOP_START";
    public static final String ACTION_STOP       = "org.fcast.android.sender.GSTPOP_STOP";
    public static final String EXTRA_CONFIG_JSON = "config_json";

    private static final int    NOTIFICATION_ID = 2; // ScreenCaptureService uses 1
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
            // Call startForeground before any blocking work — Android 14 enforces
            // this within 5s of startForegroundService().
            startForeground(NOTIFICATION_ID, buildNotification("Starting gst-pop\u2026"));

            String config = intent.getStringExtra(EXTRA_CONFIG_JSON);
            String statusJson = GstPopServiceBridge.nativeStart(config != null ? config : "");
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
            String statusJson = GstPopServiceBridge.nativeStop();
            Log.d(TAG, "nativeStop -> " + statusJson);
            stopForeground(STOP_FOREGROUND_REMOVE);
            stopSelf();
            return START_NOT_STICKY;
        }

        // Sticky-restart with null intent: don't auto-restart the daemon —
        // leave it to the UI to ask for ACTION_START again.
        stopForeground(STOP_FOREGROUND_REMOVE);
        stopSelf();
        return START_NOT_STICKY;
    }

    @Override
    public void onDestroy() {
        // Defensive: ensure Rust state matches reality if we're torn down
        // without an explicit ACTION_STOP (e.g. task removal with stopWithTask).
        GstPopServiceBridge.nativeStop();
        super.onDestroy();
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        // No binder — UI polls via GstPopServiceBridge.queryStatus().
        return null;
    }

    // ── Notification helpers ──────────────────────────────────────────────────

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

        if (isExternallyOwned(statusJson)) {
            nm.notify(NOTIFICATION_ID, buildNotification("Using external gst-pop"));
            new android.os.Handler(getMainLooper()).postDelayed(() -> {
                stopForeground(STOP_FOREGROUND_REMOVE);
                stopSelf();
            }, 500);
            return;
        }

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
                case "starting": return "Starting gst-pop on " + bind + ":" + port + "\u2026";
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

    private static boolean isExternallyOwned(String statusJson) {
        try {
            JSONObject o = new JSONObject(statusJson);
            return "running".equals(o.optString("state")) && o.optBoolean("externally_owned", false);
        } catch (JSONException e) {
            return false;
        }
    }
}
