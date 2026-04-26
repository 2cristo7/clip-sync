package com.clipsync.screenshot

import android.content.ContentResolver
import android.content.Context
import android.database.ContentObserver
import android.net.Uri
import android.os.Handler
import android.provider.MediaStore
import com.clipsync.util.L

/**
 * Watches [MediaStore.Images] for new screenshots and invokes [onScreenshot]
 * with the content URI and MIME type.
 *
 * Screenshots are identified by their `RELATIVE_PATH` containing "Screenshot"
 * (covers "Pictures/Screenshots/", "DCIM/Screenshots/", etc.) and being added
 * within the last [RECENT_THRESHOLD_SEC] seconds.
 *
 * A debounce window ([DEBOUNCE_MS]) prevents duplicate fires for the same
 * screenshot (MediaStore can notify multiple times per insertion).
 */
class ScreenshotObserver(
    private val context: Context,
    private val handler: Handler,
    private val onScreenshot: (uri: Uri, mime: String, bytes: ByteArray) -> Unit
) {

    private var registered = false
    private var lastSentId: Long = -1
    private var lastSentMs: Long = 0

    private val observer = object : ContentObserver(handler) {
        override fun onChange(selfChange: Boolean, uri: Uri?) {
            super.onChange(selfChange, uri)
            handler.removeCallbacks(queryRunnable)
            handler.postDelayed(queryRunnable, DEBOUNCE_MS)
        }
    }

    private val queryRunnable = Runnable { queryRecentScreenshot() }

    fun register() {
        if (registered) return
        context.contentResolver.registerContentObserver(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            true,
            observer
        )
        registered = true
        L.event(M, "ScreenshotObserver registered")
    }

    fun unregister() {
        if (!registered) return
        handler.removeCallbacks(queryRunnable)
        context.contentResolver.unregisterContentObserver(observer)
        registered = false
        L.event(M, "ScreenshotObserver unregistered")
    }

    private fun queryRecentScreenshot() {
        val now = System.currentTimeMillis()
        if (now - lastSentMs < DEBOUNCE_MS) return

        val cutoff = (now / 1000) - RECENT_THRESHOLD_SEC
        val resolver = context.contentResolver

        val projection = arrayOf(
            MediaStore.Images.Media._ID,
            MediaStore.Images.Media.MIME_TYPE,
            MediaStore.Images.Media.RELATIVE_PATH,
            MediaStore.Images.Media.DATE_ADDED,
            MediaStore.Images.Media.SIZE
        )
        val selection = "${MediaStore.Images.Media.DATE_ADDED} > ? AND " +
                "${MediaStore.Images.Media.RELATIVE_PATH} LIKE ?"
        val selectionArgs = arrayOf(cutoff.toString(), "%Screenshot%")
        val sortOrder = "${MediaStore.Images.Media.DATE_ADDED} DESC"

        resolver.query(
            MediaStore.Images.Media.EXTERNAL_CONTENT_URI,
            projection, selection, selectionArgs, sortOrder
        )?.use { cursor ->
            if (!cursor.moveToFirst()) return

            val idCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media._ID)
            val mimeCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.MIME_TYPE)
            val sizeCol = cursor.getColumnIndexOrThrow(MediaStore.Images.Media.SIZE)

            val id = cursor.getLong(idCol)
            val mime = cursor.getString(mimeCol) ?: "image/png"
            val size = cursor.getLong(sizeCol)

            if (id == lastSentId) return
            if (size > MAX_SIZE_BYTES) {
                L.warn(M, "Screenshot too large (${size / 1024}KB), skipping")
                return
            }

            val contentUri = Uri.withAppendedPath(
                MediaStore.Images.Media.EXTERNAL_CONTENT_URI, id.toString()
            )

            val bytes = readBytes(resolver, contentUri) ?: return
            lastSentId = id
            lastSentMs = now
            L.event(M, "New screenshot detected: id=$id mime=$mime size=${bytes.size}")
            onScreenshot(contentUri, mime, bytes)
        }
    }

    private fun readBytes(resolver: ContentResolver, uri: Uri): ByteArray? {
        return try {
            resolver.openInputStream(uri)?.use { it.readBytes() }
        } catch (e: Exception) {
            L.warn(M, "Failed to read screenshot: ${e.message}")
            null
        }
    }

    companion object {
        private const val M = "SS"
        private const val DEBOUNCE_MS = 800L
        private const val RECENT_THRESHOLD_SEC = 5L
        private const val MAX_SIZE_BYTES = 20L * 1024 * 1024
    }
}
