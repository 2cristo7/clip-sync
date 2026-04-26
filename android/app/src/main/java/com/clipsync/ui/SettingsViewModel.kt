package com.clipsync.ui

import android.Manifest
import android.content.Context
import android.net.Uri
import android.os.Build
import androidx.core.content.FileProvider
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import com.clipsync.discovery.Discovered
import com.clipsync.discovery.NsdDiscovery
import com.clipsync.net.PairingApi
import android.content.Intent
import android.provider.Settings
import com.clipsync.service.ClipForegroundService
import com.clipsync.shizuku.ShizukuClipboardManager
import com.clipsync.storage.Prefs
import com.clipsync.util.L
import okhttp3.OkHttpClient
import okhttp3.Request
import org.json.JSONObject
import rikka.shizuku.Shizuku
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import java.io.File

sealed class ShizukuInstallState {
    data object Idle : ShizukuInstallState()
    data object Fetching : ShizukuInstallState()
    data class Downloading(val progress: Int) : ShizukuInstallState()  // 0–100
    data class ReadyToInstall(val file: File) : ShizukuInstallState()
    data class Error(val message: String) : ShizukuInstallState()
}

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
    val syncEnabled: Boolean = true,
    val autoSendEnabled: Boolean = true,
    val mediaPermissionGranted: Boolean = false,
    val notificationPermissionGranted: Boolean = false,
    val shizukuState: String = "not_checked",
    val shizukuInstall: ShizukuInstallState = ShizukuInstallState.Idle,
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

            if (paired) L.event(M, "bootstrap hasPairing=true host=${prefs.host} syncEnabled=${prefs.syncEnabled}")
            else L.event(M, "bootstrap hasPairing=false")

            _state.value = _state.value.copy(
                mode = prefs.mode,
                syncEnabled = prefs.syncEnabled,
                autoSendEnabled = prefs.autoSendEnabled,
                mediaPermissionGranted = hasMediaPermission(context),
                notificationPermissionGranted = hasNotificationPermission(context),
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
            L.error(M, "bootstrap failed reading prefs", t)
        }
    }

    fun setMode(mode: String) {
        L.action(M, "setMode mode=$mode")
        _state.value = _state.value.copy(mode = mode)
    }

    fun setAutoSendEnabled(context: Context, enabled: Boolean) {
        L.action(M, "setAutoSendEnabled enabled=$enabled")
        val prefs = Prefs(context)
        prefs.autoSendEnabled = enabled
        _state.value = _state.value.copy(autoSendEnabled = enabled)
    }

    fun startSync(context: Context) {
        L.action(M, "startSync")
        val prefs = Prefs(context)
        prefs.syncEnabled = true
        val host = prefs.host ?: ""
        _state.value = _state.value.copy(syncEnabled = true, status = ConnectionStatus.Connected(host))
        ClipForegroundService.start(context)
    }

    fun stopSync(context: Context) {
        L.action(M, "stopSync")
        val prefs = Prefs(context)
        prefs.syncEnabled = false
        _state.value = _state.value.copy(syncEnabled = false, status = ConnectionStatus.Disconnected)
        ClipForegroundService.stop(context)
    }

    fun unpair(context: Context) {
        L.action(M, "unpair host=${_state.value.pairedHost}")
        Prefs(context).clearPairing()
        ClipForegroundService.stop(context)
        _state.value = _state.value.copy(
            hasPairing = false,
            pairedHost = null,
            pairedPort = 7010,
            syncEnabled = false,
            status = ConnectionStatus.Disconnected,
            error = null
        )
    }

    fun startDiscovery(context: Context) {
        discoveryJob?.cancel()
        val nsd = NsdDiscovery(context)
        discoveryJob = viewModelScope.launch(Dispatchers.IO) {
            try {
                nsd.discover().collect { d ->
                    val isNew = _state.value.discovered.none { it.name == d.name }
                    if (isNew) L.event(M, "discovery found name=${d.name} host=${d.host}:${d.port}")
                    val merged = (_state.value.discovered + d).distinctBy { it.name }
                    _state.value = _state.value.copy(discovered = merged)
                }
            } catch (t: Throwable) {
                L.warn(M, "discovery crashed: ${t.message}")
            }
        }
    }

    fun pair(context: Context, target: PairingTarget, code: String) {
        val targetLabel = when (target) {
            is PairingTarget.Auto -> "${target.discovered.host}:${target.discovered.port}"
            is PairingTarget.Manual -> "${target.host}:${target.port}"
        }
        L.action(M, "pair target=$targetLabel")
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
                L.error(M, "pair failed", t)
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
        L.action(M, "pairSuccess host=$host port=$port mode=$mode")
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
        val prev = _state.value.shizukuState
        if (state != prev) L.perm(M, "shizuku state=$state prev=$prev")
        _state.value = _state.value.copy(shizukuState = state)
    }

    fun refreshOnResume(context: Context) {
        val mediaGranted = hasMediaPermission(context)
        val notifGranted = hasNotificationPermission(context)
        _state.value = _state.value.copy(
            mediaPermissionGranted = mediaGranted,
            notificationPermissionGranted = notifGranted,
        )
        refreshShizukuState(context)
    }

    fun onMediaPermissionResult(granted: Boolean) {
        _state.value = _state.value.copy(mediaPermissionGranted = granted)
    }

    fun onNotificationPermissionResult(granted: Boolean) {
        _state.value = _state.value.copy(notificationPermissionGranted = granted)
    }

    private fun hasNotificationPermission(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(context, Manifest.permission.POST_NOTIFICATIONS) ==
                PackageManager.PERMISSION_GRANTED
        } else true
    }

    private fun hasMediaPermission(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_MEDIA_IMAGES) ==
                PackageManager.PERMISSION_GRANTED
        } else {
            ContextCompat.checkSelfPermission(context, Manifest.permission.READ_EXTERNAL_STORAGE) ==
                PackageManager.PERMISSION_GRANTED
        }
    }

    fun downloadShizuku(context: Context) {
        // Ask for "install unknown apps" permission before starting the download,
        // so the user doesn't wait for the full download only to hit a permission wall.
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            if (!context.packageManager.canRequestPackageInstalls()) {
                context.startActivity(
                    Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES).apply {
                        data = Uri.parse("package:${context.packageName}")
                        flags = Intent.FLAG_ACTIVITY_NEW_TASK
                    }
                )
                return  // User must come back and tap Download again once permission is granted
            }
        }
        viewModelScope.launch {
            _state.value = _state.value.copy(shizukuInstall = ShizukuInstallState.Fetching)
            try {
                val apkUrl = withContext(Dispatchers.IO) { fetchLatestShizukuApkUrl() }
                _state.value = _state.value.copy(shizukuInstall = ShizukuInstallState.Downloading(0))
                val file = withContext(Dispatchers.IO) {
                    downloadApk(context, apkUrl) { progress ->
                        _state.value = _state.value.copy(
                            shizukuInstall = ShizukuInstallState.Downloading(progress)
                        )
                    }
                }
                _state.value = _state.value.copy(shizukuInstall = ShizukuInstallState.ReadyToInstall(file))
            } catch (t: Throwable) {
                L.warn(M, "Shizuku download failed: ${t.message}")
                _state.value = _state.value.copy(shizukuInstall = ShizukuInstallState.Error(t.message ?: "Download failed"))
            }
        }
    }

    fun installShizuku(context: Context, file: File) {
        val uri = FileProvider.getUriForFile(context, "com.clipsync.fileprovider", file)
        context.startActivity(
            Intent(Intent.ACTION_VIEW).apply {
                setDataAndType(uri, "application/vnd.android.package-archive")
                flags = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_ACTIVITY_NEW_TASK
            }
        )
    }

    fun resetShizukuInstall() {
        _state.value = _state.value.copy(shizukuInstall = ShizukuInstallState.Idle)
    }

    private fun fetchLatestShizukuApkUrl(): String {
        val client = OkHttpClient()
        val req = Request.Builder()
            .url("https://api.github.com/repos/RikkaApps/Shizuku/releases/latest")
            .addHeader("Accept", "application/vnd.github.v3+json")
            .build()
        val body = client.newCall(req).execute().use { it.body?.string() }
            ?: throw Exception("Empty response from GitHub API")
        val assets = JSONObject(body).getJSONArray("assets")
        for (i in 0 until assets.length()) {
            val asset = assets.getJSONObject(i)
            if (asset.getString("name").endsWith(".apk")) {
                return asset.getString("browser_download_url")
            }
        }
        throw Exception("No APK asset found in latest Shizuku release")
    }

    private fun downloadApk(context: Context, url: String, onProgress: (Int) -> Unit): File {
        val client = OkHttpClient()
        val response = client.newCall(Request.Builder().url(url).build()).execute()
        val body = response.body ?: throw Exception("Empty download body")
        val length = body.contentLength()
        val dir = File(context.cacheDir, "clipsync").also { it.mkdirs() }
        val file = File(dir, "shizuku.apk")
        var downloaded = 0L
        file.outputStream().use { out ->
            body.byteStream().use { input ->
                val buf = ByteArray(8192)
                var read: Int
                while (input.read(buf).also { read = it } != -1) {
                    out.write(buf, 0, read)
                    downloaded += read
                    if (length > 0) onProgress((downloaded * 100 / length).toInt())
                }
            }
        }
        return file
    }

    fun requestShizukuPermission() {
        L.action(M, "requestShizukuPermission")
        try {
            if (!Shizuku.pingBinder()) return
            val listener = object : Shizuku.OnRequestPermissionResultListener {
                override fun onRequestPermissionResult(requestCode: Int, grantResult: Int) {
                    if (requestCode == ShizukuClipboardManager.PERMISSION_REQUEST_CODE) {
                        val granted = grantResult == PackageManager.PERMISSION_GRANTED
                        L.perm(M, "shizukuPermissionResult granted=$granted")
                        _state.value = _state.value.copy(
                            shizukuState = if (granted) "ready" else "no_permission"
                        )
                        Shizuku.removeRequestPermissionResultListener(this)
                    }
                }
            }
            Shizuku.addRequestPermissionResultListener(listener)
            Shizuku.requestPermission(ShizukuClipboardManager.PERMISSION_REQUEST_CODE)
        } catch (e: Exception) {
            L.warn(M, "requestShizukuPermission failed: ${e.message}")
        }
    }

    companion object {
        private const val M = "VM"
    }
}
