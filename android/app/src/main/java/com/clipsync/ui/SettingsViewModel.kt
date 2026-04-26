package com.clipsync.ui

import android.content.Context
import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import android.content.pm.PackageManager
import com.clipsync.discovery.Discovered
import com.clipsync.discovery.NsdDiscovery
import com.clipsync.net.PairingApi
import android.content.Intent
import com.clipsync.service.ClipForegroundService
import com.clipsync.shizuku.ShizukuClipboardManager
import com.clipsync.storage.Prefs
import rikka.shizuku.Shizuku
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

sealed class ConnectionStatus {
    data object Disconnected : ConnectionStatus()
    data object Connecting : ConnectionStatus()
    data class Connected(val host: String) : ConnectionStatus()
    data class Paused(val host: String) : ConnectionStatus()
    data class Error(val reason: String) : ConnectionStatus()
}

data class SettingsState(
    val mode: String = Prefs.MODE_AUTO,
    val discovered: List<Discovered> = emptyList(),
    val status: ConnectionStatus = ConnectionStatus.Disconnected,
    val hasPairing: Boolean = false,
    val pairedHost: String? = null,
    val pairedPort: Int = 7010,
    val overlayEnabled: Boolean = true,
    val syncEnabled: Boolean = true,
    val autoSendEnabled: Boolean = true,
    val shizukuState: String = "not_checked",
    val error: String? = null
)

class SettingsViewModel : ViewModel() {

    private val _state = MutableStateFlow(SettingsState())
    val state: StateFlow<SettingsState> = _state.asStateFlow()

    private var discoveryJob: Job? = null

    fun bootstrap(context: Context) {
        try {
            val prefs = Prefs(context)
            val paired = prefs.hasPairing()

            _state.value = _state.value.copy(
                mode = prefs.mode,
                overlayEnabled = prefs.overlayEnabled,
                syncEnabled = prefs.syncEnabled,
                autoSendEnabled = prefs.autoSendEnabled,
                hasPairing = paired,
                pairedHost = prefs.host,
                pairedPort = prefs.port,
                status = if (paired) ConnectionStatus.Connecting else ConnectionStatus.Disconnected
            )
            if (prefs.mode == Prefs.MODE_AUTO) startDiscovery(context)
            refreshShizukuState(context)

            if (paired) {
                val host = prefs.host ?: return
                val port = prefs.port
                val fp = prefs.fp ?: return
                viewModelScope.launch {
                    val alive = withContext(Dispatchers.IO) {
                        PairingApi().ping(host, port, fp)
                    }
                    _state.value = _state.value.copy(
                        status = if (alive) ConnectionStatus.Connected(host)
                                 else ConnectionStatus.Disconnected
                    )
                }
            }
        } catch (t: Throwable) {
            Log.e(TAG, "bootstrap failed reading prefs", t)
        }
    }

    fun setMode(mode: String) {
        _state.value = _state.value.copy(mode = mode)
    }

    fun setOverlayEnabled(context: Context, enabled: Boolean) {
        val prefs = Prefs(context)
        prefs.overlayEnabled = enabled
        _state.value = _state.value.copy(overlayEnabled = enabled)
        ClipForegroundService.updateOverlay(context)
    }

    fun setAutoSendEnabled(context: Context, enabled: Boolean) {
        val prefs = Prefs(context)
        prefs.autoSendEnabled = enabled
        _state.value = _state.value.copy(autoSendEnabled = enabled)
    }

    fun startSync(context: Context) {
        val prefs = Prefs(context)
        prefs.syncEnabled = true
        val host = prefs.host ?: ""
        _state.value = _state.value.copy(syncEnabled = true, status = ConnectionStatus.Connected(host))
        ClipForegroundService.start(context)
    }

    fun stopSync(context: Context) {
        val prefs = Prefs(context)
        prefs.syncEnabled = false
        _state.value = _state.value.copy(syncEnabled = false, status = ConnectionStatus.Disconnected)
        ClipForegroundService.stop(context)
    }

