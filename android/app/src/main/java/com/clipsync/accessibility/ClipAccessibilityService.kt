package com.clipsync.accessibility

import android.accessibilityservice.AccessibilityService
import android.content.ClipboardManager
import android.content.Intent
import android.view.accessibility.AccessibilityEvent
import com.clipsync.clipboard.ClipboardWriter
import com.clipsync.model.ClipPayloadBuilder
import com.clipsync.overlay.ClipSender
import com.clipsync.overlay.SendClipActivity
import com.clipsync.storage.Prefs
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Accessibility service that listens for clipboard changes and automatically
 * sends new content to the paired Mac.
 *
 * AccessibilityService has elevated system privileges that allow reading
 * ClipboardManager.getPrimaryClip() from background — unlike regular services
 * which get null on Android 10+.
 *
 * Echo prevention: ClipboardWriter labels every clip it writes with
 * [ClipboardWriter.LABEL] ("clipsync"). The listener skips any clip with
 * that label so Mac→Android writes are never echoed back.
 *
 * The service is optional and user-controlled via [Prefs.autoSendEnabled].
 * It must also be enabled in Settings → Accessibility by the user (one-time).
 */
class ClipAccessibilityService : AccessibilityService() {

    private lateinit var clipboardManager: ClipboardManager
    private lateinit var prefs: Prefs
    private val sender = ClipSender()
    private val scope = CoroutineScope(Dispatchers.Main + SupervisorJob())

    // Debounce: track last sent content hash + timestamp to avoid duplicate sends.
    private var lastSentHash: Int = 0
    private var lastSentTimeMs: Long = 0

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        handleClipChange()
    }

    override fun onServiceConnected() {
        clipboardManager = getSystemService(ClipboardManager::class.java)
        prefs = Prefs(applicationContext)
        clipboardManager.addPrimaryClipChangedListener(clipListener)
    }

    private fun handleClipChange() {
        // Guard: auto-send disabled by user preference
        if (!prefs.autoSendEnabled) return

        // Guard: sync paused globally
        if (!prefs.syncEnabled) return

        // Guard: no pairing established
        if (!prefs.hasPairing() || prefs.pairingSecret.isNullOrEmpty()) return

        val clip = clipboardManager.primaryClip ?: return
        if (clip.itemCount == 0) return

        // Echo prevention: ClipboardWriter.writeText/writeImage sets label = "clipsync".
        // Any clip with that label was written by us (incoming from Mac) — skip it.
        val label = clip.description?.label?.toString() ?: ""
        if (label == ClipboardWriter.LABEL) return

        val item = clip.getItemAt(0)
        val mimeType = clip.description?.getMimeType(0) ?: ""

        val host = prefs.host ?: return
        val port = prefs.port
        val token = prefs.token ?: return
        val secret = prefs.pairingSecret ?: return
        val fp = prefs.fp ?: return

        when {
            mimeType.startsWith("text/") || item.text != null -> {
                val text = item.coerceToText(this)?.toString()
                if (text.isNullOrBlank()) return

                // Debounce: skip if same content sent in the last 2 seconds
                val hash = text.hashCode()
                val now = System.currentTimeMillis()
                if (hash == lastSentHash && now - lastSentTimeMs < DEBOUNCE_MS) return
                lastSentHash = hash
                lastSentTimeMs = now

                val payload = ClipPayloadBuilder.text(text)
                scope.launch(Dispatchers.IO) {
                    val result = sender.send(host, port, token, secret, fp, payload)
                    broadcastResult(result is ClipSender.Result.Ok)
                }
            }
            mimeType.startsWith("image/") -> {
                val uri = item.uri ?: return
                scope.launch(Dispatchers.IO) {
                    try {
                        val stream = contentResolver.openInputStream(uri) ?: return@launch
                        val bytes = stream.use { it.readBytes() }
                        if (bytes.size > ClipPayloadBuilder.MAX_IMAGE_BYTES) return@launch
                        val mime = contentResolver.getType(uri) ?: mimeType

                        // Debounce for images: hash first 256 bytes
                        val hash = bytes.take(256).hashCode()
                        val now = System.currentTimeMillis()
                        if (hash == lastSentHash && now - lastSentTimeMs < DEBOUNCE_MS) return@launch
                        lastSentHash = hash
                        lastSentTimeMs = now

                        val payload = ClipPayloadBuilder.image(mime, bytes)
                        val result = sender.send(host, port, token, secret, fp, payload)
                        broadcastResult(result is ClipSender.Result.Ok)
                    } catch (_: Throwable) {
                        // Silently ignore — image may not be readable from accessibility context
                    }
                }
            }
        }
    }

    private suspend fun broadcastResult(success: Boolean) {
        withContext(Dispatchers.Main) {
            sendBroadcast(Intent(SendClipActivity.ACTION_SEND_RESULT).apply {
                setPackage(packageName)
                putExtra(SendClipActivity.EXTRA_SUCCESS, success)
            })
        }
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        scope.cancel()
        super.onDestroy()
    }

    // Not used — accessibilityEventTypes="" in config means no events arrive here.
    override fun onAccessibilityEvent(event: AccessibilityEvent) = Unit
    override fun onInterrupt() = Unit

    companion object {
        private const val DEBOUNCE_MS = 2_000L
    }
}
