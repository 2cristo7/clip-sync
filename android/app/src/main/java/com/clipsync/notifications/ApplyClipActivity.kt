package com.clipsync.notifications

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import com.clipsync.util.L
import android.widget.Toast
import com.clipsync.clipboard.ClipboardWriter

/**
 * Transparent trampoline Activity. Launched from the incoming-clip
 * notification; writes the payload to the system clipboard and finishes
 * immediately. Using an Activity (not a BroadcastReceiver or Service)
 * satisfies Android 10+ restrictions on clipboard writes from the
 * background.
 */
class ApplyClipActivity : Activity() {

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handleIntent(intent)
        finish()
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        handleIntent(intent)
        finish()
    }

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        when (intent.action) {
            ACTION_APPLY_TEXT -> {
                val text = intent.getStringExtra(EXTRA_TEXT)
                if (text.isNullOrEmpty()) {
                    L.warn(M, "ACTION_APPLY_TEXT with empty extra")
                    return
                }
                ClipboardWriter.writeText(this, text)
                toast("Copied to clipboard")
            }
            ACTION_APPLY_IMAGE -> {
                val uriStr = intent.getStringExtra(EXTRA_URI)
                val mime = intent.getStringExtra(EXTRA_MIME) ?: "image/*"
                if (uriStr.isNullOrEmpty()) {
                    L.warn(M, "ACTION_APPLY_IMAGE with empty uri")
                    return
                }
                val uri = Uri.parse(uriStr)
                try {
                    ClipboardWriter.writeImage(this, uri, mime)
                    toast("Image copied to clipboard")
                } catch (t: Throwable) {
                    L.warn(M, "writeImage failed: ${t.message}")
                    toast("Failed to copy image")
                }
            }
            else -> L.warn(M, "Unknown action: ${intent.action}")
        }
    }

    private fun toast(msg: String) {
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()
    }

    companion object {
        private const val M = "Apply"
        const val ACTION_APPLY_TEXT = "com.clipsync.action.APPLY_TEXT"
        const val ACTION_APPLY_IMAGE = "com.clipsync.action.APPLY_IMAGE"
        const val EXTRA_TEXT = "com.clipsync.extra.TEXT"
        const val EXTRA_URI = "com.clipsync.extra.URI"
        const val EXTRA_MIME = "com.clipsync.extra.MIME"
        const val EXTRA_NONCE = "com.clipsync.extra.NONCE"

        fun textIntent(context: Context, text: String, nonce: String): Intent {
            return Intent(context, ApplyClipActivity::class.java).apply {
                action = ACTION_APPLY_TEXT
                putExtra(EXTRA_TEXT, text)
                putExtra(EXTRA_NONCE, nonce)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        }

        fun imageIntent(context: Context, uri: Uri, mime: String, nonce: String): Intent {
            return Intent(context, ApplyClipActivity::class.java).apply {
                action = ACTION_APPLY_IMAGE
                putExtra(EXTRA_URI, uri.toString())
                putExtra(EXTRA_MIME, mime)
                putExtra(EXTRA_NONCE, nonce)
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            }
        }
    }
}
