package com.clipsync.notifications

import android.Manifest
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.util.Base64
import com.clipsync.util.L
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.clipsync.app.R
import com.clipsync.images.ImageCache
import com.clipsync.model.ClipPayload
import java.io.File

/**
 * Builds and posts notifications for frames received from the Mac.
 *
 * Channel `clipsync_incoming_v2` is IMPORTANCE_LOW with sound and vibration
 * disabled — incoming clips show in tray and status bar but never heads-up.
 *
 * Each notification carries a [PendingIntent] to [ApplyClipActivity] — a
 * translucent trampoline that writes the clip to the system clipboard and
 * finishes. We cannot write to the clipboard from a background service on
 * recent Android versions without the UI being on top, hence the trampoline.
 */
class IncomingClipNotifier(
    private val context: Context,
    private val imageCache: ImageCache = ImageCache(context)
) {

    fun ensureChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val nm = context.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            // Channel importance is immutable after creation; bump the id when
            // changing it so the new settings take effect on upgraded installs.
            nm.deleteNotificationChannel("clipsync_incoming")
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Incoming clips",
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = "New text or image received from the Mac"
                setSound(null, null)
                enableVibration(false)
                setShowBadge(true)
            }
            nm.createNotificationChannel(channel)
        }
    }

    /**
     * Post a notification for the given [payload]. If `POST_NOTIFICATIONS`
     * is revoked on Android 13+, this logs and returns silently.
     */
    fun notify(payload: ClipPayload) {
        ensureChannel()
        if (!hasPostNotifPermission()) {
            L.warn(M, "POST_NOTIFICATIONS revoked — skipping notification")
            return
        }
        val builder = when (payload.type) {
            "text" -> buildTextNotification(payload)
            "image" -> buildImageNotification(payload)
            "file" -> buildFileNotification(payload)
            else -> {
                L.warn(M, "Unknown payload type: ${payload.type}")
                return
            }
        }
        try {
            NotificationManagerCompat.from(context).notify(notifIdFor(payload), builder.build())
        } catch (sec: SecurityException) {
            L.warn(M, "SecurityException posting notification: ${sec.message}")
        }
    }

    private fun buildTextNotification(payload: ClipPayload): NotificationCompat.Builder {
        val text = decodeUtf8(payload.data)
        val preview = previewOf(text)
        val intent = ApplyClipActivity.textIntent(context, text, payload.nonce)
        val pi = PendingIntent.getActivity(
            context,
            pendingRequestCode(payload),
            intent,
            pendingFlags()
        )
        return NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Text from Mac")
            .setContentText(preview)
            .setStyle(NotificationCompat.BigTextStyle().bigText(text))
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setAutoCancel(true)
            .setContentIntent(pi)
    }

    private fun buildImageNotification(payload: ClipPayload): NotificationCompat.Builder {
        val bytes = Base64.decode(payload.data, Base64.DEFAULT)
        val ext = extensionForMime(payload.mime)
        // Write file first so we have the path for the "Save to gallery" action.
        val imageFile: File = imageCache.writeToFile(bytes, ext)
        val uri: Uri = androidx.core.content.FileProvider.getUriForFile(
            context, ImageCache.AUTHORITY, imageFile
        )
        val bitmap: Bitmap? = try {
            if (bytes.size > 5 * 1024 * 1024) {
                val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
                opts.inSampleSize = calculateInSampleSize(opts, 512, 512)
                opts.inJustDecodeBounds = false
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
            } else {
                BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
            }
        } catch (t: Throwable) {
            L.warn(M, "decodeByteArray failed: ${t.message}")
            null
        }
        val tapIntent = ApplyClipActivity.imageIntent(context, uri, payload.mime, payload.nonce)
        val tapPi = PendingIntent.getActivity(
            context,
            pendingRequestCode(payload),
            tapIntent,
            pendingFlags()
        )
        val saveIntent = Intent(context, SaveToGalleryReceiver::class.java).apply {
            action = SaveToGalleryReceiver.ACTION
            putExtra(SaveToGalleryReceiver.EXTRA_FILE_PATH, imageFile.absolutePath)
            putExtra(SaveToGalleryReceiver.EXTRA_MIME, payload.mime)
        }
        val savePi = PendingIntent.getBroadcast(
            context,
            imageFile.absolutePath.hashCode(),
            saveIntent,
            pendingFlags()
        )
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Image from Mac")
            .setContentText(payload.mime)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setAutoCancel(true)
            .setContentIntent(tapPi)
            .addAction(0, context.getString(R.string.action_save_to_gallery), savePi)
        try {
            if (bitmap != null) {
                builder.setLargeIcon(bitmap)
                builder.setStyle(
                    NotificationCompat.BigPictureStyle()
                        .bigPicture(bitmap)
                        .bigLargeIcon(null as Bitmap?)
                )
            }
        } finally {
            bitmap?.recycle()
        }
        return builder
    }

    private fun buildFileNotification(payload: ClipPayload): NotificationCompat.Builder {
        val bytes = Base64.decode(payload.data, Base64.DEFAULT)
        val fileName = payload.name ?: "clipsync_file"
        val ext = fileName.substringAfterLast('.', "bin")
        val cacheFile: File = imageCache.writeToFile(bytes, ext)

        val saveIntent = Intent(context, SaveToDownloadsReceiver::class.java).apply {
            action = SaveToDownloadsReceiver.ACTION
            putExtra(SaveToDownloadsReceiver.EXTRA_FILE_PATH, cacheFile.absolutePath)
            putExtra(SaveToDownloadsReceiver.EXTRA_MIME, payload.mime)
            putExtra(SaveToDownloadsReceiver.EXTRA_FILE_NAME, fileName)
        }
        val savePi = PendingIntent.getBroadcast(
            context,
            cacheFile.absolutePath.hashCode(),
            saveIntent,
            pendingFlags()
        )
        val sizeKB = bytes.size / 1024
        val sizeText = if (sizeKB >= 1024) "${"%.1f".format(sizeKB / 1024f)} MB" else "$sizeKB KB"

        return NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("File from Mac")
            .setContentText("$fileName ($sizeText)")
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .setAutoCancel(true)
            .setContentIntent(savePi)
            .addAction(0, context.getString(R.string.action_save_to_downloads), savePi)
    }

    private fun hasPostNotifPermission(): Boolean {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return true
        return ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.POST_NOTIFICATIONS
        ) == PackageManager.PERMISSION_GRANTED
    }

    private fun pendingFlags(): Int {
        var flags = PendingIntent.FLAG_UPDATE_CURRENT
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M) {
            flags = flags or PendingIntent.FLAG_IMMUTABLE
        }
        return flags
    }

    private fun pendingRequestCode(payload: ClipPayload): Int =
        (payload.nonce.hashCode() xor payload.ts.toInt())

    private fun notifIdFor(payload: ClipPayload): Int = when (payload.type) {
        "image" -> NOTIF_ID_IMAGE
        "file" -> NOTIF_ID_FILE
        else -> NOTIF_ID_TEXT
    }

    companion object {
        const val CHANNEL_ID = "clipsync_incoming_v2"
        private const val NOTIF_ID_TEXT = 4244   // text notifications replace each other
        const val NOTIF_ID_IMAGE = 4245          // image notifications replace each other (separate slot)
        const val NOTIF_ID_FILE = 4246           // file notifications replace each other (separate slot)
        private const val M = "Notif"
        private const val PREVIEW_MAX = 120

        internal fun previewOf(text: String): String {
            val single = text.replace(Regex("\\s+"), " ").trim()
            return if (single.length <= PREVIEW_MAX) single
            else single.substring(0, PREVIEW_MAX - 1) + "\u2026"
        }

        internal fun decodeUtf8(b64: String): String {
            return try {
                String(Base64.decode(b64, Base64.DEFAULT), Charsets.UTF_8)
            } catch (t: Throwable) {
                b64
            }
        }

        internal fun extensionForMime(mime: String): String = when (mime.lowercase()) {
            "image/png" -> "png"
            "image/jpeg", "image/jpg" -> "jpg"
            "image/webp" -> "webp"
            "image/gif" -> "gif"
            "image/tiff" -> "tiff"
            else -> "bin"
        }
    }

    private fun calculateInSampleSize(options: BitmapFactory.Options, reqWidth: Int, reqHeight: Int): Int {
        val height = options.outHeight
        val width = options.outWidth
        var inSampleSize = 1
        if (height > reqHeight || width > reqWidth) {
            val halfHeight = height / 2
            val halfWidth = width / 2
            while (halfHeight / inSampleSize >= reqHeight && halfWidth / inSampleSize >= reqWidth) {
                inSampleSize *= 2
            }
        }
        return inSampleSize
    }
}
