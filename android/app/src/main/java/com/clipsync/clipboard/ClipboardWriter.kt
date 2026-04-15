package com.clipsync.clipboard

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.ContentResolver
import android.content.Context
import android.net.Uri
import android.os.PersistableBundle

/**
 * Helper over [ClipboardManager]. Exposes pure-ish builders so the
 * `ClipData` construction can be unit-tested without instrumentation.
 */
object ClipboardWriter {

    const val LABEL = "clipsync"

    /** Build a plain text [ClipData]. Pure: no Android framework I/O. */
    fun buildTextClip(text: String): ClipData = ClipData.newPlainText(LABEL, text)

    /**
     * Build an image [ClipData] backed by a content `Uri`. Uses the
     * [ContentResolver] to resolve the MIME type when possible.
     */
    fun buildImageClip(resolver: ContentResolver, uri: Uri, fallbackMime: String): ClipData {
        val mime = resolver.getType(uri) ?: fallbackMime
        val description = ClipDescription(LABEL, arrayOf(mime))
        val item = ClipData.Item(uri)
        return ClipData(description, item)
    }

    /** Write the given text to the primary clipboard. */
    fun writeText(context: Context, text: String) {
        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(buildTextClip(text))
    }

    /** Write an image referenced by [uri] to the primary clipboard. */
    fun writeImage(context: Context, uri: Uri, mime: String) {
        val cm = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        cm.setPrimaryClip(buildImageClip(context.contentResolver, uri, mime))
    }

    /**
     * Marker describing the clip was marked sensitive. Kept as a no-op
     * helper so callers on any API level can invoke it uniformly.
     */
    fun markSensitive(clip: ClipData): ClipData {
        val desc = clip.description
        desc.extras = PersistableBundle().apply {
            putBoolean("android.content.extra.IS_SENSITIVE", true)
        }
        return clip
    }
}
