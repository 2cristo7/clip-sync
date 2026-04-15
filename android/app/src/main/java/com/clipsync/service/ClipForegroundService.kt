package com.clipsync.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.Service
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkRequest
import android.os.Build
import android.os.IBinder
import android.util.Log
import androidx.core.app.NotificationCompat
import com.clipsync.app.R
import com.clipsync.images.ImageCache
import com.clipsync.model.ClipPayload
import com.clipsync.net.ClipClient
import com.clipsync.notifications.IncomingClipNotifier
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
 * Incoming frames are logged via `Log.i("ClipSync", ...)` per the Phase 5
 * scope; actual clipboard writes land in Phase 6.
 */
class ClipForegroundService : Service() {

    private lateinit var prefs: Prefs
    private lateinit var client: ClipClient
    private lateinit var imageCache: ImageCache
    private lateinit var incomingNotifier: IncomingClipNotifier
    private var ws: WebSocket? = null
    private var backoffMs: Long = INITIAL_BACKOFF_MS
    private val handler = android.os.Handler(android.os.Looper.getMainLooper())
    private val reconnectRunnable = Runnable { connect() }

    private var networkCallback: ConnectivityManager.NetworkCallback? = null

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
            if (pruned > 0) Log.i(TAG, "Pruned $pruned stale cached image(s)")
        } catch (t: Throwable) {
            Log.w(TAG, "ImageCache cleanup failed: ${t.message}")
        }
        startForeground(NOTIF_ID, buildNotification("Connecting..."))
        registerNetworkCallback()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!prefs.hasPairing()) {
            Log.w(TAG, "No pairing stored, stopping service")
            stopSelf()
            return START_NOT_STICKY
        }
        connect()
        return START_STICKY
    }

    override fun onDestroy() {
        unregisterNetworkCallback()
        handler.removeCallbacks(reconnectRunnable)
        ws?.cancel()
        ws = null
        super.onDestroy()
    }

    private fun connect() {
        val token = prefs.token ?: return
        val fp = prefs.fp ?: return
        val host = prefs.host ?: return
        val port = prefs.port
        updateNotification("Connecting to $host...")
        val okClient = client.pinnedClient(host, fp)
        ws = client.connectWebSocket(okClient, host, port, token,
            onFrame = { payload -> onFrame(payload) },
            onStatus = { status ->
                when (status) {
                    is ClipClient.WsStatus.Open -> {
                        backoffMs = INITIAL_BACKOFF_MS
                        updateNotification("Connected ($host)")
                    }
                    is ClipClient.WsStatus.Closed -> {
                        updateNotification("Disconnected")
                        scheduleReconnect()
                    }
                    is ClipClient.WsStatus.Error -> {
                        updateNotification("Error: ${status.message}")
                        scheduleReconnect()
                    }
                }
            })
    }

    private fun onFrame(payload: ClipPayload) {
        Log.i(TAG, "frame type=${payload.type} mime=${payload.mime} bytes=${payload.data.length} ts=${payload.ts}")
        try {
            incomingNotifier.notify(payload)
        } catch (t: Throwable) {
            Log.w(TAG, "notify failed: ${t.message}")
        }
    }

    private fun scheduleReconnect() {
        handler.removeCallbacks(reconnectRunnable)
        val delay = backoffMs
        backoffMs = min(backoffMs * 2, MAX_BACKOFF_MS)
        Log.i(TAG, "Reconnect in ${delay}ms")
        handler.postDelayed(reconnectRunnable, delay)
    }

    private fun registerNetworkCallback() {
        val cm = getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        val req = NetworkRequest.Builder().build()
        val cb = object : ConnectivityManager.NetworkCallback() {
            override fun onAvailable(network: Network) {
                Log.i(TAG, "Network available, reconnecting")
                backoffMs = INITIAL_BACKOFF_MS
                handler.removeCallbacks(reconnectRunnable)
                handler.post(reconnectRunnable)
            }
            override fun onLost(network: Network) {
                Log.i(TAG, "Network lost")
            }
        }
        cm.registerNetworkCallback(req, cb)
        networkCallback = cb
    }

    private fun unregisterNetworkCallback() {
        val cm = getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager
        networkCallback?.let {
            try { cm.unregisterNetworkCallback(it) } catch (_: Throwable) {}
        }
        networkCallback = null
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

    companion object {
        private const val TAG = "ClipSync"
        private const val CHANNEL_ID = "clipsync_sync"
        private const val NOTIF_ID = 4242
        private const val INITIAL_BACKOFF_MS = 1_000L
        private const val MAX_BACKOFF_MS = 30_000L

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
    }
}
