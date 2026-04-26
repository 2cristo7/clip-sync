package com.clipsync.accessibility

import android.accessibilityservice.AccessibilityService
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import com.clipsync.clipboard.ClipboardWriter
import com.clipsync.overlay.SendClipActivity
import com.clipsync.storage.Prefs

/**
 * Polls the clipboard every [POLL_MS] ms and auto-sends on change.
 *
 * OnPrimaryClipChangedListener is blocked for background processes on Android 12+,
 * even in AccessibilityServices. However, AccessibilityServices CAN call
 * getPrimaryClip() without restrictions. Polling is the only reliable approach
 * on Android 13+ (Pixel 9a, stock Android 14/15).
 *
 * User must enable: Settings → Accessibility → ClipSync.
 */
class ClipAccessibilityService : AccessibilityService() {

    private val handler = Handler(Looper.getMainLooper())
    private var lastClipHash = 0
    private var hashSeeded = false   // true after first successful hash read
    private var lastAutoSendMs = 0L
    private var pollCount = 0

    private val pollRunnable = object : Runnable {
        override fun run() {
            checkClipboard()
            handler.postDelayed(this, POLL_MS)
        }
    }

    override fun onServiceConnected() {
        if (Build.VERSION.SDK_INT > Build.VERSION_CODES.R) {
            disableSelf()
            return
        }
        val seed = readClipHash()
        if (seed != 0) {
            lastClipHash = seed
            hashSeeded = true
        }
        handler.post(pollRunnable)
        Log.i(TAG, "ClipAccessibilityService connected, polling every ${POLL_MS}ms (seed hash=$lastClipHash, seeded=$hashSeeded)")
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) = Unit
    override fun onInterrupt() = Unit

    override fun onUnbind(intent: Intent?): Boolean {
        if (Build.VERSION.SDK_INT > Build.VERSION_CODES.R) return super.onUnbind(intent)
        handler.removeCallbacks(pollRunnable)
        Log.i(TAG, "ClipAccessibilityService unbound")
        return super.onUnbind(intent)
    }

    private fun checkClipboard() {
        pollCount++
        val verbose = pollCount % 10 == 0 // every 5s

        val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = try { cm.primaryClip } catch (e: Exception) {
            Log.w(TAG, "getPrimaryClip() threw: ${e.message}")
            null
        }

        if (verbose) {
            val desc = if (clip == null) "NULL" else "itemCount=${clip.itemCount}"
            Log.d(TAG, "poll #$pollCount: clip=$desc lastHash=$lastClipHash")
        }

        if (clip == null || clip.itemCount == 0) return

        val item = clip.getItemAt(0)
        val content = item.text?.toString() ?: item.uri?.toString() ?: ""
        val hash = content.hashCode()

        if (verbose) {
            Log.d(TAG, "  preview='${content.take(40)}' hash=$hash")
        }

        if (hash == 0 || hash == lastClipHash) return

        // First successful read after startup: just record the hash, don't send.
        // Prevents sending whatever was already in the clipboard when the service starts.
        if (!hashSeeded) {
            lastClipHash = hash
            hashSeeded = true
            Log.d(TAG, "Hash seeded on first read: $hash")
            return
        }

        Log.i(TAG, "Hash changed $lastClipHash -> $hash  preview='${content.take(60)}'")
        lastClipHash = hash

        val prefs = Prefs(applicationContext)
        if (!prefs.autoSendEnabled) { Log.d(TAG, "skip: autoSendEnabled=false"); return }
        if (!prefs.syncEnabled)     { Log.d(TAG, "skip: syncEnabled=false"); return }
        if (!prefs.hasPairing())    { Log.d(TAG, "skip: no pairing"); return }

        val now = System.currentTimeMillis()
        if (now - ClipboardWriter.lastMacWriteMs < 2_000) { Log.d(TAG, "skip: echo"); return }
        if (now - lastAutoSendMs < 1_000)                 { Log.d(TAG, "skip: debounce"); return }

        lastAutoSendMs = now
        Log.i(TAG, "-> launching auto-send")
        startActivity(SendClipActivity.intent(this).putExtra(SendClipActivity.EXTRA_AUTO_SEND, true))
    }

    private fun readClipHash(): Int {
        return try {
            val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            val clip = cm.primaryClip ?: return 0
            if (clip.itemCount == 0) return 0
            val item = clip.getItemAt(0)
            (item.text?.toString() ?: item.uri?.toString() ?: "").hashCode()
        } catch (_: Exception) { 0 }
    }

    companion object {
        private const val TAG = "ClipSync"
        private const val POLL_MS = 500L
    }
}
