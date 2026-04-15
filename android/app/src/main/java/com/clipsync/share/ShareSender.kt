package com.clipsync.share

import com.clipsync.crypto.HmacSigner
import com.clipsync.model.ClipPayload
import com.clipsync.net.ClipClient
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import java.util.Base64
import java.util.UUID

/**
 * Posts a shared clip (text or image bytes) to the mac server's `/inject`
 * endpoint. Builds a [ClipPayload], base64-encodes binary content, signs the
 * body with the pairing-secret (HMAC-SHA256), and authenticates with the
 * bearer token issued during pairing.
 *
 * Uses [ClipClient.pinnedClient] so TLS is pinned to the server's SPKI.
 */
class ShareSender(
    private val clientFactory: ClipClient = ClipClient(),
    private val clockMs: () -> Long = { System.currentTimeMillis() }
) {

    sealed class Result {
        data object Ok : Result()
        data class Failed(val reason: String) : Result()
    }

    fun sendText(
        host: String,
        port: Int,
        token: String,
        pairingSecretB64: String,
        fpBase64Url: String,
        text: String
    ): Result {
        val client = clientFactory.pinnedClient(host, fpBase64Url)
        val payload = buildTextPayload(text)
        return post(client, host, port, token, pairingSecretB64, payload)
    }

    fun sendImage(
        host: String,
        port: Int,
        token: String,
        pairingSecretB64: String,
        fpBase64Url: String,
        mime: String,
        bytes: ByteArray
    ): Result {
        val client = clientFactory.pinnedClient(host, fpBase64Url)
        val payload = buildImagePayload(mime, bytes)
        return post(client, host, port, token, pairingSecretB64, payload)
    }

    /** Exposed for tests — lets the test inject its own OkHttpClient and URL. */
    internal fun sendWithClient(
        client: OkHttpClient,
        url: String,
        token: String,
        pairingSecretB64: String,
        payload: ClipPayload
    ): Result = post(client, url, token, pairingSecretB64, payload)

    private fun post(
        client: OkHttpClient,
        host: String,
        port: Int,
        token: String,
        pairingSecretB64: String,
        payload: ClipPayload
    ): Result = post(client, "https://$host:$port/inject", token, pairingSecretB64, payload)

    private fun post(
        client: OkHttpClient,
        url: String,
        token: String,
        pairingSecretB64: String,
        payload: ClipPayload
    ): Result {
        val secret = try {
            Base64.getDecoder().decode(pairingSecretB64)
        } catch (t: Throwable) {
            return Result.Failed("invalid secret")
        }
        val body = payload.toJson()
        val ts = clockMs() / 1000L
        val sigHeader = HmacSigner.signatureHeader(secret, ts, body)

        val req = Request.Builder()
            .url(url)
            .header("Authorization", "Bearer $token")
            .header("X-ClipSync-Signature", sigHeader)
            .header("X-ClipSync-Source", "android-share")
            .post(body.toRequestBody(JSON))
            .build()

        return try {
            client.newCall(req).execute().use { resp ->
                if (resp.isSuccessful) Result.Ok
                else Result.Failed("HTTP ${resp.code}")
            }
        } catch (t: Throwable) {
            Result.Failed(t.message ?: "network error")
        }
    }

    fun buildTextPayload(text: String): ClipPayload {
        val b64 = Base64.getEncoder().encodeToString(text.toByteArray(Charsets.UTF_8))
        return ClipPayload(
            type = "text",
            mime = "text/plain",
            data = b64,
            ts = clockMs() / 1000L,
            nonce = UUID.randomUUID().toString()
        )
    }

    fun buildImagePayload(mime: String, bytes: ByteArray): ClipPayload {
        val b64 = Base64.getEncoder().encodeToString(bytes)
        return ClipPayload(
            type = "image",
            mime = mime,
            data = b64,
            ts = clockMs() / 1000L,
            nonce = UUID.randomUUID().toString()
        )
    }

    companion object {
        private val JSON = "application/json; charset=utf-8".toMediaType()
        const val MAX_IMAGE_BYTES: Int = 20 * 1024 * 1024
    }
}
