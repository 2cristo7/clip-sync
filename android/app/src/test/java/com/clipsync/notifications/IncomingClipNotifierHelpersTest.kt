package com.clipsync.notifications

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for the pure helpers on [IncomingClipNotifier] — kept
 * framework-free so they run on the JVM without Robolectric.
 */
class IncomingClipNotifierHelpersTest {

    @Test
    fun previewOf_short_text_returned_as_is() {
        val out = IncomingClipNotifier.previewOf("hello world")
        assertEquals("hello world", out)
    }

    @Test
    fun previewOf_collapses_whitespace() {
        val out = IncomingClipNotifier.previewOf("a   b\n\tc ")
        assertEquals("a b c", out)
    }

    @Test
    fun previewOf_truncates_long_text_with_ellipsis() {
        val long = "x".repeat(500)
        val out = IncomingClipNotifier.previewOf(long)
        assertEquals(120, out.length)
        assertTrue(out.endsWith("\u2026"))
    }

    @Test
    fun extensionForMime_maps_known_types() {
        assertEquals("png", IncomingClipNotifier.extensionForMime("image/png"))
        assertEquals("jpg", IncomingClipNotifier.extensionForMime("image/jpeg"))
        assertEquals("jpg", IncomingClipNotifier.extensionForMime("image/jpg"))
        assertEquals("webp", IncomingClipNotifier.extensionForMime("image/webp"))
        assertEquals("gif", IncomingClipNotifier.extensionForMime("image/gif"))
    }

    @Test
    fun extensionForMime_defaults_to_bin() {
        assertEquals("bin", IncomingClipNotifier.extensionForMime("application/octet-stream"))
        assertEquals("bin", IncomingClipNotifier.extensionForMime(""))
    }

    @Test
    fun extensionForMime_is_case_insensitive() {
        assertEquals("png", IncomingClipNotifier.extensionForMime("Image/PNG"))
    }
}
