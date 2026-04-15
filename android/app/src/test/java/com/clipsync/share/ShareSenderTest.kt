package com.clipsync.share

import com.clipsync.crypto.HmacSigner
import com.clipsync.model.ClipPayload
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.Base64

class ShareSenderTest {

    private lateinit var server: MockWebServer
    private val secretBytes = "0123456789abcdef0123456789abcdef".toByteArray()
    private val secretB64: String = Base64.getEncoder().encodeToString(secretBytes)
    private val token = "test-token"
    private val fixedTsMs = 1_712_000_000_000L

    private fun sender() = ShareSender(clockMs = { fixedTsMs })

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    private fun url(): String = server.url("/inject").toString()

    @Test
    fun post_text_sends_correct_body_and_headers() {
        server.enqueue(MockResponse().setResponseCode(200).setBody("""{"ok":true,"nonce":"x"}"""))

        val s = sender()
        val payload = s.buildTextPayload("hello mac")
        val result = s.sendWithClient(
            client = OkHttpClient(),
            url = url(),
            token = token,
            pairingSecretB64 = secretB64,
            payload = payload
        )
        assertTrue("expected Ok but got $result", result is ShareSender.Result.Ok)

        val recorded = server.takeRequest()
        assertEquals("POST", recorded.method)
        assertEquals("/inject", recorded.path)
        assertEquals("Bearer $token", recorded.getHeader("Authorization"))

        val sigHeader = recorded.getHeader("X-ClipSync-Signature")
        assertNotNull("signature header missing", sigHeader)
        assertTrue(
            "signature header format",
            sigHeader!!.matches(Regex("""t=\d+, v1=[0-9a-f]+"""))
        )

        // Validate HMAC matches exactly what HmacSigner would produce for this body at this ts.
        val bodyStr = recorded.body.readUtf8()
        val ts = fixedTsMs / 1000L
        val expected = HmacSigner.signatureHeader(secretBytes, ts, bodyStr)
        assertEquals(expected, sigHeader)

        // Body is valid ClipPayload JSON with text + base64-encoded content.
        val json = JSONObject(bodyStr)
        assertEquals("text", json.getString("type"))
        assertEquals("text/plain", json.getString("mime"))
        val data = json.getString("data")
        assertEquals("hello mac", String(Base64.getDecoder().decode(data), Charsets.UTF_8))
    }

    @Test
    fun post_image_payload_encodes_base64_and_mime() {
        server.enqueue(MockResponse().setResponseCode(200).setBody("""{"ok":true,"nonce":"y"}"""))

        val bytes = byteArrayOf(0x00, 0x01, 0x02, 0x7f, 0x55.toByte())
        val s = sender()
        val payload = s.buildImagePayload("image/png", bytes)
        val result = s.sendWithClient(
            client = OkHttpClient(),
            url = url(),
            token = token,
            pairingSecretB64 = secretB64,
            payload = payload
        )
        assertTrue(result is ShareSender.Result.Ok)

        val recorded = server.takeRequest()
        val json = JSONObject(recorded.body.readUtf8())
        assertEquals("image", json.getString("type"))
        assertEquals("image/png", json.getString("mime"))
        val data = Base64.getDecoder().decode(json.getString("data"))
        assertEquals(bytes.toList(), data.toList())
    }

    @Test
    fun server_401_returns_failed_result() {
        server.enqueue(MockResponse().setResponseCode(401).setBody("unauthorized"))

        val s = sender()
        val payload = s.buildTextPayload("x")
        val result = s.sendWithClient(
            client = OkHttpClient(),
            url = url(),
            token = "bogus",
            pairingSecretB64 = secretB64,
            payload = payload
        )
        assertTrue("expected Failed", result is ShareSender.Result.Failed)
        val failed = result as ShareSender.Result.Failed
        assertTrue(failed.reason.contains("401"))
    }

    @Test
    fun invalid_secret_short_circuits_to_failed_without_hitting_network() {
        // Do not enqueue: if we hit the network, the test hangs/fails.
        val s = sender()
        val payload = s.buildTextPayload("x")
        val result = s.sendWithClient(
            client = OkHttpClient(),
            url = url(),
            token = token,
            pairingSecretB64 = "@@@not-base64@@@",
            payload = payload
        )
        assertTrue(result is ShareSender.Result.Failed)
    }
}
