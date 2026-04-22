package com.clipsync.overlay

import android.app.Activity
import android.content.ClipboardManager
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.util.Log
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
 * Because Android 10+ only allows reading `ClipboardManager.getPrimaryClip()`
 * when the app is in the foreground, this Activity briefly gains focus, reads
 * the clipboard, dispatches the payload to the Mac via [ClipSender], and then
 * calls `finish()`.
 *
 * The entire lifecycle is invisible to the user (translucent theme, no UI).
 */
class SendClipActivity : Activity() {

    private val scope = CoroutineScope(Dispatchers.Main)
    private val sender = ClipSender()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        sendClipboard()
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
                // Notify overlay to show success feedback
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

        fun intent(context: Context): Intent {
            return Intent(context, SendClipActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_ACTIVITY_NO_ANIMATION)
                addFlags(Intent.FLAG_ACTIVITY_EXCLUDE_FROM_RECENTS)
            }
        }
    }
}
