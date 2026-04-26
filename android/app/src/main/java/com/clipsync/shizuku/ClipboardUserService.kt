package com.clipsync.shizuku

import android.content.ClipData
import android.os.IBinder
import android.util.Log
import java.lang.reflect.Method
import rikka.shizuku.SystemServiceHelper

/**
 * Shizuku UserService that runs in a separate process with UID 2000 (shell).
 *
 * Accesses the system clipboard via reflection on the hidden [IClipboard]
 * interface. Hidden API restrictions do not apply to UID 2000 processes,
 * so no HiddenApiBypass is needed.
 *
 * If the IClipboard method signatures change in a future Android version,
 * all methods gracefully return null/0 and the app falls back to the
 * AccessibilityService approach.
 */
class ClipboardUserService : IClipUserService.Stub() {

    private val clipboard: Any by lazy {
        val binder = SystemServiceHelper.getSystemService("clipboard")
        val stubClass = Class.forName("android.content.IClipboard\$Stub")
        val asInterface = stubClass.getMethod("asInterface", IBinder::class.java)
        asInterface.invoke(null, binder)!!
    }

    override fun getClipboardText(): String? {
        val clip = getPrimaryClip() ?: return null
        if (clip.itemCount == 0) return null
        return clip.getItemAt(0).text?.toString()
    }

    override fun setClipboardText(text: String) {
        val clip = ClipData.newPlainText("clipsync", text)
        setPrimaryClipInternal(clip)
    }

    override fun getClipboardHash(): Int {
        val clip = getPrimaryClip() ?: return 0
        if (clip.itemCount == 0) return 0
        val item = clip.getItemAt(0)
        val content = item.text?.toString() ?: item.uri?.toString() ?: ""
        return content.hashCode()
    }

    override fun getClipboardMime(): String? {
        val clip = getPrimaryClip() ?: return null
        val desc = clip.description ?: return null
        return if (desc.mimeTypeCount > 0) desc.getMimeType(0) else null
    }

    override fun destroy() {
        System.exit(0)
    }

    // --- Reflection-based access to IClipboard hidden API ---
    // We discover the method signature at runtime instead of hard-coding parameter
    // types, because Android adds extra parameters across versions (e.g. deviceId
    // in Android 14+). We pick the method by name and fill in sensible defaults
    // (null for String?, 0 for int) for any extra parameters we don't recognise.

    private val getMethod: Method? by lazy { findMethod("getPrimaryClip") }
    private val setMethod: Method? by lazy { findMethod("setPrimaryClip") }

    private fun findMethod(name: String): Method? {
        val m = clipboard.javaClass.methods
            .filter { it.name == name }
            .maxByOrNull { it.parameterCount }
        if (m == null) Log.e(TAG, "Method $name not found on IClipboard")
        return m
    }

    private fun getPrimaryClip(): ClipData? {
        val method = getMethod ?: return null
        return try {
            val args = buildArgs(method, firstArg = PACKAGE)
            method.invoke(clipboard, *args) as? ClipData
        } catch (e: java.lang.reflect.InvocationTargetException) {
            Log.w(TAG, "getPrimaryClip failed (cause): ${e.cause}")
            null
        } catch (e: Exception) {
            Log.w(TAG, "getPrimaryClip failed: $e")
            null
        }
    }

    private fun setPrimaryClipInternal(clip: ClipData) {
        val method = setMethod ?: return
        try {
            val args = buildArgs(method, firstArg = clip)
            method.invoke(clipboard, *args)
        } catch (e: java.lang.reflect.InvocationTargetException) {
            Log.w(TAG, "setPrimaryClip failed (cause): ${e.cause}")
        } catch (e: Exception) {
            Log.w(TAG, "setPrimaryClip failed: $e")
        }
    }

    /**
     * Builds the argument array for an IClipboard method by inspecting
     * parameter types at runtime:
     *  - ClipData  → [firstArg] (only used in setPrimaryClip)
     *  - 1st String → package name (or [firstArg] if firstArg is String)
     *  - other Strings → null (attributionTag etc.)
     *  - int / Integer → 0  (userId, deviceId etc.)
     */
    private fun buildArgs(method: Method, firstArg: Any): Array<Any?> {
        var stringCount = 0
        return Array(method.parameterCount) { i ->
            val type = method.parameterTypes[i]
            when {
                type == ClipData::class.java -> firstArg
                type == String::class.java -> {
                    val v: Any? = if (stringCount == 0 && firstArg is String) firstArg
                                  else if (stringCount == 0) PACKAGE
                                  else null  // attributionTag etc.
                    stringCount++
                    v
                }
                type == Int::class.javaPrimitiveType || type == Integer::class.java -> 0
                else -> null
            }
        }
    }

    companion object {
        private const val TAG = "ClipSync/UserService"
        // UserService runs as UID 2000 (shell). We pass our own app package so the
        // system clipboard access notification shows "ClipSync" instead of "Shell".
        // Shell UID typically bypasses package/UID validation on most ROMs.
        // If the security check fails, getPrimaryClip() catches the SecurityException
        // and returns null, falling back gracefully.
        private const val PACKAGE = "com.clipsync.app"
    }
}
