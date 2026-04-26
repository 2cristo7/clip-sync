package com.clipsync.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.Context
import android.content.Intent
import com.clipsync.net.NetworkChangeObserver
import android.content.ClipboardManager
import android.os.Build
import android.os.IBinder
import com.clipsync.util.L
import androidx.core.app.NotificationCompat
import com.clipsync.app.R
import com.clipsync.clipboard.ClipboardWriter
import com.clipsync.images.ImageCache
import android.util.Base64
import com.clipsync.model.ClipPayload
import com.clipsync.model.ClipPayloadBuilder
import com.clipsync.net.ClipClient
import com.clipsync.notifications.IncomingClipNotifier
import com.clipsync.overlay.ClipOverlayManager
import com.clipsync.overlay.ClipSender
import com.clipsync.overlay.SendClipActivity
import com.clipsync.screenshot.ScreenshotObserver
import com.clipsync.shizuku.ShizukuClipboardManager
import com.clipsync.storage.Prefs
import okhttp3.WebSocket
import kotlin.math.min

/**
 * Long-running foreground service keeping the `/ws` WebSocket alive.
 *
 * Restart policy:
 *  - exponential backoff capped at [MAX_BACKOFF_MS].
 *  - immediate reconnect whenever the default network changes.
 *
 * Incoming frames are forwarded to [IncomingClipNotifier].
 * Outbound sends are triggered manually via the overlay FAB → [SendClipActivity].
 */
class ClipForegroundService : Service() {

