package com.clipsync.clipboard

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

/**
 * Framework-light tests for [ClipboardWriter]. We avoid pulling in
 * Robolectric by checking invariants that don't cross the android.jar
 * boundary:
 *   - the clipboard label stays stable across refactors,
 *   - the public API surface stays in place (reflection probe).
 *
 * The actual Android `ClipData` construction is exercised on-device; the
 * functions are thin wrappers around `ClipData.newPlainText` /
 * `ClipData.Item(Uri)` that the platform tests cover.
 */
class ClipboardWriterTest {

    @Test
    fun label_constant_is_stable() {
        assertEquals("clipsync", ClipboardWriter.LABEL)
    }

    @Test
    fun exposes_expected_builders() {
        val methods = ClipboardWriter::class.java.declaredMethods.map { it.name }.toSet()
        assertNotNull(methods)
        // The public API we rely on from ApplyClipActivity + tests.
        listOf("buildTextClip", "buildImageClip", "writeText", "writeImage").forEach { name ->
            assert(methods.contains(name)) { "expected method $name on ClipboardWriter, got $methods" }
        }
    }
}
