package com.clipsync.crypto

import java.security.MessageDigest
import java.security.cert.Certificate
import java.util.Base64 as JBase64

/**
 * Helpers for converting SPKI-SHA256 fingerprints between:
 *
 *  - base64url (no padding) — as advertised by mDNS TXT `fp` record (43 chars).
 *  - base64 (standard, with padding) — as required by OkHttp `CertificatePinner`
 *    in the `sha256/<b64>` format.
 *
 *  Also exposes a helper to compute the SPKI-SHA256 of an X.509 cert, used for
 *  TOFU (trust on first use) in manual mode where we don't know the fp yet.
 *
 *  Uses [java.util.Base64] (available since API 26, matches our minSdk) so this
 *  object stays testable on the JVM without Robolectric.
 */
object Fingerprint {

    /**
     * Convert a base64url-without-padding fp (e.g. `abc-_XYZ` 43 chars) to
     * a standard base64 string (with `+/` and padding) consumable by
     * OkHttp `CertificatePinner` via `sha256/<base64>`.
     */
    fun base64UrlToStandard(fpBase64Url: String): String {
        val raw = JBase64.getUrlDecoder().decode(fpBase64Url)
        return JBase64.getEncoder().encodeToString(raw)
    }

    /**
     * Compute SPKI-SHA256 of a certificate and return it as base64url
     * (no padding), matching the format used by the mac server mDNS TXT.
     */
    fun spkiSha256Base64Url(cert: Certificate): String {
        val spki = cert.publicKey.encoded
        val hash = MessageDigest.getInstance("SHA-256").digest(spki)
        return JBase64.getUrlEncoder().withoutPadding().encodeToString(hash)
    }

    /**
     * Build the OkHttp pin string `sha256/<standard-base64>` from a
     * base64url fp.
     */
    fun okHttpPin(fpBase64Url: String): String = "sha256/${base64UrlToStandard(fpBase64Url)}"
}
