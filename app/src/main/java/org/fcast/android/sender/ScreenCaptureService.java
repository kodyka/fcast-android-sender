package org.fcast.android.sender;

import android.app.Activity;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.Service;
import android.content.Context;
import android.content.Intent;
import android.os.Build;
import android.os.IBinder;
import android.util.Log;

import androidx.annotation.Nullable;

public class ScreenCaptureService extends Service {
    private static final String TAG = "ScreenCaptureService";

    /** Action that {@link MainActivity} sets on the start intent after a
     *  successful MediaProjection consent. Any other action (or a null
     *  sticky restart) is treated as "ignore and stop". */
    public static final String ACTION_RESULT = "org.fcast.android.sender.SCREENCAP_RESULT";

    private Notification notification;

    public ScreenCaptureService() {
    }

    @Override
    public void onCreate() {
        super.onCreate();

        Log.d(TAG, "onCreate");

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            String NOTIF_CHANNEL_ID = "org.fcast.android.sender.ScreenCaptureService";
            NotificationChannel channel = new NotificationChannel(NOTIF_CHANNEL_ID, "ScreenCaptureService", NotificationManager.IMPORTANCE_NONE);
            channel.setLockscreenVisibility(Notification.VISIBILITY_PRIVATE);
            NotificationManager manager = (NotificationManager) getSystemService(Context.NOTIFICATION_SERVICE);
            if (manager != null) {
                manager.createNotificationChannel(channel);
                notification = new Notification.Builder(this, channel.getId()).build();
            }
        }
    }

    @Override
    public int onStartCommand(Intent intent, int flags, int startId) {
        Log.d(TAG, "onStartCommand intent=" + intent);

        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
            Log.w(TAG, "Pre-Q SDK; screen capture is unsupported");
            stopSelfResult(startId);
            return START_NOT_STICKY;
        }

        if (intent == null || !ACTION_RESULT.equals(intent.getAction())) {
            Log.w(TAG, "Ignoring sticky/null restart or unexpected action="
                    + (intent == null ? "null" : intent.getAction()));
            stopSelfResult(startId);
            return START_NOT_STICKY;
        }

        int resultCode = intent.getIntExtra("resultCode", Activity.RESULT_CANCELED);
        Intent data = intent.getParcelableExtra("data");
        if (resultCode != Activity.RESULT_OK || data == null) {
            Log.w(TAG, "Missing screen-capture result payload resultCode=" + resultCode);
            stopSelfResult(startId);
            return START_NOT_STICKY;
        }

        startForeground(1, notification);
        Log.d(TAG, "Started foreground");

        CaptureResultBus.deliver(resultCode, data);
        return START_NOT_STICKY;
    }

    public void stopCapture() {
        stopForeground(true);

        stopSelf();
    }

    @Override
    public void onDestroy() {
        stopCapture();
        super.onDestroy();
    }

    @Nullable
    @Override
    public IBinder onBind(Intent intent) {
        return null;
    }
}
