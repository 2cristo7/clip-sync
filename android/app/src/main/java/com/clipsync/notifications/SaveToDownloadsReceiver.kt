package com.clipsync.notifications

import android.content.BroadcastReceiver
import android.content.ContentValues
import android.content.Context
import android.content.Intent
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

class SaveToDownloadsReceiver : BroadcastReceiver() {

    override fun onReceive(context: Context, intent: Intent) {
        val filePath = intent.getStringExtra(EXTRA_FILE_PATH) ?: return
        val mime = intent.getStringExtra(EXTRA_MIME) ?: "application/octet-stream"
        val fileName = intent.getStringExtra(EXTRA_FILE_NAME) ?: "clipsync_file"

        val file = File(filePath)
        if (!file.exists()) {
            L.warn(M, "source file not found: $filePath")
            showToast(context, context.getString(R.string.save_to_downloads_failed))
            return
        }

        val saved = saveToDownloads(context, file, mime, fileName)
        NotificationManagerCompat.from(context).cancel(IncomingClipNotifier.NOTIF_ID_FILE)
        showToast(
            context,
            if (saved) context.getString(R.string.saved_to_downloads)
            else context.getString(R.string.save_to_downloads_failed)
        )
    }

    private fun saveToDownloads(context: Context, source: File, mime: String, fileName: String): Boolean {
        return try {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                val values = ContentValues().apply {
                    put(MediaStore.Downloads.DISPLAY_NAME, fileName)
                    put(MediaStore.Downloads.MIME_TYPE, mime)
                    put(MediaStore.Downloads.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/ClipSync")
                    put(MediaStore.Downloads.IS_PENDING, 1)
                }
                val collection = MediaStore.Downloads.getContentUri(MediaStore.VOLUME_EXTERNAL_PRIMARY)
                val itemUri = context.contentResolver.insert(collection, values) ?: return false
                context.contentResolver.openOutputStream(itemUri)?.use { source.inputStream().copyTo(it) }
                values.clear()
                values.put(MediaStore.Downloads.IS_PENDING, 0)
                context.contentResolver.update(itemUri, values, null, null)
                L.event(M, "saved via MediaStore: $fileName")
                true
            } else {
                @Suppress("DEPRECATION")
                val dir = File(
                    Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS),
                    "ClipSync"
                )
                dir.mkdirs()
                val dest = File(dir, fileName)
                source.copyTo(dest, overwrite = true)
                L.event(M, "saved via legacy storage: ${dest.absolutePath}")
                true
            }
        } catch (t: Throwable) {
            L.error(M, "saveToDownloads failed: ${t.message}")
            false
        }
    }

    private fun showToast(context: Context, message: String) {
        Handler(Looper.getMainLooper()).post {
            Toast.makeText(context.applicationContext, message, Toast.LENGTH_SHORT).show()
        }
    }

    companion object {
        const val ACTION = "com.clipsync.action.SAVE_TO_DOWNLOADS"
        const val EXTRA_FILE_PATH = "file_path"
        const val EXTRA_MIME = "mime"
        const val EXTRA_FILE_NAME = "file_name"
        private const val M = "SaveDownloads"
    }
}
