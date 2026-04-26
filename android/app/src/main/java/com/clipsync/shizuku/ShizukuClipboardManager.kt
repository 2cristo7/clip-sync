package com.clipsync.shizuku

import android.content.ComponentName
import android.content.Context
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.util.Log
import rikka.shizuku.Shizuku

/**
 * Bridge between the app and the Shizuku [ClipboardUserService].
 *
 * Manages the full Shizuku lifecycle: installation detection, permission
 * requesting, UserService binding, death/reconnection handling.
 *
 * Call [initialize] once (e.g. from [ClipForegroundService.onCreate]).
 * Query [isAvailable] before every clipboard operation. Call [destroy]
 * on service teardown.
 */
class ShizukuClipboardManager(private val context: Context) {

    enum class State {
        NOT_INSTALLED,
        NOT_RUNNING,
        NO_PERMISSION,
        BINDING,
        READY,
        DEAD
    }

    var state: State = State.NOT_INSTALLED
        private set

    var onStateChanged: ((State) -> Unit)? = null

    private var userService: IClipUserService? = null
    private val handler = Handler(Looper.getMainLooper())
    private var reconnectAttempts = 0

    // --- Shizuku listeners ---

    private val binderReceivedListener = Shizuku.OnBinderReceivedListener {
        Log.i(TAG, "Shizuku binder received")
        checkPermissionAndBind()
    }

    private val binderDeadListener = Shizuku.OnBinderDeadListener {
        Log.w(TAG, "Shizuku binder dead")
        userService = null
        updateState(State.NOT_RUNNING)
    }

    private val permissionResultListener =
        Shizuku.OnRequestPermissionResultListener { requestCode, grantResult ->
            if (requestCode == PERMISSION_REQUEST_CODE) {
                if (grantResult == PackageManager.PERMISSION_GRANTED) {
                    bindUserService()
                } else {
                    updateState(State.NO_PERMISSION)
                }
            }
        }

    // --- ServiceConnection ---

    private val serviceConnection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName, binder: IBinder) {
            if (binder.pingBinder()) {
                userService = IClipUserService.Stub.asInterface(binder)
                reconnectAttempts = 0
                updateState(State.READY)
                Log.i(TAG, "UserService connected")
            }
        }

        override fun onServiceDisconnected(name: ComponentName) {
            userService = null
            updateState(State.DEAD)
            Log.w(TAG, "UserService disconnected")
            scheduleReconnect()
        }
    }

    // --- Public API ---

    fun initialize() {
        if (!isShizukuInstalled()) {
            updateState(State.NOT_INSTALLED)
            return
        }
        Shizuku.addBinderReceivedListenerSticky(binderReceivedListener)
        Shizuku.addBinderDeadListener(binderDeadListener)
        Shizuku.addRequestPermissionResultListener(permissionResultListener)
    }

    fun requestPermission() {
        if (!Shizuku.pingBinder()) return
        if (Shizuku.shouldShowRequestPermissionRationale()) {
            updateState(State.NO_PERMISSION)
            return
        }
        Shizuku.requestPermission(PERMISSION_REQUEST_CODE)
    }

    fun isAvailable(): Boolean =
        state == State.READY && userService?.asBinder()?.pingBinder() == true

    fun getClipboardText(): String? =
        runCatching { userService?.clipboardText }.getOrNull()

    fun setClipboardText(text: String) {
        runCatching { userService?.setClipboardText(text) }
    }

    fun getClipboardHash(): Int =
        runCatching { userService?.clipboardHash ?: 0 }.getOrDefault(0)

    fun getClipboardMime(): String? =
        runCatching { userService?.clipboardMime }.getOrNull()

    fun getClipboardUri(): String? =
        runCatching { userService?.clipboardUri }.getOrNull()

    fun setClipboardUri(uri: String, mime: String) {
        runCatching { userService?.setClipboardUri(uri, mime) }
    }

    fun destroy() {
        handler.removeCallbacksAndMessages(null)
        try { Shizuku.removeBinderReceivedListener(binderReceivedListener) } catch (_: Exception) {}
        try { Shizuku.removeBinderDeadListener(binderDeadListener) } catch (_: Exception) {}
        try { Shizuku.removeRequestPermissionResultListener(permissionResultListener) } catch (_: Exception) {}
        unbindUserService()
        userService = null
    }

    // --- Internals ---

    private fun checkPermissionAndBind() {
        if (!Shizuku.pingBinder()) {
            updateState(State.NOT_RUNNING)
            return
        }
        if (Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) {
            bindUserService()
        } else {
            updateState(State.NO_PERMISSION)
        }
    }

    private fun buildServiceArgs(): Shizuku.UserServiceArgs =
        Shizuku.UserServiceArgs(
            ComponentName(context.packageName, ClipboardUserService::class.java.name)
        )
            .processNameSuffix("clipboard")
            .debuggable(false)
            .version(2)

    private fun bindUserService() {
        updateState(State.BINDING)
        try {
            Shizuku.bindUserService(buildServiceArgs(), serviceConnection)
        } catch (e: Exception) {
            Log.e(TAG, "bindUserService failed: ${e.message}")
            updateState(State.NOT_RUNNING)
        }
    }

    private fun unbindUserService() {
        try {
            Shizuku.unbindUserService(buildServiceArgs(), serviceConnection, true)
        } catch (_: Exception) { }
    }

    private fun scheduleReconnect() {
        if (reconnectAttempts >= MAX_RECONNECT) return
        reconnectAttempts++
        handler.postDelayed({
            if (Shizuku.pingBinder()) bindUserService()
            else scheduleReconnect()
        }, RECONNECT_DELAY_MS * reconnectAttempts)
    }

    private fun updateState(new: State) {
        if (state != new) {
            state = new
            onStateChanged?.invoke(new)
        }
    }

    private fun isShizukuInstalled(): Boolean {
        return try {
            @Suppress("DEPRECATION")
            context.packageManager.getPackageInfo("moe.shizuku.privileged.api", 0)
            true
        } catch (_: PackageManager.NameNotFoundException) {
            false
        }
    }

    companion object {
        private const val TAG = "ClipSync/Shizuku"
        const val PERMISSION_REQUEST_CODE = 7777
        private const val MAX_RECONNECT = 5
        private const val RECONNECT_DELAY_MS = 2_000L
    }
}
