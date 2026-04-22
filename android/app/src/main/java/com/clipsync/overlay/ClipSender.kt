package com.clipsync.overlay

import com.clipsync.crypto.HmacSigner
import com.clipsync.model.ClipPayload
import com.clipsync.net.ClipClient
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import android.util.Base64

/**
 * Posts a [ClipPayload] to the mac server's `POST /inject` endpoint.
 *
 * Authenticates with the Bearer token issued during pairing and signs the
 * body with HMAC-SHA256 using the pairing-secret. Uses
 * [ClipClient.pinnedClient] so TLS is pinned to the server's SPKI.
 */
class ClipSender(
    private val clientFactory: ClipClient = ClipClient(),
    private val clockMs: () -> Long = { System.currentTimeMillis() }
) {

    sealed class Result {
        data object Ok : Result()
        data class Failed(val reason: String) : Result()
    }

    fun send(
        host: String,
        port: Int,
        token: String,
        pairingSecretB64: String,
        fpBase64Url: String,
        payload: ClipPayload
    ): Result {
        val client = clientFactory.pinnedClient(host, fpBase64Url)
        return post(client, "https://$host:$port/inject", token, pairingSecretB64, payload)
    }

    private fun post(
        client: OkHttpClient,
        url: String,
        token: String,
        pairingSecretB64: String,
        payload: ClipPayload
    ): Result {
        val secret = try {
            Base64.decode(pairingSecretB64, Base64.DEFAULT)
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
            .header("X-ClipSync-Source", "android-fab")
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

    companion object {
        private val JSON = "application/json; charset=utf-8".toMediaType()
    }
}
