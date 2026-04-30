package com.clipsync.ui

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.viewmodel.compose.viewModel
import com.clipsync.model.ErrorAction
import com.clipsync.storage.Prefs
import com.clipsync.ui.components.ErrorBanner
import com.clipsync.ui.sections.ConnectionSection
import com.clipsync.ui.sections.PermissionRow
import com.clipsync.ui.sections.PermissionsSection
import com.clipsync.ui.sections.TailscaleSection
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuColors
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.sp
import android.text.TextUtils
import kotlinx.coroutines.delay

@Composable
fun SettingsScreen(
    isDark: Boolean = true,
    deepLinkUri: android.net.Uri? = null,
    onToggleTheme: (cx: Float, cy: Float) -> Unit = { _, _ -> },
    vm: SettingsViewModel = viewModel(),
) {
    val context = LocalContext.current
    val state by vm.state.collectAsState()
    var pairingTarget by remember { mutableStateOf<PairingTarget?>(null) }

    val mediaPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted -> vm.onMediaPermissionResult(granted) }

    LaunchedEffect(Unit) { vm.bootstrap(context) }

    LaunchedEffect(deepLinkUri) {
        val uri = deepLinkUri ?: return@LaunchedEffect
        if (uri.scheme == "clipsync" && uri.host == "pair") {
            val host = uri.getQueryParameter("host") ?: return@LaunchedEffect
            val port = uri.getQueryParameter("port")?.toIntOrNull() ?: Prefs.DEFAULT_PORT
            val code = uri.getQueryParameter("code") ?: return@LaunchedEffect
            vm.pair(context, PairingTarget.Manual(host, port), code)
        }
    }

    val lifecycleOwner = context as? androidx.lifecycle.LifecycleOwner
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) vm.refreshOnResume(context)
        }
        lifecycleOwner?.lifecycle?.addObserver(observer)
        onDispose { lifecycleOwner?.lifecycle?.removeObserver(observer) }
    }

    val shizukuReady = state.shizukuState == "ready"
    // Step 0a: On mobile data + Tailscale not installed → suggest Tailscale
    val showTailscaleModal = state.isOnMobileData
            && state.tailscaleState is TailscaleState.NotInstalled
            && !state.hasPairing
    // Step 0b: Shizuku not installed → blocking install modal
    val showInstallModal = !showTailscaleModal && state.shizukuState == "not_installed"
    // Step 1: Permissions modal — Media access + Shizuku both required
    val showPermissionsModal = !showTailscaleModal && !showInstallModal && (!state.mediaPermissionGranted || !shizukuReady)

    // Modal 0a — Tailscale not installed while on mobile data
    var tailscaleDismissed by rememberSaveable { mutableStateOf(false) }
    if (showTailscaleModal && !tailscaleDismissed) {
        AlertDialog(
            onDismissRequest = { tailscaleDismissed = true },
            containerColor = NeuColors.SurfaceRaised,
            title = { Text("Setup Tailscale", style = MaterialTheme.typography.headlineMedium) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        "You're on mobile data. To sync with your Mac from outside your home WiFi, install Tailscale on both devices.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Text(
                        "Tailscale is free and creates a secure network between your devices.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            },
            confirmButton = {
                NeuButton(onClick = { vm.openTailscalePlayStore(context) }, isAccent = true) {
                    Text("Install Tailscale", color = NeuColors.Accent)
                }
            },
            dismissButton = {
                TextButton(onClick = { tailscaleDismissed = true }) {
                    Text("Skip", color = NeuColors.TextSecondary)
                }
            },
        )
    }

    // Modal 0b — Download & install Shizuku (blocking)
    if (showInstallModal) {
        val install = state.shizukuInstall
        AlertDialog(
            onDismissRequest = {},
            containerColor = NeuColors.SurfaceRaised,
            title = { Text("Install Shizuku", style = MaterialTheme.typography.headlineMedium) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        "Shizuku is required for ClipSync to access the clipboard on Android 12+. " +
                        "It's free and open-source.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Text(
                        "After installing, open Shizuku and start its service via Wireless Debugging or ADB.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                    when (install) {
                        is ShizukuInstallState.Fetching -> {
                            Text("Fetching latest version…", style = MaterialTheme.typography.bodySmall)
                            androidx.compose.material3.LinearProgressIndicator(
                                modifier = Modifier.fillMaxWidth()
                            )
                        }
                        is ShizukuInstallState.Downloading -> {
                            Text("Downloading… ${install.progress}%", style = MaterialTheme.typography.bodySmall)
                            @Suppress("DEPRECATION")
                            androidx.compose.material3.LinearProgressIndicator(
                                progress = install.progress / 100f,
                                modifier = Modifier.fillMaxWidth(),
                            )
                        }
                        is ShizukuInstallState.Error -> {
                            Text(
                                "Error: ${install.message}",
                                style = MaterialTheme.typography.bodySmall,
                                color = NeuColors.Error,
                            )
                        }
                        else -> {}
                    }
                }
            },
            confirmButton = {
                when (install) {
                    is ShizukuInstallState.Idle, is ShizukuInstallState.Error ->
                        NeuButton(onClick = { vm.downloadShizuku(context) }, isAccent = true) {
                            Text(
                                if (install is ShizukuInstallState.Error) "Retry" else "Download",
                                color = NeuColors.Accent,
                            )
                        }
                    is ShizukuInstallState.ReadyToInstall ->
                        NeuButton(onClick = { vm.installShizuku(context, install.file) }, isAccent = true) {
                            Text("Install now", color = NeuColors.Accent)
                        }
                    else -> {}  // Fetching / Downloading — no button
                }
            },
        )
    }

    // Modal 1 — Permissions (blocking): Media access + Shizuku
    if (showPermissionsModal) {
        AlertDialog(
            onDismissRequest = {},
            containerColor = NeuColors.SurfaceRaised,
            title = { Text("Permissions required", style = MaterialTheme.typography.headlineMedium) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text(
                        "Grant these permissions so ClipSync can sync your clipboard.",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                    Spacer(Modifier.height(4.dp))
                    PermissionRow(
                        label = "Media access",
                        granted = state.mediaPermissionGranted,
                        onGrant = {
                            val perm = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU)
                                android.Manifest.permission.READ_MEDIA_IMAGES
                            else
                                android.Manifest.permission.READ_EXTERNAL_STORAGE
                            mediaPermissionLauncher.launch(perm)
                        },
                        onRevoke = {
                            context.startActivity(
                                Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                                    data = Uri.parse("package:${context.packageName}")
                                }
                            )
                        },
                    )
                    PermissionRow(
                        label = "Shizuku",
                        granted = shizukuReady,
                        onGrant = {
                            when (state.shizukuState) {
                                "not_running" -> {
                                    val launch = context.packageManager
                                        .getLaunchIntentForPackage("moe.shizuku.privileged.api")
                                    context.startActivity(
                                        launch ?: Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS)
                                    )
                                }
                                "no_permission" -> vm.requestShizukuPermission()
                                else -> {}
                            }
                        },
                        grantLabel = when (state.shizukuState) {
                            "not_running" -> "Open Shizuku"
                            "no_permission" -> "Grant"
                            else -> "Setup"
                        },
                        onRevoke = {
                            context.packageManager
                                .getLaunchIntentForPackage("moe.shizuku.privileged.api")
                                ?.let { context.startActivity(it) }
                        },
                        revokeLabel = "Manage",
                    )
                }
            },
            confirmButton = {},
        )
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(NeuColors.Background)
            .windowInsetsPadding(WindowInsets.safeDrawing)
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            // Header
            Spacer(Modifier.height(2.dp))
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween,
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    "ClipSync",
                    style = MaterialTheme.typography.headlineLarge,
                    fontWeight = FontWeight.Bold,
                )
                var themeBtnCoords by remember { mutableStateOf<LayoutCoordinates?>(null) }
                IconButton(
                    onClick = {
                        themeBtnCoords?.let { c ->
                            val pos = c.positionInWindow()
                            onToggleTheme(
                                pos.x + c.size.width / 2f,
                                pos.y + c.size.height / 2f,
                            )
                        } ?: onToggleTheme(0f, 0f)
                    },
                    modifier = Modifier
                        .size(40.dp)
                        .clip(CircleShape)
                        .background(NeuColors.SurfaceRaised)
                        .onGloballyPositioned { themeBtnCoords = it },
                ) {
                    Icon(
                        imageVector = if (isDark) Icons.Filled.LightMode else Icons.Filled.DarkMode,
                        contentDescription = if (isDark) "Light mode" else "Dark mode",
                        tint = NeuColors.Accent,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
            Spacer(Modifier.height(8.dp))

            // Connection Mode, status card, discovered servers / manual input
            ConnectionSection(
                state = state,
                vm = vm,
                context = context,
                onPairingTarget = { pairingTarget = it },
            )

            // Tailscale status
            TailscaleSection(state = state, vm = vm, context = context)

            // Clipboard Access, Features, Permissions
            PermissionsSection(state = state, vm = vm, context = context)

            // Error banners
            state.errors.forEach { error ->
                ErrorBanner(
                    error = error,
                    onDismiss = { vm.dismissError(error.id) },
                    onAction = { action ->
                        when (action) {
                            is ErrorAction.Retry -> {
                                vm.dismissError(error.id)
                                vm.bootstrap(context)
                            }
                            is ErrorAction.Repair -> {
                                vm.dismissError(error.id)
                                vm.unpair(context)
                            }
                            is ErrorAction.OpenUrl -> {
                                val intent = Intent(Intent.ACTION_VIEW, Uri.parse(action.url))
                                context.startActivity(intent)
                            }
                        }
                    },
                )
            }

            Spacer(Modifier.height(16.dp))
        }
    }

    // Pairing dialog
    pairingTarget?.let { target ->
        PairingCodeDialog(
            target = target,
            onDismiss = { pairingTarget = null },
            onConfirm = { code ->
                pairingTarget = null
                vm.pair(context, target, code)
            },
        )
    }
}

