package com.clipsync.crypto

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import javax.crypto.Mac
import javax.crypto.spec.SecretKeySpec

class HmacSignerTest {

    @Test
    fun hex_is_lowercase_and_correct_length() {
        val bytes = byteArrayOf(0x0a, 0x1b, 0x2c.toByte(), 0xff.toByte())
        val hex = HmacSigner.hex(bytes)
        assertEquals("0a1b2cff", hex)
    }

    @Test
    fun hmacSha256_matches_jdk_implementation() {
        val secret = "s3cret".toByteArray()
        val data = "hello".toByteArray()
        val expected = Mac.getInstance("HmacSHA256").apply {
            init(SecretKeySpec(secret, "HmacSHA256"))
        }.doFinal(data)
        val actual = HmacSigner.hmacSha256(secret, data)
        assertEquals(expected.toList(), actual.toList())
    }

    @Test
    fun signatureHeader_matches_mac_server_format() {
        // Format expected by the mac server (Phase 4):
        //   t=<ts>, v1=<hex of HMAC-SHA256(secret, "<ts>.<body>")>
        val secret = "pairing-secret-bytes".toByteArray()
        val ts = 1_712_000_000L
        val body = """{"type":"text","data":"aGk="}"""
        val header = HmacSigner.signatureHeader(secret, ts, body)

        assertTrue(header.startsWith("t=$ts, v1="))

        val expectedHex = HmacSigner.hex(
            HmacSigner.hmacSha256(secret, "$ts.$body".toByteArray())
        )
        assertEquals("t=$ts, v1=$expectedHex", header)
    }
}
