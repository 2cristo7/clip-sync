package com.clipsync.crypto

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.security.MessageDigest
import java.util.Base64

class FingerprintTest {

    @Test
    fun base64UrlToStandard_matches_raw_bytes() {
        // Construct random SPKI-like 32-byte digest, encode both ways,
        // then make sure the round-trip via Fingerprint.base64UrlToStandard
        // yields the same raw bytes.
        val raw = MessageDigest.getInstance("SHA-256").digest("hello".toByteArray())
        val urlNoPad = Base64.getUrlEncoder().withoutPadding().encodeToString(raw)
        val standard = Fingerprint.base64UrlToStandard(urlNoPad)
        val decoded = Base64.getDecoder().decode(standard)
        assertEquals(raw.toList(), decoded.toList())
    }

    @Test
    fun base64UrlToStandard_translates_urlsafe_alphabet() {
        // Craft a 32-byte blob that contains `-` and `_` in its base64url form.
        val bytes = ByteArray(32) { i -> (i * 7 + 0xF0).toByte() }
        val urlNoPad = Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)
        // Pre-condition: the base64url encoding contains at least one URL-safe char.
        assertTrue("expected '-' or '_' in $urlNoPad",
            urlNoPad.contains('-') || urlNoPad.contains('_'))

        val standard = Fingerprint.base64UrlToStandard(urlNoPad)
        // Standard base64 must NOT contain `-` or `_` and should decode to the original.
        assertTrue(!standard.contains('-') && !standard.contains('_'))
        val decoded = Base64.getDecoder().decode(standard)
        assertEquals(bytes.toList(), decoded.toList())
    }

    @Test
    fun okHttpPin_has_expected_prefix() {
        val fp = Base64.getUrlEncoder().withoutPadding()
            .encodeToString(ByteArray(32) { 1 })
        val pin = Fingerprint.okHttpPin(fp)
        assertTrue(pin.startsWith("sha256/"))
        // Suffix is the standard base64 of 32 zero-ish bytes → 44 chars incl. padding.
        val suffix = pin.removePrefix("sha256/")
        val decoded = Base64.getDecoder().decode(suffix)
        assertEquals(32, decoded.size)
    }
}
