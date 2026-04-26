package com.clipsync.notifications

import android.content.BroadcastReceiver
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.media.MediaScannerConnection
import android.os.Build
import android.os.Environment
import android.os.Handler
import android.os.Looper
import android.provider.MediaStore
import android.widget.Toast
import androidx.core.app.NotificationManagerCompat
import com.clipsync.app.R
import com.clipsync.util.L
import java.io.File

/**
 * Saves an incoming image (stored in ImageCache) to the device gallery.
 *
 * Triggered by the "Save to gallery" action button on image notifications.
 * On Android 10+ uses the scoped MediaStore API (no extra permission needed).
 * On Android 9 and below uses the legacy external storage path and triggers
 * a media scanner rescan so the file appears in gallery apps immediately.
 */
class SaveToGalleryReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        val filePath = intent.getStringExtra(EXTRA_FILE_PATH) ?: return
        val mime = intent.getStringExtra(EXTRA_MIME) ?: "image/png"

        val file = File(filePath)
        if (!file.exists()) {
            L.warn(M, "source file not found: $filePath")
            showToast(context, context.getString(R.string.save_to_gallery_failed))
            return
        }

        val saved = saveToGallery(context, file, mime)
        NotificationManagerCompat.from(context).cancel(IncomingClipNotifier.NOTIF_ID_IMAGE)
        showToast(
            context,
            if (saved) context.getString(R.string.saved_to_gallery)
            else context.getString(R.string.save_to_gallery_failed)
        )
    }

    private fun saveToGallery(context: Context, source: File, mime: String): Boolean {
        val filename = "clipsync_${System.currentTimeMillis()}.${extensionFor(mime)}"
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val values = ContentValues().apply {
                    put(MediaStore.Images.Media.DISPLAY_NAME, filename)
                    put(MediaStore.Images.Media.MIME_TYPE, mime)
                    put(MediaStore.Images.Media.RELATIVE_PATH, Environment.DIRECTORY_PICTURES + "/ClipSync")
                    put(MediaStore.Images.Media.IS_PENDING, 1)
                }
                val collection = MediaStore.Images.Media.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
                val itemUri = context.contentResolver.insert(collection, values) ?: return false
                context.contentResolver.openOutputStream(itemUri)?.use { source.inputStream().copyTo(it) }
                values.clear()
                values.put(MediaStore.Images.Media.IS_PENDING, 0)
                context.contentResolver.update(itemUri, values, null, null)
                L.event(M, "saved via MediaStore: $filename")
                true
            } else {
                val dir = File(
                    @Suppress("DEPRECATION")
                    Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_PICTURES),
                    "ClipSync"
                )
                dir.mkdirs()
                val dest = File(dir, filename)
                source.copyTo(dest, overwrite = true)
                MediaScannerConnection.scanFile(context, arrayOf(dest.absolutePath), arrayOf(mime), null)
                L.event(M, "saved via legacy storage: ${dest.absolutePath}")
                true
            }
        } catch (t: Throwable) {
            L.error(M, "saveToGallery failed: ${t.message}")
            false
        }
    }

    private fun showToast(context: Context, message: String) {
        Handler(Looper.getMainLooper()).post {
            Toast.makeText(context.applicationContext, message, Toast.LENGTH_SHORT).show()
        }
    }

    companion object {
        const val ACTION = "com.clipsync.action.SAVE_TO_GALLERY"
        const val EXTRA_FILE_PATH = "file_path"
        const val EXTRA_MIME = "mime"
        private const val M = "SaveGallery"

        private fun extensionFor(mime: String): String = when (mime.lowercase()) {
            "image/png" -> "png"
            "image/jpeg", "image/jpg" -> "jpg"
            "image/webp" -> "webp"
            "image/gif" -> "gif"
            else -> "png"
        }
    }
}
