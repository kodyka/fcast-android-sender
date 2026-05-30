package org.fcast.android.sender.data

/**
 * Opaque alias -> bytes mapping. Implementations:
 *  - AndroidSecretStore       (production; EncryptedSharedPreferences)
 *  - InMemorySecretStore      (tests)
 *
 * Bytes are returned as a Java byte[], the caller is responsible for
 * scrubbing them after use (use kotlin.with `… .fill(0)`).
 */
interface SecretStore {
    fun get(alias: String): ByteArray?
    fun put(alias: String, value: ByteArray)
    fun delete(alias: String)
}
