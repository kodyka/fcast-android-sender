package org.fcast.android.sender.runtime

import org.json.JSONObject

internal object StatusParser {
    /**
     * Parses a backend status JSON blob into [BackendStatus]. Unparseable
     * input is mapped to `state="error"` with the literal exception
     * message or error explanation; the caller does not see exceptions.
     */
    fun parse(json: String?): BackendStatus {
        if (json.isNullOrEmpty()) return BackendStatus("error", "empty status", null)
        return try {
            val obj = JSONObject(json)
            val state   = obj.optString("state", "unknown")
            val message = if (obj.has("message") && !obj.isNull("message")) {
                obj.getString("message").takeIf { it.isNotEmpty() }
            } else null
            val extra   = obj.optJSONObject("extra")
            BackendStatus(state, message, extra)
        } catch (e: Exception) {
            BackendStatus("error", "unparseable: $json", null)
        }
    }
}
