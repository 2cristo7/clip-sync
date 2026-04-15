package com.clipsync.crypto

import java.util.Locale
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

/**
 * Builds the `X-ClipSync-Signature` header for `POST /inject`.
 *
 * Spec (mac server, Phase 4):
 *   header value = "t=<unix_ts>, v1=<hex>"
 *   v1 = HMAC-SHA256(pairing-secret, "<ts>.<body>") hex-encoded (lowercase)
 *
 * The `pairing-secret` is delivered via the `secret` field of the `/pair`
 * response (Phase 7) and persisted in EncryptedSharedPreferences
 * (`Prefs.pairingSecret`, base64). Callers decode it and pass the raw bytes
 * to [signatureHeader].
 */
object HmacSigner {

    /**
     * Produce the full header value `t=<ts>, v1=<hex>` for a JSON body.
     */
    fun signatureHeader(secret: ByteArray, timestampSec: Long, body: String): String {
        val mac = hex(hmacSha256(secret, "$timestampSec.$body".toByteArray(Charsets.UTF_8)))
        return "t=$timestampSec, v1=$mac"
    }

    fun hmacSha256(secret: ByteArray, data: ByteArray): ByteArray {
        val mac = Mac.getInstance("HmacSHA256")
        mac.init(SecretKeySpec(secret, "HmacSHA256"))
        return mac.doFinal(data)
    }

    fun hex(bytes: ByteArray): String {
        val sb = StringBuilder(bytes.size * 2)
        for (b in bytes) sb.append(String.format(Locale.ROOT, "%02x", b.toInt() and 0xff))
        return sb.toString()
    }
}
