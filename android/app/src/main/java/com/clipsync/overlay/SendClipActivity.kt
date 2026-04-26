package com.clipsync.overlay

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.util.Log
import android.view.View
import android.widget.Toast
import com.clipsync.model.ClipPayloadBuilder
import com.clipsync.storage.Prefs
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Transparent trampoline Activity launched by the clipboard overlay FAB.
 *
 * Android 10+ restricts ClipboardManager.getPrimaryClip() to apps that have
 * a focused window (or are the default IME). Two issues prevented this from
 * working on Pixel / Android 12-13:
 *
 *  1. No setContentView() — without a view hierarchy the window is not fully
 *     registered in WindowManager and may not receive input focus.
 *  2. Clipboard was read in onCreate() — the window has not received input
 *     focus yet at that point; focus arrives later via onWindowFocusChanged().
 *
 * Fix: attach a transparent content view so the window is properly set up,
 * then read clipboard in onWindowFocusChanged(hasFocus=true). A short
 * postDelayed fallback handles the edge case where a translucent window never
 * fires onWindowFocusChanged on some devices.
 */
class SendClipActivity : Activity() {

    private val scope = CoroutineScope(Dispatchers.Main)
    private val sender = ClipSender()
    private val handler = Handler(Looper.getMainLooper())

    private var clipboardAttempted = false
    private var isAutoSend = false

    private val fallbackRunnable = Runnable {
        if (!clipboardAttempted) {
            clipboardAttempted = true
            sendClipboard()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        isAutoSend = intent.getBooleanExtra(EXTRA_AUTO_SEND, false)

        // A real (transparent) content view is required so the window is
        // properly registered in WindowManager and can receive input focus.
        // Without this, getPrimaryClip() returns null on Android 10+.
        setContentView(View(this))

        // Fallback: if onWindowFocusChanged never fires (can happen with
        // translucent activities on some Android 12-13 builds), read clipboard
        // after 200 ms — enough time for the window to settle.
        handler.postDelayed(fallbackRunnable, 200)
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        // Primary trigger: the window now has input focus, so getPrimaryClip()
        // will return the actual content instead of null.
        if (hasFocus && !clipboardAttempted) {
            clipboardAttempted = true
            handler.removeCallbacks(fallbackRunnable)
            sendClipboard()
        }
    }

    override fun onDestroy() {
        handler.removeCallbacks(fallbackRunnable)
        super.onDestroy()
    }

    private fun sendClipboard() {
        val prefs = Prefs(applicationContext)
        if (!prefs.hasPairing() || prefs.pairingSecret.isNullOrEmpty()) {
            toast("Pair ClipSync first")
            finish()
            return
        }

        val host = prefs.host!!
        val port = prefs.port
        val token = prefs.token!!
        val secret = prefs.pairingSecret!!
        val fp = prefs.fp!!

        val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val clip = cm.primaryClip
        if (clip == null || clip.itemCount == 0) {
            toast("Nothing to send")
            finish()
            return
        }

        val item = clip.getItemAt(0)
        val mimeType = clip.description?.getMimeType(0) ?: ""

        when {
            // Check image MIME first — some clips have both URI and text
            mimeType.startsWith("image/") -> {
                val uri = item.uri
                if (uri == null) {
                    toast("No image in clipboard")
                    finish()
                    return
                }
                scope.launch {
                    val result = withContext(Dispatchers.IO) {
                        try {
                            val stream = contentResolver.openInputStream(uri)
                                ?: return@withContext ClipSender.Result.Failed("Can't open image")
                            val bytes = stream.use { it.readBytes() }
                            if (bytes.size > ClipPayloadBuilder.MAX_IMAGE_BYTES) {
                                return@withContext ClipSender.Result.Failed("Image too large")
                            }
                            val mime = contentResolver.getType(uri) ?: mimeType
                            val payload = ClipPayloadBuilder.image(mime, bytes)
                            sender.send(host, port, token, secret, fp, payload)
                        } catch (t: Throwable) {
                            ClipSender.Result.Failed(t.message ?: "read error")
                        }
                    }
                    handleResult(result)
                }
            }
            mimeType.startsWith("text/") || item.text != null -> {
                val text = item.coerceToText(this)?.toString()
                if (text.isNullOrEmpty()) {
                    toast("Empty clipboard")
                    finish()
                    return
                }
                val payload = ClipPayloadBuilder.text(text)
                scope.launch {
                    val result = withContext(Dispatchers.IO) {
                        sender.send(host, port, token, secret, fp, payload)
                    }
                    handleResult(result)
                }
            }
            else -> {
                toast("Unsupported clipboard content")
                finish()
            }
        }
    }

    private fun handleResult(result: ClipSender.Result) {
        when (result) {
            is ClipSender.Result.Ok -> {
                toast("Sent to Mac")
                sendBroadcast(Intent(ACTION_SEND_RESULT).apply {
                    setPackage(packageName)
                    putExtra(EXTRA_SUCCESS, true)
                })
            }
            is ClipSender.Result.Failed -> {
                toast("Failed: ${result.reason}")
                sendBroadcast(Intent(ACTION_SEND_RESULT).apply {
                    setPackage(packageName)
                    putExtra(EXTRA_SUCCESS, false)
                })
                Log.w(TAG, "Send failed: ${result.reason}")
            }
        }
        finish()
    }

    private fun toast(msg: String) {
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()
    }

    companion object {
        private const val TAG = "ClipSync/Send"
        const val ACTION_SEND_RESULT = "com.clipsync.action.SEND_RESULT"
        const val EXTRA_SUCCESS = "success"
        const val EXTRA_AUTO_SEND = "auto_send"

        fun intent(context: Context): Intent {
            return Intent(context, SendClipActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
                addFlags(Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS)
            }
        }
    }
}
