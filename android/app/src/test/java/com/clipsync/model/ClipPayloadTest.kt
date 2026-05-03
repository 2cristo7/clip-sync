package com.clipsync.model

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test

class ClipPayloadTest {

    @Test
    fun roundtrip_text_payload() {
        val p = ClipPayload(
            type = "text",
            mime = "text/plain",
            data = "aGVsbG8=",
            ts = System.currentTimeMillis(),
            nonce = "n-123"
        )
        val parsed = ClipPayload.fromJson(p.toJson())
        assertEquals(p, parsed)
    }

    @Test
    fun toJson_contains_all_fields() {
        val now = System.currentTimeMillis()
        val p = ClipPayload("image", "image/png", "ZGF0YQ==", now, "abc")
        val o = JSONObject(p.toJson())
        assertEquals("image", o.getString("type"))
        assertEquals("image/png", o.getString("mime"))
        assertEquals("ZGF0YQ==", o.getString("data"))
        assertEquals(now, o.getLong("ts"))
        assertEquals("abc", o.getString("nonce"))
    }

    @Test
    fun fromJson_parses_server_like_frame() {
        val now = System.currentTimeMillis()
        val raw = """{"type":"text","mime":"text/plain","data":"aGk=","ts":$now,"nonce":"xyz"}"""
        val p = ClipPayload.fromJson(raw)
        assertNotNull(p)
        assertEquals("text", p.type)
        assertEquals(now, p.ts)
    }
}
