package com.clipsync.storage

import android.content.Context
import android.content.SharedPreferences
import androidx.security.crypto.EncryptedSharedPreferences
import androidx.security.crypto.MasterKey

/**
 * EncryptedSharedPreferences wrapper holding connection state:
 * token, fp (server SPKI-SHA256 base64url), host, port, mode (auto|manual).
 */
class Prefs(context: Context) {
    private val prefs: SharedPreferences by lazy {
        val masterKey = MasterKey.Builder(context)
            .setKeyScheme(MasterKey.KeyScheme.AES256_GCM)
            .build()
        EncryptedSharedPreferences.create(
            context,
            FILE,
            masterKey,
            EncryptedSharedPreferences.PrefKeyEncryptionScheme.AES256_SIV,
            EncryptedSharedPreferences.PrefValueEncryptionScheme.AES256_GCM
        )
    }

    var token: String?
        get() = prefs.getString(K_TOKEN, null)
        set(v) { prefs.edit().putString(K_TOKEN, v).apply() }

    var fp: String?
        get() = prefs.getString(K_FP, null)
        set(v) { prefs.edit().putString(K_FP, v).apply() }

    var host: String?
        get() = prefs.getString(K_HOST, null)
        set(v) { prefs.edit().putString(K_HOST, v).apply() }

    var port: Int
        get() = prefs.getInt(K_PORT, 7010)
        set(v) { prefs.edit().putInt(K_PORT, v).apply() }

    var mode: String
        get() = prefs.getString(K_MODE, MODE_AUTO) ?: MODE_AUTO
        set(v) { prefs.edit().putString(K_MODE, v).apply() }

    /**
     * Base64-encoded pairing-secret shared by the Mac during `/pair`. Used to
     * HMAC-sign `POST /inject` requests from the share target (Phase 7).
     */
    var pairingSecret: String?
        get() = prefs.getString(K_SECRET, null)
        set(v) { prefs.edit().putString(K_SECRET, v).apply() }

    fun clearPairing() {
        prefs.edit()
            .remove(K_TOKEN)
            .remove(K_FP)
            .remove(K_SECRET)
            .apply()
    }

    fun hasPairing(): Boolean = !token.isNullOrEmpty() && !fp.isNullOrEmpty() && !host.isNullOrEmpty()

    companion object {
        private const val FILE = "clipsync_prefs"
        private const val K_TOKEN = "token"
        private const val K_FP = "fp"
        private const val K_HOST = "host"
        private const val K_PORT = "port"
        private const val K_MODE = "mode"
        private const val K_SECRET = "pairing_secret"
        const val MODE_AUTO = "auto"
        const val MODE_MANUAL = "manual"
    }
}
