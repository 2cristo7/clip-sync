package com.clipsync.net

import com.clipsync.crypto.Fingerprint
import com.clipsync.model.ClipPayload
import okhttp3.CertificatePinner
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import java.security.cert.X509Certificate
import java.util.concurrent.TimeUnit
import javax.net.ssl.SSLContext
import javax.net.ssl.SSLSession
import javax.net.ssl.X509TrustManager

/**
 * Factory for the two flavours of [OkHttpClient] we need:
 *
 *  - [pinnedClient]: production client with `CertificatePinner` for the
 *    known server fingerprint. This is what normal connections go through.
 *
 *  - [tofuClient]: first-connect permissive client that doesn't validate the
 *    cert chain against system CAs (the server is self-signed), but
 *    captures the presented leaf cert's SPKI-SHA256 so the caller can pin
 *    it for subsequent connections. Returned alongside the client is a
 *    holder whose [FpHolder.fpBase64Url] is populated after the first
 *    successful handshake.
 *
 * Also provides [connectWebSocket] for `GET /ws` with Bearer auth.
 */
class ClipClient {

    fun pinnedClient(host: String, fpBase64Url: String): OkHttpClient {
        val pin = Fingerprint.okHttpPin(fpBase64Url)
        val pinner = CertificatePinner.Builder()
            .add(host, pin)
            .build()
        return baseBuilder()
            .certificatePinner(pinner)
            .hostnameVerifier { _, _ -> true } // self-signed cert, CN may not match IP
            .build()
    }

    class FpHolder {
        @Volatile var fpBase64Url: String? = null
    }

    fun tofuClient(): Pair<OkHttpClient, FpHolder> {
        val holder = FpHolder()
        val trustAll = object : X509TrustManager {
            override fun checkClientTrusted(chain: Array<out X509Certificate>?, authType: String?) {}
            override fun checkServerTrusted(chain: Array<out X509Certificate>?, authType: String?) {
                val leaf = chain?.firstOrNull() ?: return
                holder.fpBase64Url = Fingerprint.spkiSha256Base64Url(leaf)
            }
            override fun getAcceptedIssuers(): Array<X509Certificate> = emptyArray()
        }
        val sslContext = SSLContext.getInstance("TLS")
        sslContext.init(null, arrayOf(trustAll), java.security.SecureRandom())
        val client = baseBuilder()
            .sslSocketFactory(sslContext.socketFactory, trustAll)
            .hostnameVerifier { _: String, _: SSLSession -> true }
            .build()
        return client to holder
    }

    fun connectWebSocket(
        client: OkHttpClient,
        host: String,
        port: Int,
        token: String,
        onFrame: (ClipPayload) -> Unit,
        onStatus: (WsStatus) -> Unit
    ): WebSocket {
        val req = Request.Builder()
            .url("https://$host:$port/ws")
            .header("Authorization", "Bearer $token")
            .build()
        val listener = object : WebSocketListener() {
            override fun onOpen(webSocket: WebSocket, response: Response) {
                onStatus(WsStatus.Open)
            }

            override fun onMessage(webSocket: WebSocket, text: String) {
                val payload = try {
                    ClipPayload.fromJson(text)
                } catch (t: Throwable) {
                    onStatus(WsStatus.Error("bad frame: ${t.message}"))
                    return
                }
                onFrame(payload)
            }

            override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                webSocket.close(1000, null)
                onStatus(WsStatus.Closed(code, reason))
            }

            override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                onStatus(WsStatus.Error(t.message ?: "ws failure"))
            }
        }
        return client.newWebSocket(req, listener)
    }

    sealed class WsStatus {
        data object Open : WsStatus()
        data class Closed(val code: Int, val reason: String) : WsStatus()
        data class Error(val message: String) : WsStatus()
    }

    private fun baseBuilder(): OkHttpClient.Builder = OkHttpClient.Builder()
        .connectTimeout(10, TimeUnit.SECONDS)
        .readTimeout(0, TimeUnit.SECONDS) // WebSocket: no read timeout
        .writeTimeout(10, TimeUnit.SECONDS)
        .pingInterval(20, TimeUnit.SECONDS)
        .retryOnConnectionFailure(true)
}
