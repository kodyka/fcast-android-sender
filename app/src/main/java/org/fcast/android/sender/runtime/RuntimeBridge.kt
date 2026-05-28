package org.fcast.android.sender.runtime

import org.json.JSONObject

/** Which embedded backend the user has chosen. Mirrors Slint's MediaBackendKind. */
enum class BackendKind {
    GSTPOP,
    MIGRATION,
    ;

    companion object {
        /** Slint reports the enum as a string; map both Slint-style and Rust-style. */
        fun fromString(s: String?): BackendKind = when (s?.lowercase()) {
            "gstpop", "gst-pop", "gst_pop" -> GSTPOP
            "migration", null               -> MIGRATION
            else -> throw IllegalArgumentException("Unknown backend kind: $s")
        }
    }
}

/** Result of a backend lifecycle call. */
data class BackendStatus(
    val state: String,            // "running" | "stopped" | "starting" | "error"
    val message: String?,         // human-readable message; null when state == running
    val extra: JSONObject?        // backend-specific extension; nullable
) {
    val isRunning: Boolean get() = state == "running"
    val isError:   Boolean get() = state == "error"
}

/**
 * Typed gateway between the Android shell and the embedded Rust runtimes.
 *
 * Implementations:
 *  - JniRuntimeBridge      — production; calls into JNI service bridges.
 *  - FakeRuntimeBridge     — for tests (see step 10).
 *
 * All methods suspend so callers can switch dispatchers freely. The JNI impl
 * dispatches to Dispatchers.IO internally before crossing the JNI boundary,
 * since service-bridge calls block waiting on the embedded daemon.
 */
interface RuntimeBridge {
    suspend fun startEmbeddedBackend(kind: BackendKind, configJson: String): BackendStatus
    suspend fun stopEmbeddedBackend(kind: BackendKind): BackendStatus
    suspend fun backendStatus(kind: BackendKind): BackendStatus

    /** Generic graph command (replaces direct `nativeGraphCommand` calls). */
    suspend fun graphCommand(action: String, params: JSONObject): JSONObject
}
