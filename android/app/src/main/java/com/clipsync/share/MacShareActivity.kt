package com.clipsync.share

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
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

        val uri: Uri? = intent?.getParcelableExtra(Intent.EXTRA_STREAM)
        if (uri == null) {
            toast("No image received")
            finish()
            return
        }

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

        L.action(M, "share received uri=$uri")

        scope.launch {
            val result = withContext(Dispatchers.IO) {
                try {
                    val stream = contentResolver.openInputStream(uri)
                        ?: return@withContext "Cannot open image"
                    val bytes = stream.use { it.readBytes() }
                    if (bytes.size > ClipPayloadBuilder.MAX_IMAGE_BYTES) {
                        return@withContext "Image too large (max ${ClipPayloadBuilder.MAX_IMAGE_BYTES / 1_048_576} MB)"
                    }
                    val mime = contentResolver.getType(uri) ?: "image/jpeg"
                    val payload = ClipPayloadBuilder.image(mime, bytes)
                    val sender = ClipSender(ClipClient())
                    val r = sender.send(host, port, token, secret, fp, payload)
                    L.event(M, "share send result=$r")
                    null // null = success
                } catch (t: Throwable) {
                    L.error(M, "share send failed", t)
                    t.message ?: "Unknown error"
                }
            }
            if (result == null) {
                toast("Sent to Mac")
            } else {
                toast("Failed: $result")
            }
            finish()
        }
    }

    private fun toast(msg: String) =
        Toast.makeText(applicationContext, msg, Toast.LENGTH_SHORT).show()

    companion object {
        private const val M = "Share"
    }
}