@Composable
private fun PairingCodeDialog(
    target: PairingTarget,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit,
) {
    var code by remember { mutableStateOf("") }
    var remainingSeconds by remember { mutableIntStateOf(120) }
    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(Unit) { focusRequester.requestFocus() }
    LaunchedEffect(Unit) {
        while (remainingSeconds > 0) {
            delay(1000)
            remainingSeconds--
        }
        onDismiss()
    }
    val hostLabel = when (target) {
        is PairingTarget.Auto -> "${target.discovered.host}:${target.discovered.port}"
        is PairingTarget.Manual -> "${target.host}:${target.port}"
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NeuColors.SurfaceRaised,
        title = {
            Text(
                "Enter pairing code",
                style = MaterialTheme.typography.headlineMedium,
            )
        },
        text = {
            Column {
                Text(
                    hostLabel,
                    style = MaterialTheme.typography.bodyMedium,
                )
                Spacer(Modifier.height(4.dp))
                Text(
                    "Code expires in ${remainingSeconds}s",
                    style = MaterialTheme.typography.bodySmall,
                    color = if (remainingSeconds <= 30) NeuColors.Error else NeuColors.TextSecondary,
                )
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = code,
                    onValueChange = { code = it.filter { c -> c.isDigit() }.take(6) },
                    label = { Text("6-digit code") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                    keyboardActions = KeyboardActions(onDone = { if (code.length == 6) onConfirm(code) }),
                    supportingText = {
                        Text("${code.length}/6 digits")
                    },
                    textStyle = MaterialTheme.typography.headlineMedium.copy(
                        letterSpacing = 8.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .focusRequester(focusRequester),
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(code) },
                enabled = code.length == 6,
            ) {
                Text(
                    "Pair",
                    color = if (code.length == 6) NeuColors.Accent else NeuColors.TextSecondary,
                    fontWeight = FontWeight.Bold,
                )
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) {
                Text("Cancel", color = NeuColors.TextSecondary)
            }
        },
    )
}

sealed class PairingTarget {
    data class Auto(val discovered: com.clipsync.discovery.Discovered) : PairingTarget()
    data class Manual(val host: String, val port: Int) : PairingTarget()
}

private fun isAccessibilityServiceEnabled(context: android.content.Context): Boolean {
    val service = "${context.packageName}/.accessibility.ClipAccessibilityService"
    val enabledServices = Settings.Secure.getString(
        context.contentResolver,
        Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
    ) ?: return false
    val splitter = TextUtils.SimpleStringSplitter(':')
    splitter.setString(enabledServices)
    while (splitter.hasNext()) {
        if (splitter.next().equals(service, ignoreCase = true)) return true
    }
    return false
}
