package com.clipsync.model

import android.util.Base64
import java.util.UUID

/**
 * Constructs [ClipPayload] instances for outbound dispatch (Pixel → Mac).
 *
 * Extracted from the removed `ShareSender` so that any send path (overlay,
 * notification action, etc.) can build payloads without duplicating logic.
 */
object ClipPayloadBuilder {

    fun text(text: String, clockMs: Long = System.currentTimeMillis()): ClipPayload {
        val b64 = Base64.encodeToString(text.toByteArray(Charsets.UTF_8), Base64.NO_WRAP)
        return ClipPayload(
            type = "text",
            mime = "text/plain",
            data = b64,
            ts = clockMs,
            nonce = UUID.randomUUID().toString()
        )
    }

    fun image(mime: String, bytes: ByteArray, clockMs: Long = System.currentTimeMillis()): ClipPayload {
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        return ClipPayload(
            type = "image",
            mime = mime,
            data = b64,
            ts = clockMs,
            nonce = UUID.randomUUID().toString()
        )
    }

    fun file(mime: String, name: String, bytes: ByteArray, clockMs: Long = System.currentTimeMillis()): ClipPayload {
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        return ClipPayload(
            type = "file",
            mime = mime,
            data = b64,
            ts = clockMs,
            nonce = UUID.randomUUID().toString(),
            name = name
        )
    }

    const val MAX_IMAGE_BYTES: Int = 20 * 1024 * 1024
    const val MAX_FILE_BYTES: Int = 20 * 1024 * 1024
}
