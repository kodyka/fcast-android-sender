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
