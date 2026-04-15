package com.clipsync.discovery

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class NsdDiscoveryTest {

    @Test
    fun parseTxt_returns_empty_on_null() {
        val parsed = NsdDiscovery.parseTxt(null)
        assertTrue(parsed.isEmpty())
    }

    @Test
    fun parseTxt_decodes_utf8_values() {
        val attrs: Map<String, ByteArray?> = mapOf(
            "fp" to "abcDEF123_-".toByteArray(Charsets.UTF_8),
            "version" to "0.1.0".toByteArray(Charsets.UTF_8),
            "name" to "Mac Mini".toByteArray(Charsets.UTF_8),
            "bogus" to null
        )
        val parsed = NsdDiscovery.parseTxt(attrs)
        assertEquals("abcDEF123_-", parsed["fp"])
        assertEquals("0.1.0", parsed["version"])
        assertEquals("Mac Mini", parsed["name"])
        assertTrue(!parsed.containsKey("bogus"))
    }
}
