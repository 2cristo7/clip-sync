package com.clipsync.share

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.provider.OpenableColumns
import android.widget.Toast
import com.clipsync.model.ClipPayloadBuilder
import com.clipsync.net.ClipClient
import com.clipsync.overlay.ClipSender
import com.clipsync.storage.Prefs
import com.clipsync.util.L
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MacShareActivity : Activity() {

    private val scope = CoroutineScope(Dispatchers.Main)

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val prefs = Prefs(applicationContext)
        if (!prefs.hasPairing()) {
            toast("Pair ClipSync first")
            finish()
            return
        }

        val host = prefs.host ?: run { finish(); return }
        val port = prefs.port
        val token = prefs.token ?: run { finish(); return }
        val secret = prefs.pairingSecret ?: run { finish(); return }
        val fp = prefs.fp ?: run { finish(); return }

        when (intent?.action) {
            Intent.ACTION_SEND_MULTIPLE -> {
                @Suppress("DEPRECATION")
                val uris: List<Uri> = intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM) ?: emptyList()
                if (uris.isEmpty()) { toast("Nothing to send"); finish(); return }
                L.action(M, "share multiple count=${uris.size}")
                sendMultiple(host, port, token, secret, fp, uris)
                return
            }
        }

        val uri: Uri? = intent?.getParcelableExtra(Intent.EXTRA_STREAM)
        if (uri == null) {
            val text = intent?.getStringExtra(Intent.EXTRA_TEXT)
            if (text != null) {
                sendPayload(host, port, token, secret, fp, ClipPayloadBuilder.text(text))
                return
            }
            toast("Nothing to send")
            finish()
            return
        }

        val mime = contentResolver.getType(uri) ?: "application/octet-stream"
        L.action(M, "share received uri=$uri mime=$mime")

        scope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    val stream = contentResolver.openInputStream(uri)
                        ?: return@withContext "Cannot open file"
                    val bytes = stream.use { it.readBytes() }
                    if (bytes.size > ClipPayloadBuilder.MAX_FILE_BYTES) {
                        return@withContext "File too large (max ${ClipPayloadBuilder.MAX_FILE_BYTES / 1_048_576} MB)"
                    }
                    val payload = if (mime.startsWith("image/")) {
                        ClipPayloadBuilder.image(mime, bytes)
                    } else {
                        val name = getFileName(uri)
                        ClipPayloadBuilder.file(mime, name, bytes)
                    }
                    val sender = ClipSender(ClipClient())
                    val r = sender.send(host, port, token, secret, fp, payload)
                    L.event(M, "share send result=$r")
                    if (r is ClipSender.Result.Failed) r.reason else null
                } catch (t: Throwable) {
                    L.error(M, "share send failed", t)
                    t.message ?: "Unknown error"
                }
            }
            toast(if (result == null) "Sent to Mac" else "Failed: $result")
            finish()
        }
    }

    private fun sendMultiple(host: String, port: Int, token: String, secret: String, fp: String, uris: List<Uri>) {
        scope.launch {
            var sent = 0
            var failed = 0
            for (uri in uris) {
                val error = withContext(Dispatchers.IO) {
                    try {
                        val stream = contentResolver.openInputStream(uri) ?: return@withContext "Cannot open"
                        val bytes = stream.use { it.readBytes() }
                        if (bytes.size > ClipPayloadBuilder.MAX_FILE_BYTES) return@withContext "Too large"
                        val mime = contentResolver.getType(uri) ?: "application/octet-stream"
                        val payload = if (mime.startsWith("image/")) {
                            ClipPayloadBuilder.image(mime, bytes)
                        } else {
                            ClipPayloadBuilder.file(mime, getFileName(uri), bytes)
                        }
                        val r = ClipSender(ClipClient()).send(host, port, token, secret, fp, payload)
                        if (r is ClipSender.Result.Failed) r.reason else null
                    } catch (t: Throwable) { t.message ?: "error" }
                }
                if (error == null) sent++ else failed++
            }
            val msg = if (failed == 0) "Sent $sent to Mac" else "Sent $sent, failed $failed"
            toast(msg)
            finish()
        }
    }

    private fun sendPayload(host: String, port: Int, token: String, secret: String, fp: String, payload: com.clipsync.model.ClipPayload) {
        scope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    val r = ClipSender(ClipClient()).send(host, port, token, secret, fp, payload)
                    if (r is ClipSender.Result.Failed) r.reason else null
                } catch (t: Throwable) { t.message ?: "Unknown error" }
            }
            toast(if (result == null) "Sent to Mac" else "Failed: $result")
            finish()
        }
    }

    private fun getFileName(uri: Uri): String {
        if (uri.scheme == "content") {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0) return cursor.getString(idx)
                }
            }
        }
        return uri.lastPathSegment ?: "unknown_file"
    }

    private fun toast(msg: String) =
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()

    companion object {
        private const val M = "Share"
    }
}
