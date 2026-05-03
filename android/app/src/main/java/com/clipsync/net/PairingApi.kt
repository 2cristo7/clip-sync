package com.clipsync.net

import com.clipsync.crypto.Fingerprint
import okhttp3.OkHttpClient
import okhttp3.Request
import java.util.concurrent.TimeUnit
import org.json.JSONObject

/**
 * Performs the `GET /pair?code=XXXXXX` handshake against the mac server.
 *
 * Two entry points depending on trust context:
 *
 *  - [pairWithKnownFp]: the caller already knows the server's SPKI-SHA256
 *    fingerprint (e.g. learned via mDNS TXT). We pin the connection before
 *    sending the request — classic pinning.
 *
 *  - [pairWithTofu]: TOFU path for manual-IP mode. We build a one-shot
 *    permissive client that records the cert presented during the TLS
 *    handshake, then let the caller persist that fp for future pinning.
 */
class PairingApi(
    private val clientFactory: ClipClient = ClipClient()
) {

    data class PairingResponse(
        val token: String,
        val sig: String,
        /** Base64-encoded pairing-secret used to HMAC-sign `POST /inject`. */
        val secret: String
    )

    data class TofuPairingResponse(
        val token: String,
        val sig: String,
        val secret: String,
        val fpBase64Url: String
    )

    fun pairWithKnownFp(host: String, port: Int, code: String, fpBase64Url: String): PairingResponse {
        val client = clientFactory.pinnedClient(host, fpBase64Url)
        return requestPair(client, host, port, code)
    }

    fun pairWithTofu(host: String, port: Int, code: String): TofuPairingResponse {
        val (client, fpHolder) = clientFactory.tofuClient()
        val resp = requestPair(client, host, port, code)
        val fp = fpHolder.fpBase64Url
            ?: throw IllegalStateException("TOFU client did not capture server cert fingerprint")
        return TofuPairingResponse(resp.token, resp.sig, resp.secret, fp)
    }

    private fun requestPair(client: OkHttpClient, host: String, port: Int, code: String): PairingResponse {
        val url = "https://$host:$port/pair?code=$code"
        val req = Request.Builder().url(url).get().build()
        val configured = client.newBuilder()
            .callTimeout(15, TimeUnit.SECONDS)
            .build()
        configured.newCall(req).execute().use { resp ->
            val body = resp.body?.string() ?: ""
            if (!resp.isSuccessful) {
                throw PairingException("HTTP ${resp.code}: $body")
            }
            val json = JSONObject(body)
            val token = json.optString("token", "")
            val sig = json.optString("sig", "")
            val secret = json.optString("secret", "")
            if (token.isEmpty() || sig.isEmpty() || secret.isEmpty()) {
                throw PairingException("Malformed /pair response: $body")
            }
            return PairingResponse(token, sig, secret)
        }
    }

    /**
     * Returns [Result.success] with `true` if the server at [host]:[port] responds to a ping.
     * Returns [Result.failure] with the underlying exception on network or TLS errors.
     * Uses the stored fingerprint for TLS pinning. Timeout: 3 seconds.
     */
    fun ping(host: String, port: Int, fp: String): Result<Boolean> = runCatching {
        val client = clientFactory.pinnedClient(host, fp)
            .newBuilder()
            .callTimeout(3, TimeUnit.SECONDS)
            .connectTimeout(3, TimeUnit.SECONDS)
            .build()
        val req = Request.Builder().url("https://$host:$port/health").get().build()
        client.newCall(req).execute().use { true }
    }

    class PairingException(message: String) : Exception(message)

    companion object {
        // Re-exposed for convenience / tests.
        fun pinFor(fpBase64Url: String): String = Fingerprint.okHttpPin(fpBase64Url)
    }
}
