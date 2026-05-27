package org.fcast.android.sender;

import android.content.Context;
import android.content.Intent;
import android.util.Log;

/**
 * Thin wrapper around the native migration-runtime lifecycle and the Android
 * service that hosts it. All UI/Activity/Rust code MUST go through this class —
 * direct startService / native calls bypass the lifecycle bookkeeping.
 *
 * Mirrors {@link GstPopServiceBridge}; the migration runtime currently takes
 * no start-time config, but a configJson parameter is preserved for symmetry.
 */
public final class MigrationRuntimeServiceBridge {
    private static final String TAG = "MigrationRuntimeServiceBridge";

    private MigrationRuntimeServiceBridge() {}

    // ── Public API ────────────────────────────────────────────────────────────

    /**
     * Request the service to start. Returns immediately — the service drives
     * the native start on its onStartCommand thread. UI polls
     * {@link #queryStatus()} for the resulting state.
     */
    public static void start(Context context, String configJson) {
        Intent intent = new Intent(context, MigrationRuntimeService.class)
            .setAction(MigrationRuntimeService.ACTION_START)
            .putExtra(MigrationRuntimeService.EXTRA_CONFIG_JSON,
                      configJson == null ? "{}" : configJson);
        try {
            context.startForegroundService(intent);
        } catch (Exception e) {
            Log.e(TAG, "startForegroundService failed: " + e);
        }
    }

    /** Request graceful shutdown. */
    public static void stop(Context context) {
        Intent intent = new Intent(context, MigrationRuntimeService.class)
            .setAction(MigrationRuntimeService.ACTION_STOP);
        try {
            context.startService(intent);
        } catch (Exception e) {
            Log.e(TAG, "stopService failed: " + e);
        }
    }

    /**
     * Synchronous status query. Returns the JSON-serialised status from Rust.
     * Safe to call from any thread.
     */
    public static String queryStatus() {
        return nativeGetMigrationRuntimeStatus();
    }

    // ── Called only from MigrationRuntimeService — not from UI code ──────────

    static String nativeStart(String configJson) {
        return nativeStartMigrationRuntimeHost(configJson);
    }

    static String nativeStop() {
        return nativeStopMigrationRuntimeHost();
    }

    // ── Native exports (Java_org_fcast_android_sender_MigrationRuntimeServiceBridge_* in lib.rs) ──

    private static native String nativeStartMigrationRuntimeHost(String configJson);
    private static native String nativeStopMigrationRuntimeHost();
    private static native String nativeGetMigrationRuntimeStatus();
}
