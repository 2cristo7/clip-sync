package com.clipsync.model

import org.json.JSONObject

/**
 * Wire protocol payload shared between mac server and android client.
 * See docs/protocol.md and docs/phase-4-summary.md.
 *
 * Example: {"type":"text","mime":"text/plain","data":"<base64>","ts":172..., "nonce":"..."}
 */
data class ClipPayload(
    val type: String,      // "text" | "image"
    val mime: String,
    val data: String,      // base64 encoded payload
    val ts: Long,          // unix seconds
    val nonce: String
) {
    fun toJson(): String {
        val o = JSONObject()
        o.put("type", type)
        o.put("mime", mime)
        o.put("data", data)
        o.put("ts", ts)
        o.put("nonce", nonce)
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
                nonce = o.getString("nonce")
            )
        }
    }
}
