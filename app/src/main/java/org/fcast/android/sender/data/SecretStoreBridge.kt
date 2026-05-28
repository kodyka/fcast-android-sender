package org.fcast.android.sender.data

/**
 * Static JNI bridge — the Rust side calls into the *currently installed*
 * SecretStore by going through this class.
 *
 * AppGraph installs the production store at startup:
 *
 *     SecretStoreBridge.install((application as FcastApp).graph.secretStore)
 */
object SecretStoreBridge {

    @Volatile private var store: SecretStore? = null

    fun install(s: SecretStore) {
        store = s
    }

    /** Called from native code via JNI. Returns null when alias is unknown. */
    @JvmStatic
    @Suppress("unused")
    fun jniGet(alias: String): ByteArray? = store?.get(alias)

    /** Called from native code via JNI. */
    @JvmStatic
    @Suppress("unused")
    fun jniPut(alias: String, value: ByteArray) {
        store?.put(alias, value)
    }

    /** Called from native code via JNI. */
    @JvmStatic
    @Suppress("unused")
    fun jniDelete(alias: String) {
        store?.delete(alias)
    }
}
