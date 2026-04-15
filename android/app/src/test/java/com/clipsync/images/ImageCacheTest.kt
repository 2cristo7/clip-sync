package com.clipsync.images

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import java.io.File

/**
 * Unit tests for the non-Android parts of [ImageCache]. The Android
 * framework dependency ([FileProvider] resolution) is avoided by exercising
 * [ImageCache.writeToFile] and [ImageCache.cleanupOlderThan] directly with
 * a temp-folder backed provider.
 */
class ImageCacheTest {

    @get:Rule
    val tmp = TemporaryFolder()

    private fun newCache(root: File): ImageCache = ImageCache { root }

    @Test
    fun writeToFile_persists_bytes_with_extension() {
        val root = tmp.newFolder("clipsync")
        val cache = newCache(root)
        val bytes = byteArrayOf(0x89.toByte(), 0x50, 0x4E, 0x47)
        val file = cache.writeToFile(bytes, "png")
        assertTrue(file.exists())
        assertTrue(file.name.endsWith(".png"))
        assertArrayEquals(bytes, file.readBytes())
    }

    @Test
    fun writeToFile_uses_bin_for_blank_ext() {
        val root = tmp.newFolder("clipsync")
        val cache = newCache(root)
        val file = cache.writeToFile(byteArrayOf(1, 2, 3), "")
        assertTrue(file.name.endsWith(".bin"))
    }

    @Test
    fun cleanupOlderThan_removes_stale_files_only() {
        val root = tmp.newFolder("clipsync")
        val cache = newCache(root)
        val oldFile = cache.writeToFile(byteArrayOf(1), "png")
        val now = System.currentTimeMillis()
        // 48h in the past
        assertTrue(oldFile.setLastModified(now - 48L * 3600_000L))
        val fresh = cache.writeToFile(byteArrayOf(2), "png")

        val deleted = cache.cleanupOlderThan(maxAgeMs = 24L * 3600_000L, now = now)
        assertEquals(1, deleted)
        assertFalse(oldFile.exists())
        assertTrue(fresh.exists())
    }

    @Test
    fun cleanupOlderThan_on_missing_dir_returns_zero() {
        val root = File(tmp.root, "does-not-exist")
        val cache = newCache(root)
        assertEquals(0, cache.cleanupOlderThan())
    }
}