    fun startDiscovery(context: Context) {
        discoveryJob?.cancel()
        val nsd = NsdDiscovery(context)
        discoveryJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                nsd.discover().collect { d ->
                    val merged = (_state.value.discovered + d).distinctBy { it.name }
                    _state.value = _state.value.copy(discovered = merged)
                }
            } catch (t: Throwable) {
                Log.w(TAG, "discovery crashed: ${t.message}")
            }
        }
    }

    fun pair(context: Context, target: PairingTarget, code: String) {
        viewModelScope.launch {
            _state.value = _state.value.copy(status = ConnectionStatus.Connecting, error = null)
            try {
                val prefs = Prefs(context)
                val api = PairingApi()

                when (target) {
                    is PairingTarget.Auto -> {
                        val d = target.discovered
                        val fp = d.fp
                        if (fp.isNullOrEmpty()) {
                            // mDNS did not provide fp — fall back to TOFU.
                            val resp = withContext(Dispatchers.IO) {
                                api.pairWithTofu(d.host, d.port, code)
                            }
                            persistAndStart(context, prefs, d.host, d.port, resp.token, resp.fpBase64Url, resp.secret, Prefs.MODE_AUTO)
                        } else {
                            val resp = withContext(Dispatchers.IO) {
                                api.pairWithKnownFp(d.host, d.port, code, fp)
                            }
                            persistAndStart(context, prefs, d.host, d.port, resp.token, fp, resp.secret, Prefs.MODE_AUTO)
                        }
                    }
                    is PairingTarget.Manual -> {
                        val resp = withContext(Dispatchers.IO) {
                            api.pairWithTofu(target.host, target.port, code)
                        }
                        persistAndStart(context, prefs, target.host, target.port, resp.token, resp.fpBase64Url, resp.secret, Prefs.MODE_MANUAL)
                    }
                }
            } catch (t: Throwable) {
                Log.e(TAG, "pair failed", t)
                _state.value = _state.value.copy(
                    status = ConnectionStatus.Error(t.message ?: "unknown"),
                    error = t.message
                )
            }
        }
    }

    private fun persistAndStart(
        context: Context,
        prefs: Prefs,
        host: String,
        port: Int,
        token: String,
        fp: String,
        pairingSecret: String,
        mode: String
    ) {
        prefs.host = host
        prefs.port = port
        prefs.token = token
        prefs.fp = fp
        prefs.pairingSecret = pairingSecret
        prefs.mode = mode
        prefs.syncEnabled = true
        _state.value = _state.value.copy(
            syncEnabled = true,
            hasPairing = true,
            pairedHost = host,
            pairedPort = port,
            status = ConnectionStatus.Connected(host),
            error = null
        )
        ClipForegroundService.start(context)
    }

    fun refreshShizukuState(context: Context) {
        val installed = try {
            @Suppress("DEPRECATION")
            context.packageManager.getPackageInfo("moe.shizuku.privileged.api", 0)
            true
        } catch (_: PackageManager.NameNotFoundException) {
            false
        }

        val state = if (!installed) {
            "not_installed"
        } else if (!Shizuku.pingBinder()) {
            "not_running"
        } else if (Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED) {
            "no_permission"
        } else {
            "ready"
        }
        _state.value = _state.value.copy(shizukuState = state)
    }

    fun stopShizukuListener(context: Context) {
        val i = Intent(context, ClipForegroundService::class.java).apply {
            action = ClipForegroundService.ACTION_STOP_SHIZUKU
        }
        try {
            if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.O) {
                context.startForegroundService(i)
            } else {
                context.startService(i)
            }
        } catch (_: Exception) {}
        _state.value = _state.value.copy(shizukuState = "stopped")
    }

    fun requestShizukuPermission() {
        try {
            if (!Shizuku.pingBinder()) return
            val listener = object : Shizuku.OnRequestPermissionResultListener {
                override fun onRequestPermissionResult(requestCode: Int, grantResult: Int) {
                    if (requestCode == ShizukuClipboardManager.PERMISSION_REQUEST_CODE) {
                        _state.value = _state.value.copy(
                            shizukuState = if (grantResult == android.content.pm.PackageManager.PERMISSION_GRANTED)
                                "ready" else "no_permission"
                        )
                        Shizuku.removeRequestPermissionResultListener(this)
                    }
                }
            }
            Shizuku.addRequestPermissionResultListener(listener)
            Shizuku.requestPermission(ShizukuClipboardManager.PERMISSION_REQUEST_CODE)
        } catch (e: Exception) {
            Log.w(TAG, "requestShizukuPermission failed: ${e.message}")
        }
    }

    companion object {
        private const val TAG = "ClipSync/VM"
    }
}
