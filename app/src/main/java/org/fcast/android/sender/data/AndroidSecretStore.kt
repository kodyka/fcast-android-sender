package org.fcast.android.sender.data

import android.content.Context
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * EncryptedSharedPreferences-backed secret store.
 *
 * Storage location: SharedPreferences("fcast_secrets") under the app's
 * private storage, encrypted by a MasterKey held in the Android Keystore.
 *
 * NOTE: requires androidx.security:security-crypto:1.1.0-alpha06+ in the
 *       app/build.gradle dependencies block.
 */
class AndroidSecretStore(context: Context) : SecretStore {

    private val masterKey: MasterKey = MasterKey.Builder(context)
        .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
        .build()

    private val prefs = EncryptedSharedPreferences.create(
        context,
        SHARED_PREFS_NAME,
        masterKey,
        EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
        EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM,
    )

    override fun get(alias: String): ByteArray? {
        val encoded = prefs.getString(alias, null) ?: return null
        return android.util.Base64.decode(encoded, android.util.Base64.NO_WRAP)
    }

    override fun put(alias: String, value: ByteArray) {
        val encoded = android.util.Base64.encodeToString(value, android.util.Base64.NO_WRAP)
        prefs.edit().putString(alias, encoded).apply()
    }

    override fun delete(alias: String) {
        prefs.edit().remove(alias).apply()
    }

    companion object {
        const val SHARED_PREFS_NAME = "fcast_secrets"
    }
}
