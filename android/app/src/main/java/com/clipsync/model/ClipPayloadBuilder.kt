package com.clipsync.model

import java.util.Base64
import java.util.UUID

/**
 * Constructs [ClipPayload] instances for outbound dispatch (Pixel → Mac).
 *
 * Extracted from the removed `ShareSender` so that any send path (overlay,
 * notification action, etc.) can build payloads without duplicating logic.
 */
object ClipPayloadBuilder {

    fun text(text: String, clockMs: Long = System.currentTimeMillis()): ClipPayload {
        val b64 = Base64.getEncoder().encodeToString(text.toByteArray(Charsets.UTF_8))
        return ClipPayload(
            type = "text",
            mime = "text/plain",
            data = b64,
            ts = clockMs / 1000L,
            nonce = UUID.randomUUID().toString()
        )
    }

    fun image(mime: String, bytes: ByteArray, clockMs: Long = System.currentTimeMillis()): ClipPayload {
        val b64 = Base64.getEncoder().encodeToString(bytes)
        return ClipPayload(
            type = "image",
            mime = mime,
            data = b64,
            ts = clockMs / 1000L,
            nonce = UUID.randomUUID().toString()
        )
    }

    const val MAX_IMAGE_BYTES: Int = 20 * 1024 * 1024
}
