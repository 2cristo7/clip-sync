package com.clipsync.ui

import android.content.Context
import android.util.Log
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import com.clipsync.discovery.Discovered
import com.clipsync.discovery.NsdDiscovery
import com.clipsync.net.PairingApi
import com.clipsync.service.ClipForegroundService
import com.clipsync.storage.Prefs
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
    data class Error(val reason: String) : ConnectionStatus()
}

data class SettingsState(
    val mode: String = Prefs.MODE_AUTO,
    val discovered: List<Discovered> = emptyList(),
    val status: ConnectionStatus = ConnectionStatus.Disconnected,
    val error: String? = null
)

class SettingsViewModel : ViewModel() {

    private val _state = MutableStateFlow(SettingsState())
    val state: StateFlow<SettingsState> = _state.asStateFlow()

    private var discoveryJob: Job? = null

    fun bootstrap(context: Context) {
        val prefs = Prefs(context)
        _state.value = _state.value.copy(
            mode = prefs.mode,
            status = if (prefs.hasPairing()) ConnectionStatus.Connected(prefs.host ?: "")
                     else ConnectionStatus.Disconnected
        )
        if (prefs.mode == Prefs.MODE_AUTO) startDiscovery(context)
    }

    fun setMode(mode: String) {
        _state.value = _state.value.copy(mode = mode)
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
                            persistAndStart(context, prefs, d.host, d.port, resp.token, resp.fpBase64Url, Prefs.MODE_AUTO)
                        } else {
                            val resp = withContext(Dispatchers.IO) {
                                api.pairWithKnownFp(d.host, d.port, code, fp)
                            }
                            persistAndStart(context, prefs, d.host, d.port, resp.token, fp, Prefs.MODE_AUTO)
                        }
                    }
                    is PairingTarget.Manual -> {
                        val resp = withContext(Dispatchers.IO) {
                            api.pairWithTofu(target.host, target.port, code)
                        }
                        persistAndStart(context, prefs, target.host, target.port, resp.token, resp.fpBase64Url, Prefs.MODE_MANUAL)
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
        mode: String
    ) {
        prefs.host = host
        prefs.port = port
        prefs.token = token
        prefs.fp = fp
        prefs.mode = mode
        _state.value = _state.value.copy(status = ConnectionStatus.Connected(host), error = null)
        ClipForegroundService.start(context)
    }

    companion object {
        private const val TAG = "ClipSync/VM"
    }
}
