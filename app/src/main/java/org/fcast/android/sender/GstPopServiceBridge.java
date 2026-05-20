package org.fcast.android.sender;

import android.content.Context;
import android.content.Intent;
import android.util.Log;

/**
 * Thin wrapper around the native gst-pop daemon lifecycle and the Android
 * service that hosts it. All UI/Activity code MUST go through this class —
 * direct startService / native calls bypass the lifecycle bookkeeping.
 */
public final class GstPopServiceBridge {
    private static final String TAG = "GstPopServiceBridge";

    private GstPopServiceBridge() {}

    // ── Public API ────────────────────────────────────────────────────────────

    /**
     * Request the service to start. Returns immediately — the service drives
     * the native start on its onStartCommand thread. UI polls {@link #queryStatus()}
     * for the resulting state.
     */
    public static void start(Context context, String configJson) {
        Intent intent = new Intent(context, GstPopService.class)
            .setAction(GstPopService.ACTION_START)
            .putExtra(GstPopService.EXTRA_CONFIG_JSON,
                      configJson == null ? "{}" : configJson);
        try {
            context.startForegroundService(intent);
        } catch (Exception e) {
            Log.e(TAG, "startForegroundService failed: " + e);
        }
    }

    /** Request graceful shutdown. */
    public static void stop(Context context) {
        Intent intent = new Intent(context, GstPopService.class)
            .setAction(GstPopService.ACTION_STOP);
        try {
            context.startService(intent);
        } catch (Exception e) {
            Log.e(TAG, "stopService failed: " + e);
        }
    }

    /**
     * Synchronous status query. Returns the JSON-serialised EmbeddedStatus
     * from Rust. Safe to call from any thread.
     */
    public static String queryStatus() {
        return nativeGetGstPopServiceStatus();
    }

    // ── Called only from GstPopService — not from UI code ────────────────────

    static String nativeStart(String configJson) {
        return nativeStartGstPopServiceHost(configJson);
    }

    static String nativeStop() {
        return nativeStopGstPopServiceHost();
    }

    // ── Native exports (Java_org_fcast_android_sender_GstPopServiceBridge_* in lib.rs) ──

    private static native String nativeStartGstPopServiceHost(String configJson);
    private static native String nativeStopGstPopServiceHost();
    private static native String nativeGetGstPopServiceStatus();
}