    private lateinit var prefs: Prefs
    private lateinit var client: ClipClient
    private lateinit var imageCache: ImageCache
    private lateinit var incomingNotifier: IncomingClipNotifier
    private var ws: WebSocket? = null
    @Volatile private var wsGeneration = 0   // incremented on each connect(); stale callbacks are ignored
    private var backoffMs: Long = INITIAL_BACKOFF_MS
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())
    private val reconnectRunnable = Runnable { connect() }

    private var networkObserver: NetworkChangeObserver? = null
    private var overlayManager: ClipOverlayManager? = null
    private var clipboardManager: ClipboardManager? = null
    private var screenshotObserver: ScreenshotObserver? = null
    private var clipListenerRegistered = false
    private var clipListenerRegisteredAt = 0L   // grace period to suppress registration-fire
    private var lastAutoSendMs = 0L

    // Shizuku clipboard (Tier 1)
    private var shizukuManager: ShizukuClipboardManager? = null
    private var lastShizukuHash = 0
    private var shizukuHashSeeded = false        // true after first successful hash read
    private val shizukuPollRunnable = object : Runnable {
        override fun run() {
            pollViaShizuku()
            handler.postDelayed(this, SHIZUKU_POLL_MS)
        }
    }

    private val clipChangedListener = ClipboardManager.OnPrimaryClipChangedListener {
        L.verbose(M, "clipListener fired")

        // Some devices fire the listener immediately on registration.
        // Skip events within 1.5 s of registering to avoid sending stale clipboard.
        val now = System.currentTimeMillis()
        if (now - clipListenerRegisteredAt < 1_500) {
            L.verbose(M, "skip: registration grace period")
            return@OnPrimaryClipChangedListener
        }

        if (!prefs.autoSendEnabled) {
            L.verbose(M, "skip: autoSendEnabled=false")
            return@OnPrimaryClipChangedListener
        }
        if (!prefs.syncEnabled) {
            L.verbose(M, "skip: syncEnabled=false")
            return@OnPrimaryClipChangedListener
        }

        val echoAge = now - ClipboardWriter.lastMacWriteMs
        if (echoAge < 2_000) {
            L.verbose(M, "skip: echo suppression ${echoAge}ms")
            return@OnPrimaryClipChangedListener
        }

        val debounceAge = now - lastAutoSendMs
        if (debounceAge < 1_000) {
            L.verbose(M, "skip: debounce ${debounceAge}ms")
            return@OnPrimaryClipChangedListener
        }

        lastAutoSendMs = now
        L.event(M, "auto-send launched")
        startActivity(SendClipActivity.intent(this).putExtra(SendClipActivity.EXTRA_AUTO_SEND, true))
    }

    override fun onBind(intent: Intent?): IBinder? = null

    override fun onCreate() {
        super.onCreate()
        prefs = Prefs(this)
        client = ClipClient()
        imageCache = ImageCache(this)
        incomingNotifier = IncomingClipNotifier(this, imageCache)
        ensureNotificationChannel()
        incomingNotifier.ensureChannel()
        try {
            val pruned = imageCache.cleanupOlderThan()
            if (pruned > 0) L.event(M, "pruned $pruned stale cached images")
        } catch (t: Throwable) {
            L.warn(M, "ImageCache cleanup failed: ${t.message}")
        }
        startForeground(NOTIF_ID, buildNotification("Connecting..."))
        networkObserver = NetworkChangeObserver(this) {
            backoffMs = INITIAL_BACKOFF_MS
            handler.removeCallbacks(reconnectRunnable)
            handler.post(reconnectRunnable)
        }
        networkObserver?.register()
        overlayManager = ClipOverlayManager(this)
        clipboardManager = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        shizukuManager = ShizukuClipboardManager(this).also { mgr ->
            mgr.onStateChanged = { state ->
                L.event(M, "shizuku state=$state")
                handler.post { onShizukuStateChanged(state) }
            }
            mgr.initialize()
        }
        screenshotObserver = ScreenshotObserver(this, handler) { _, mime, bytes ->
            if (!prefs.autoSendEnabled || !prefs.syncEnabled) return@ScreenshotObserver
            L.event(M, "screenshot auto-send mime=$mime bytes=${bytes.size}")
            overlayManager?.showUploadIndicator()
            val payload = ClipPayloadBuilder.image(mime, bytes)
            sendPayloadToMac(payload)
        }
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == ACTION_UPDATE_OVERLAY) {
            handler.post {
                if (prefs.overlayEnabled) overlayManager?.showFab() else overlayManager?.dismiss()
            }
            return START_STICKY
        }
        if (intent?.action == ACTION_STOP_SHIZUKU) {
            handler.post {
                stopShizukuPolling()
                shizukuManager?.destroy()
                shizukuManager = null
                L.action(M, "shizuku listener stopped")
            }
            return START_STICKY
        }
        if (!prefs.hasPairing()) {
            L.warn(M, "No pairing stored, stopping service")
            stopSelf()
            return START_NOT_STICKY
        }
        connect()
        return START_STICKY
    }

    override fun onDestroy() {
        screenshotObserver?.unregister()
        screenshotObserver = null
        networkObserver?.unregister()
        networkObserver = null
        overlayManager?.destroy()
        overlayManager = null
        unregisterClipListener()
        stopShizukuPolling()
        shizukuManager?.destroy()
        shizukuManager = null
        handler.removeCallbacks(reconnectRunnable)
        wsGeneration++  // invalidate any in-flight callbacks
        ws?.cancel()
        ws = null
        super.onDestroy()
    }

    private fun connect() {
        val token = prefs.token ?: return
        val fp = prefs.fp ?: return
        val host = prefs.host ?: return
        val port = prefs.port
        // Bump generation so callbacks from any previous WebSocket are ignored.
        // This prevents the cancel-triggers-onFailure-triggers-reconnect loop and
        // also prevents duplicate connections from multiple calls to connect().
        wsGeneration++
        val gen = wsGeneration
        ws?.cancel()
        ws = null
        updateNotification("Connecting to $host...")
        val okClient = client.pinnedClient(host, fp)
        ws = client.connectWebSocket(okClient, host, port, token,
            onFrame = { payload ->
                if (gen != wsGeneration) return@connectWebSocket
                onFrame(payload)
            },
            onStatus = { status ->
                if (gen != wsGeneration) return@connectWebSocket  // stale callback
                when (status) {
                    is ClipClient.WsStatus.Open -> {
                        backoffMs = INITIAL_BACKOFF_MS
                        L.event("WS", "connected host=$host")
                        updateNotification("Connected ($host)")
                        handler.post {
                            if (prefs.overlayEnabled) overlayManager?.showFab()
                            registerClipListener()
                            if (shizukuManager?.isAvailable() == true) startShizukuPolling()
                            screenshotObserver?.register()
                        }
                    }
                    is ClipClient.WsStatus.Closed -> {
                        L.event("WS", "closed host=$host code=${status.code}")
                        updateNotification("Disconnected")
                        handler.post {
                            overlayManager?.dismiss()
                            unregisterClipListener()
                            stopShizukuPolling()
                            screenshotObserver?.unregister()
                        }
                        scheduleReconnect()
                    }
                    is ClipClient.WsStatus.Error -> {
                        L.warn("WS", "error host=$host msg=${status.message}")
                        updateNotification("Error: ${status.message}")
                        handler.post {
                            overlayManager?.dismiss()
                            unregisterClipListener()
                            stopShizukuPolling()
                            screenshotObserver?.unregister()
                        }
                        scheduleReconnect()
                    }
                }
            })
    }

    private fun onFrame(payload: ClipPayload) {
        L.event(M, "frame type=${payload.type} mime=${payload.mime} bytes=${payload.data.length}")

        // Tier 1: Write text directly via Shizuku (no ApplyClipActivity trampoline)
        if (payload.type == "text" && shizukuManager?.isAvailable() == true) {
            val text = IncomingClipNotifier.decodeUtf8(payload.data)
            ClipboardWriter.lastMacWriteMs = System.currentTimeMillis()
            shizukuManager?.setClipboardText(text)
            lastShizukuHash = text.hashCode()
            L.event(M, "clipboard write via shizuku chars=${text.length}")
            // Still show notification for visual feedback
            try { incomingNotifier.notify(payload) } catch (t: Throwable) {
                L.warn(M, "notify failed: ${t.message}")
            }
            return
        }

        // Images: always use ApplyClipActivity trampoline. Shizuku setClipboardUri
        // fails silently because UID 2000 (shell) cannot grant FileProvider URI
        // permissions on the system clipboard.
        if (payload.type == "image") {
            broadcastLoading(show = true)
            try {
                val bytes = Base64.decode(payload.data, Base64.DEFAULT)
                val ext = IncomingClipNotifier.extensionForMime(payload.mime)
                val uri = imageCache.writeImage(bytes, ext)
                ClipboardWriter.lastMacWriteMs = System.currentTimeMillis()
                lastShizukuHash = uri.toString().hashCode()
                startActivity(
                    com.clipsync.notifications.ApplyClipActivity.imageIntent(
                        this, uri, payload.mime, payload.nonce
                    )
                )
                L.event(M, "launching ApplyClipActivity bytes=${bytes.size}")
            } catch (t: Throwable) {
                L.warn(M, "Image clipboard write failed: ${t.message}")
                broadcastLoading(show = false, success = false)
            }
            // Still show notification for visual feedback
            try { incomingNotifier.notify(payload) } catch (t: Throwable) {
                L.warn(M, "notify failed: ${t.message}")
            }
            return
        }

        // Tier 2/3: Notification with ApplyClipActivity (no Shizuku)
        try {
            incomingNotifier.notify(payload)
        } catch (t: Throwable) {
            L.warn(M, "notify failed: ${t.message}")
        }
    }

    private fun registerClipListener() {
        if (clipListenerRegistered) return
        clipListenerRegisteredAt = System.currentTimeMillis()
        clipboardManager?.addPrimaryClipChangedListener(clipChangedListener)
        clipListenerRegistered = true
        L.event(M, "clipboard listener registered")
    }

    private fun unregisterClipListener() {
        if (!clipListenerRegistered) return
        clipboardManager?.removePrimaryClipChangedListener(clipChangedListener)
        clipListenerRegistered = false
        L.event(M, "clipboard listener unregistered")
    }

    private fun scheduleReconnect() {
        handler.removeCallbacks(reconnectRunnable)
        val delay = backoffMs
        backoffMs = min(backoffMs * 2, MAX_BACKOFF_MS)
        L.event(M, "reconnect in ${delay}ms")
        handler.postDelayed(reconnectRunnable, delay)
    }

    private fun ensureNotificationChannel() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            val channel = NotificationChannel(
                CHANNEL_ID,
                getString(R.string.notif_channel_name),
                NotificationManager.IMPORTANCE_LOW
            ).apply {
                description = getString(R.string.notif_channel_desc)
                setShowBadge(false)
            }
            nm.createNotificationChannel(channel)
        }
    }

    private fun buildNotification(text: String): Notification {
        return NotificationCompat.Builder(this, CHANNEL_ID)
            .setContentTitle("ClipSync")
            .setContentText(text)
            .setSmallIcon(R.drawable.ic_notification)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setPriority(NotificationCompat.PRIORITY_LOW)
            .build()
    }

    private fun updateNotification(text: String) {
        val nm = getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
        nm.notify(NOTIF_ID, buildNotification(text))
    }

    // --- Shizuku polling (Tier 1) ---

    private fun pollViaShizuku() {
        val mgr = shizukuManager ?: return
        if (!mgr.isAvailable()) return

        val hash = mgr.getClipboardHash()
        if (hash == 0) return

        // First successful read: just seed the hash, don't send.
        // This prevents sending whatever is already in the clipboard on startup.
        if (!shizukuHashSeeded) {
            lastShizukuHash = hash
            shizukuHashSeeded = true
            L.verbose(M, "shizuku hash seeded=$hash")
            return
        }

        if (hash == lastShizukuHash) return

        // Clipboard actually changed — apply filters before sending.
        if (!prefs.autoSendEnabled || !prefs.syncEnabled || !prefs.hasPairing()) return

        val now = System.currentTimeMillis()
        if (now - ClipboardWriter.lastMacWriteMs < 2_000) return  // echo suppression
        if (now - lastAutoSendMs < 1_000) return                  // debounce

        lastShizukuHash = hash
        lastAutoSendMs = now

        val mime = mgr.getClipboardMime()
        if (mime != null && mime.startsWith("text")) {
            val text = mgr.getClipboardText() ?: return
            L.event(M, "shizuku auto-send text chars=${text.length}")
            sendTextToMac(text)
        } else if (mime != null && mime.startsWith("image")) {
            // Image clips: use the trampoline Activity which has proper clipboard
            // access and gets temporary URI read permission from the system.
            // Direct URI reading from a Service fails because the clipboard URI
            // grant was given to the Shizuku process, not our app process.
            L.event(M, "shizuku image clip detected mime=$mime")
            broadcastLoading(show = true)
            startActivity(
                SendClipActivity.intent(this)
                    .putExtra(SendClipActivity.EXTRA_AUTO_SEND, true)
            )
        } else {
            L.verbose(M, "shizuku unsupported clip mime=$mime")
        }
    }

    private fun sendTextToMac(text: String) {
        sendPayloadToMac(ClipPayloadBuilder.text(text))
    }

    private fun sendPayloadToMac(payload: ClipPayload) {
        val host = prefs.host ?: return
        val port = prefs.port
        val token = prefs.token ?: return
        val secret = prefs.pairingSecret ?: return
        val fp = prefs.fp ?: return
        val sender = ClipSender(client)
        Thread {
            val result = sender.send(host, port, token, secret, fp, payload)
            L.event(M, "auto-send result=$result type=${payload.type}")
            val ok = result is ClipSender.Result.Ok
            handler.post { overlayManager?.hideUploadIndicator(ok) }
        }.start()
    }

    private fun broadcastLoading(show: Boolean, success: Boolean = true) {
        val action = if (show) ClipOverlayManager.ACTION_SHOW_LOADING
                     else ClipOverlayManager.ACTION_HIDE_LOADING
        sendBroadcast(Intent(action).apply {
            setPackage(packageName)
            if (!show) putExtra(ClipOverlayManager.EXTRA_LOADING_SUCCESS, success)
        })
    }

    private fun startShizukuPolling() {
        handler.removeCallbacks(shizukuPollRunnable)
        shizukuHashSeeded = false
        lastShizukuHash = 0
        handler.post(shizukuPollRunnable)
        L.event(M, "shizuku polling started")
    }

    private fun stopShizukuPolling() {
        handler.removeCallbacks(shizukuPollRunnable)
        shizukuHashSeeded = false
        L.event(M, "shizuku polling stopped")
    }

    private fun onShizukuStateChanged(state: ShizukuClipboardManager.State) {
        when (state) {
            ShizukuClipboardManager.State.READY -> {
                if (ws != null) startShizukuPolling()
            }
            else -> stopShizukuPolling()
        }
    }

    companion object {
        private const val M = "SVC"
        private const val CHANNEL_ID = "clipsync_sync"
        private const val NOTIF_ID = 4242
        private const val INITIAL_BACKOFF_MS = 1_000L
        private const val MAX_BACKOFF_MS = 30_000L
        private const val SHIZUKU_POLL_MS = 500L
        const val ACTION_UPDATE_OVERLAY = "com.clipsync.UPDATE_OVERLAY"
        const val ACTION_STOP_SHIZUKU = "com.clipsync.STOP_SHIZUKU"

        fun start(context: Context) {
            val i = Intent(context, ClipForegroundService::class.java)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(i)
            } else {
                context.startService(i)
            }
        }

        fun stop(context: Context) {
            context.stopService(Intent(context, ClipForegroundService::class.java))
        }

        fun updateOverlay(context: Context) {
            val i = Intent(context, ClipForegroundService::class.java).apply {
                action = ACTION_UPDATE_OVERLAY
            }
            try {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                    context.startForegroundService(i)
                } else {
                    context.startService(i)
                }
            } catch (_: Exception) {
                // Service not running — pref already saved, FAB will respect it on next start
            }
        }
    }
}
