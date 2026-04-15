package com.clipsync.share

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.util.Log
import android.webkit.MimeTypeMap
import android.widget.Toast
import com.clipsync.app.MainActivity
import com.clipsync.storage.Prefs
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.ByteArrayOutputStream
import java.io.InputStream

/**
 * Invisible activity registered as a share target. Handles ACTION_SEND for
 * text/plain and image mime-types, plus ACTION_SEND_MULTIPLE for images
 * (sends only the first image to keep UX simple for v1).
 *
 * Flow:
 *   1. Validate pairing (if missing → launch MainActivity).
 *   2. Extract text or image bytes from the intent.
 *   3. Hand off to [ShareSender] on IO dispatcher.
 *   4. Toast the result and finish().
 */
class ShareReceiverActivity : Activity() {

    private val scope = CoroutineScope(Dispatchers.Main)
    private val sender: ShareSender = ShareSender()

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        handle(intent)
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        if (intent != null) handle(intent)
    }

    private fun handle(intent: Intent) {
        val prefs = Prefs(applicationContext)
        if (!prefs.hasPairing() || prefs.pairingSecret.isNullOrEmpty()) {
            Toast.makeText(this, "Pair ClipSync first", Toast.LENGTH_LONG).show()
            startActivity(Intent(this, MainActivity::class.java).apply {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            })
            finish()
            return
        }

        val host = prefs.host!!
        val port = prefs.port
        val token = prefs.token!!
        val secret = prefs.pairingSecret!!
        val fp = prefs.fp!!

        val action = intent.action
        val type = intent.type ?: ""

        when {
            action == Intent.ACTION_SEND && type == "text/plain" -> {
                val text = intent.getStringExtra(Intent.EXTRA_TEXT)
                if (text.isNullOrEmpty()) {
                    toastAndFinish("Nothing to share")
                    return
                }
                dispatchSend {
                    sender.sendText(host, port, token, secret, fp, text)
                }
            }
            action == Intent.ACTION_SEND && type.startsWith("image/") -> {
                val uri = extraStream(intent)
                if (uri == null) {
                    toastAndFinish("No image in share")
                    return
                }
                sendImageFromUri(uri, host, port, token, secret, fp)
            }
            action == Intent.ACTION_SEND_MULTIPLE && type.startsWith("image/") -> {
                val uris = extraStreamList(intent)
                val first = uris?.firstOrNull()
                if (first == null) {
                    toastAndFinish("No image in share")
                    return
                }
                sendImageFromUri(first, host, port, token, secret, fp)
            }
            else -> toastAndFinish("Unsupported share")
        }
    }

    @Suppress("DEPRECATION")
    private fun extraStream(intent: Intent): Uri? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            intent.getParcelableExtra(Intent.EXTRA_STREAM)
        }
    }

    @Suppress("DEPRECATION")
    private fun extraStreamList(intent: Intent): List<Uri>? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
        } else {
            intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
        }
    }

    private fun sendImageFromUri(
        uri: Uri,
        host: String,
        port: Int,
        token: String,
        secret: String,
        fp: String
    ) {
        scope.launch {
            val loaded = withContext(Dispatchers.IO) { readImage(uri) }
            when (loaded) {
                is ImageLoad.TooLarge -> toastAndFinish("Image too large")
                is ImageLoad.Failed -> toastAndFinish("Failed: ${loaded.reason}")
                is ImageLoad.Ok -> {
                    val result = withContext(Dispatchers.IO) {
                        sender.sendImage(host, port, token, secret, fp, loaded.mime, loaded.bytes)
                    }
                    finishWith(result)
                }
            }
        }
    }

    private fun dispatchSend(block: suspend () -> ShareSender.Result) {
        scope.launch {
            val result = withContext(Dispatchers.IO) { block() }
            finishWith(result)
        }
    }

    private fun finishWith(result: ShareSender.Result) {
        when (result) {
            is ShareSender.Result.Ok -> toastAndFinish("Sent to Mac")
            is ShareSender.Result.Failed -> toastAndFinish("Failed: ${result.reason}")
        }
    }

    private fun toastAndFinish(message: String) {
        Toast.makeText(this, message, Toast.LENGTH_SHORT).show()
        finish()
    }

    private sealed class ImageLoad {
        data class Ok(val mime: String, val bytes: ByteArray) : ImageLoad()
        data object TooLarge : ImageLoad()
        data class Failed(val reason: String) : ImageLoad()
    }

    private fun readImage(uri: Uri): ImageLoad {
        return try {
            val mime = resolveMime(uri)
            val input: InputStream = contentResolver.openInputStream(uri)
                ?: return ImageLoad.Failed("open stream")
            input.use { stream ->
                val out = ByteArrayOutputStream()
                val buf = ByteArray(32 * 1024)
                var total = 0
                while (true) {
                    val n = stream.read(buf)
                    if (n <= 0) break
                    total += n
                    if (total > ShareSender.MAX_IMAGE_BYTES) return ImageLoad.TooLarge
                    out.write(buf, 0, n)
                }
                ImageLoad.Ok(mime, out.toByteArray())
            }
        } catch (t: Throwable) {
            Log.w(TAG, "readImage failed: ${t.message}")
            ImageLoad.Failed(t.message ?: "read error")
        }
    }

    private fun resolveMime(uri: Uri): String {
        val fromResolver = contentResolver.getType(uri)
        if (!fromResolver.isNullOrEmpty()) return fromResolver
        val ext = MimeTypeMap.getFileExtensionFromUrl(uri.toString())
        if (!ext.isNullOrEmpty()) {
            val m = MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext.lowercase())
            if (!m.isNullOrEmpty()) return m
        }
        return "image/*"
    }

    companion object {
        private const val TAG = "ClipSync/Share"
    }
}
