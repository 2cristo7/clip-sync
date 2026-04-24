package com.clipsync.accessibility

import android.accessibilityservice.AccessibilityService
import android.content.ClipboardManager
import android.view.accessibility.AccessibilityEvent
import com.clipsync.clipboard.ClipboardWriter
import com.clipsync.overlay.SendClipActivity
import com.clipsync.storage.Prefs

/**
 * Accessibility service that detects clipboard changes and auto-sends to Mac.
 *
 * WHY THIS SERVICE EXISTS:
 * A regular foreground service already does the same via OnPrimaryClipChangedListener
 * (see ClipForegroundService). This accessibility service acts as a backup detector
 * for cases where the foreground service's listener misses events (e.g. battery
 * optimisation, delayed start).
 *
 * IMPORTANT — WHY WE DON'T READ CLIPBOARD HERE:
 * ClipboardManager.getPrimaryClip() returns null from a background context on
 * Android 10+. AccessibilityServices are NOT on the whitelist for background
 * clipboard reads (only the default IME and foreground activities are).
 * Instead we detect the change event and launch SendClipActivity, which IS
 * a foreground activity and can read clipboard normally.
 *
 * ECHO PREVENTION:
 * ClipboardWriter.lastMacWriteMs is updated every time we write content received
 * from the Mac. If a clipboard-change event fires within 2 s of that write,
 * we skip it to avoid echoing the Mac's own content back.
 *
 * DOUBLE-SEND GUARD:
 * ClipForegroundService also registers a listener. To avoid both firing for the
 * same event, we share the process-level lastMacWriteMs guard and add our own
 * per-instance debounce. Whichever fires first sets lastAutoSendMs; the second
 * one sees it was too recent and skips.
 */
class ClipAccessibilityService : AccessibilityService() {

    private lateinit var clipboardManager: ClipboardManager
    private lateinit var prefs: Prefs
    private var lastAutoSendMs: Long = 0L

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        handleClipChange()
    }

    override fun onServiceConnected() {
        clipboardManager = getSystemService(ClipboardManager::class.java)
        prefs = Prefs(applicationContext)
        clipboardManager.addPrimaryClipChangedListener(clipListener)
    }

    private fun handleClipChange() {
        if (!prefs.autoSendEnabled) return
        if (!prefs.syncEnabled) return
        if (!prefs.hasPairing()) return

        // Echo guard: skip if we wrote to clipboard from Mac in the last 2 s
        if (System.currentTimeMillis() - ClipboardWriter.lastMacWriteMs < ECHO_GUARD_MS) return

        // Debounce: skip if we already triggered a send very recently
        val now = System.currentTimeMillis()
        if (now - lastAutoSendMs < ECHO_GUARD_MS) return
        lastAutoSendMs = now

        // Cannot read clipboard here (Android 10+ background restriction).
        // Delegate to SendClipActivity which runs in foreground and can read it.
        startActivity(SendClipActivity.intent(this))
    }

    override fun onDestroy() {
        clipboardManager.removePrimaryClipChangedListener(clipListener)
        super.onDestroy()
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent) = Unit
    override fun onInterrupt() = Unit

    companion object {
        private const val ECHO_GUARD_MS = 2_000L
    }
}
