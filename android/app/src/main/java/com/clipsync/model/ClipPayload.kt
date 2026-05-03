package com.clipsync.model

import org.json.JSONObject

/**
 * Wire protocol payload shared between mac server and android client.
 * See docs/protocol.md and docs/phase-4-summary.md.
 *
 * Example: {"type":"text","mime":"text/plain","data":"<base64>","ts":172..., "nonce":"..."}
 */
data class ClipPayload(
    val type: String,      // "text" | "image" | "file"
    val mime: String,
    val data: String,      // base64 encoded payload
    val ts: Long,          // unix milliseconds
    val nonce: String,
    val name: String? = null
) {
    fun validate() {
        require(type in listOf("text", "image", "file")) { "Invalid type: $type" }
        require(mime.isNotEmpty() && mime.length <= 256) { "Invalid MIME: $mime" }
        require(nonce.isNotEmpty()) { "Empty nonce" }
        name?.let { require(it.length <= 1024) { "Name too long: ${it.length}" } }
        val nowMs = System.currentTimeMillis()
        require(kotlin.math.abs(nowMs - ts) < 5 * 60 * 1000) { "Timestamp out of range" }
    }

    fun toJson(): String {
        val o = JSONObject()
        o.put("type", type)
        o.put("mime", mime)
        o.put("data", data)
        o.put("ts", ts)
        o.put("nonce", nonce)
        if (name != null) o.put("name", name)
        return o.toString()
    }

    companion object {
        fun fromJson(raw: String): ClipPayload {
            val o = JSONObject(raw)
            return ClipPayload(
                type = o.getString("type"),
                mime = o.getString("mime"),
                data = o.getString("data"),
                ts = o.getLong("ts"),
                nonce = o.getString("nonce"),
                name = if (o.has("name")) o.getString("name") else null
            ).also { it.validate() }
        }
    }
}
