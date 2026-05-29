package org.fcast.android.sender.runtime

import android.content.Context
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import org.fcast.android.sender.GstPopServiceBridge
import org.fcast.android.sender.MainActivity
import org.fcast.android.sender.MigrationRuntimeServiceBridge
import org.json.JSONObject

/**
 * JNI-backed RuntimeBridge — the only RuntimeBridge in the production graph.
 *
 * Implementations notes:
 *  - All native calls happen on Dispatchers.IO. The service bridges call
 *    Service-level startForegroundService(), which itself returns quickly, but
 *    the daemon-side bootstrap can take 100+ ms. Keep main thread free.
 *  - Errors are mapped to a BackendStatus(state="error") rather than thrown,
 *    so callers don't have to write try/catch around every call.
 */
class JniRuntimeBridge(
    private val appContext: Context,
) : RuntimeBridge {

    override suspend fun startEmbeddedBackend(
        kind: BackendKind,
        configJson: String,
    ): BackendStatus = withContext(Dispatchers.IO) {
        try {
            val raw = when (kind) {
                BackendKind.GSTPOP    -> {
                    GstPopServiceBridge.start(appContext, configJson)
                    GstPopServiceBridge.queryStatus()
                }
                BackendKind.MIGRATION -> {
                    MigrationRuntimeServiceBridge.start(appContext, configJson)
                    MigrationRuntimeServiceBridge.queryStatus()
                }
            }
            parseStatus(raw)
        } catch (t: Throwable) {
            Log.e(TAG, "startEmbeddedBackend($kind) failed", t)
            BackendStatus("error", t.message ?: "start failed", null)
        }
    }

    override suspend fun stopEmbeddedBackend(kind: BackendKind): BackendStatus =
        withContext(Dispatchers.IO) {
            try {
                val raw = when (kind) {
                    BackendKind.GSTPOP    -> { GstPopServiceBridge.stop(appContext); GstPopServiceBridge.queryStatus() }
                    BackendKind.MIGRATION -> { MigrationRuntimeServiceBridge.stop(appContext); MigrationRuntimeServiceBridge.queryStatus() }
                }
                parseStatus(raw)
            } catch (t: Throwable) {
                Log.e(TAG, "stopEmbeddedBackend($kind) failed", t)
                BackendStatus("error", t.message ?: "stop failed", null)
            }
        }

    override suspend fun backendStatus(kind: BackendKind): BackendStatus =
        withContext(Dispatchers.IO) {
            val raw = when (kind) {
                BackendKind.GSTPOP    -> GstPopServiceBridge.queryStatus()
                BackendKind.MIGRATION -> MigrationRuntimeServiceBridge.queryStatus()
            }
            parseStatus(raw)
        }

    override suspend fun graphCommand(action: String, params: JSONObject): JSONObject =
        withContext(Dispatchers.IO) {
            val payload = JSONObject().put(action, params)
            val raw = MainActivity.nativeGraphCommand(payload.toString())
            try { JSONObject(raw) } catch (_: Exception) {
                JSONObject().put("state", "error").put("message", raw)
            }
        }

    /** Internal — exposed for unit tests via reflection or @VisibleForTesting. */
    internal fun parseStatus(json: String?): BackendStatus {
        return StatusParser.parse(json)
    }

    companion object {
        private const val TAG = "JniRuntimeBridge"
    }
}
