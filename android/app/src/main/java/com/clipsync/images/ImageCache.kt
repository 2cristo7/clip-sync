package com.clipsync.images

import android.content.Context
import android.net.Uri
import androidx.core.content.FileProvider
import com.clipsync.util.L
import java.io.File
import java.util.UUID

/**
 * Writes incoming image bytes under `cacheDir/clipsync/<uuid>.<ext>` and
 * exposes them to other apps via a `FileProvider` with authority
 * [AUTHORITY]. Stale files older than [DEFAULT_MAX_AGE_MS] are pruned on
 * demand.
 */
class ImageCache private constructor(
    private val context: Context?,
    private val rootProvider: () -> File
) {

    constructor(context: Context) : this(context, { File(context.cacheDir, DIR_NAME) })

    /** Test-only: inject a custom root folder without needing a Context. */
    internal constructor(rootProvider: () -> File) : this(null, rootProvider)

    /** Absolute directory under which incoming images live. */
    val dir: File
        get() {
            val f = rootProvider()
            if (!f.exists()) f.mkdirs()
            return f
        }

    /**
     * Persist [bytes] as `<uuid>.<ext>` and return a content `Uri` that
     * downstream apps can read via [FileProvider].
     */
    fun writeImage(bytes: ByteArray, ext: String): Uri {
        val ctx = requireNotNull(context) { "Context required for FileProvider" }
        val file = writeToFile(bytes, ext)
        return FileProvider.getUriForFile(ctx, AUTHORITY, file)
    }

    /** Package-private file writer — exposed for tests. */
    internal fun writeToFile(bytes: ByteArray, ext: String): File {
        val safeExt = ext.ifBlank { "bin" }
        val file = File(dir, "${UUID.randomUUID()}.$safeExt")
        file.outputStream().use { it.write(bytes) }
        enforceMaxSize()
        return file
    }

    /**
     * Evict oldest files until the cache directory is under [MAX_CACHE_SIZE_BYTES].
     */
    fun enforceMaxSize() {
        val root = rootProvider()
        if (!root.exists() || !root.isDirectory) return
        val files = root.listFiles()?.sortedBy { it.lastModified() } ?: return
        var totalSize = files.sumOf { it.length() }
        var evicted = 0
        for (file in files) {
            if (totalSize <= MAX_CACHE_SIZE_BYTES) break
            totalSize -= file.length()
            file.delete()
            evicted++
        }
        if (evicted > 0) L.event("ImageCache", "evicted $evicted files to stay under ${MAX_CACHE_SIZE_BYTES / 1024 / 1024}MB")
    }

    /**
     * Delete files older than [maxAgeMs] from the cache directory.
     * Returns the number of files deleted.
     */
    fun cleanupOlderThan(maxAgeMs: Long = DEFAULT_MAX_AGE_MS, now: Long = System.currentTimeMillis()): Int {
        val root = rootProvider()
        if (!root.exists() || !root.isDirectory) return 0
        var deleted = 0
        val cutoff = now - maxAgeMs
        root.listFiles()?.forEach { f ->
            if (f.isFile && f.lastModified() < cutoff) {
                if (f.delete()) deleted++
            }
        }
        return deleted
    }

    companion object {
        const val AUTHORITY = "com.clipsync.fileprovider"
        const val DIR_NAME = "clipsync"
        const val DEFAULT_MAX_AGE_MS = 24L * 60L * 60L * 1000L // 24h
        private const val MAX_CACHE_SIZE_BYTES = 200L * 1024 * 1024 // 200MB
    }
}
