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
import android.util.Log
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import com.clipsync.app.R
import com.clipsync.images.ImageCache
import com.clipsync.model.ClipPayload

/**
 * Builds and posts notifications for frames received from the Mac.
 *
 * Channel `clipsync_incoming` is IMPORTANCE_DEFAULT with sound disabled so
 * incoming clips are visible but non-intrusive.
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
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Incoming clips",
                NotificationManager.IMPORTANCE_DEFAULT
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
            Log.w(TAG, "POST_NOTIFICATIONS revoked — skipping notification")
            return
        }
        val builder = when (payload.type) {
            "text" -> buildTextNotification(payload)
            "image" -> buildImageNotification(payload)
            else -> {
                Log.w(TAG, "Unknown payload type: ${payload.type}")
                return
            }
        }
        try {
            NotificationManagerCompat.from(context).notify(notifIdFor(payload), builder.build())
        } catch (sec: SecurityException) {
            Log.w(TAG, "SecurityException posting notification: ${sec.message}")
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
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .setContentIntent(pi)
    }

    private fun buildImageNotification(payload: ClipPayload): NotificationCompat.Builder {
        val bytes = Base64.decode(payload.data, Base64.DEFAULT)
        val ext = extensionForMime(payload.mime)
        val uri: Uri = imageCache.writeImage(bytes, ext)
        val bitmap: Bitmap? = try {
            BitmapFactory.decodeByteArray(bytes, 0, bytes.size)
        } catch (t: Throwable) {
            Log.w(TAG, "decodeByteArray failed: ${t.message}")
            null
        }
        val intent = ApplyClipActivity.imageIntent(context, uri, payload.mime, payload.nonce)
        val pi = PendingIntent.getActivity(
            context,
            pendingRequestCode(payload),
            intent,
            pendingFlags()
        )
        val builder = NotificationCompat.Builder(context, CHANNEL_ID)
            .setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("Image from Mac")
            .setContentText(payload.mime)
            .setPriority(NotificationCompat.PRIORITY_DEFAULT)
            .setAutoCancel(true)
            .setContentIntent(pi)
        if (bitmap != null) {
            builder.setLargeIcon(bitmap)
            builder.setStyle(
                NotificationCompat.BigPictureStyle()
                    .bigPicture(bitmap)
                    .bigLargeIcon(null as Bitmap?)
            )
        }
        return builder
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

    private fun notifIdFor(payload: ClipPayload): Int =
        (payload.nonce.hashCode() and 0x7fffffff) or 1

    companion object {
        const val CHANNEL_ID = "clipsync_incoming"
        private const val TAG = "ClipSync"
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
            else -> "bin"
        }
    }
}
