package com.clipsync.ui

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.text.TextUtils
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.expandVertically
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.shrinkVertically
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
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.safeDrawing
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.DarkMode
import androidx.compose.material.icons.filled.LightMode
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.clipsync.discovery.Discovered
import com.clipsync.storage.Prefs
import com.clipsync.ui.ShizukuInstallState
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuCard
import com.clipsync.ui.theme.NeuColors
import com.clipsync.ui.theme.NeuSectionHeader
import com.clipsync.ui.theme.NeuSegmentedToggle
import com.clipsync.ui.theme.NeuStatusBadge

@Composable
fun SettingsScreen(
    isDark: Boolean = true,
    onToggleTheme: () -> Unit = {},
    vm: SettingsViewModel = viewModel(),
) {
    val context = LocalContext.current
    val state by vm.state.collectAsState()
    var pairingTarget by remember { mutableStateOf<PairingTarget?>(null) }
    var manualHost by rememberSaveable { mutableStateOf("") }
    var manualPort by rememberSaveable { mutableStateOf("7010") }

    val mediaPermissionLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.RequestPermission()
    ) { granted -> vm.onMediaPermissionResult(granted) }

    LaunchedEffect(Unit) { vm.bootstrap(context) }

    val lifecycleOwner = context as? androidx.lifecycle.LifecycleOwner
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_RESUME) vm.refreshOnResume(context)
        }
        lifecycleOwner?.lifecycle?.addObserver(observer)
        onDispose { lifecycleOwner?.lifecycle?.removeObserver(observer) }
    }

    val shizukuReady = state.shizukuState == "ready"
    // Step 0: Shizuku not installed → blocking install modal
    val showInstallModal = state.shizukuState == "not_installed"
    // Step 1: Permissions modal — Media access + Shizuku both required
    val showPermissionsModal = !showInstallModal && (!state.mediaPermissionGranted || !shizukuReady)

    // Modal 0 — Download & install Shizuku (blocking)
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
                                color = NeuColors.TextOnAccent,
                            )
                        }
                    is ShizukuInstallState.ReadyToInstall ->
                        NeuButton(onClick = { vm.installShizuku(context, install.file) }, isAccent = true) {
                            Text("Install now", color = NeuColors.TextOnAccent)
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
            verticalArrangement = Arrangement.spacedBy(16.dp)
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
                IconButton(
                    onClick = onToggleTheme,
                    modifier = Modifier
                        .size(40.dp)
                        .clip(CircleShape)
                        .background(NeuColors.SurfaceRaised)
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

            // Status card
            NeuCard {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    Row(
                        verticalAlignment = Alignment.CenterVertically,
                        modifier = Modifier.fillMaxWidth()
                    ) {
                        StatusDot(state.status)
                        Spacer(Modifier.width(12.dp))
                        Column(Modifier.weight(1f)) {
                            Text(
                                statusTitle(state.status),
                                style = MaterialTheme.typography.titleMedium,
                            )
                            Text(
                                statusSubtitle(state.status, state.hasPairing),
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                        if (state.shizukuState == "ready") {
                            Spacer(Modifier.width(8.dp))
                            NeuStatusBadge(label = "Shizuku", color = NeuColors.Connected)
                        }
                    }

                    if (state.hasPairing) {
                        val isActive = state.status is ConnectionStatus.Connected ||
                            state.status is ConnectionStatus.Connecting
                        if (!isActive) {
                            NeuButton(
                                onClick = { vm.startSync(context) },
                                modifier = Modifier.fillMaxWidth(),
                                isAccent = true
                            ) {
                                Text("Connect", color = NeuColors.TextOnAccent)
                            }
                        } else {
                            NeuButton(
                                onClick = { vm.stopSync(context) },
                                modifier = Modifier.fillMaxWidth(),
                                isAccent = false
                            ) {
                                Text("Stop Sync", color = NeuColors.Error)
                            }
                        }
                    }
                }
            }

            // Mode selector
            NeuSectionHeader("Connection Mode")
            NeuSegmentedToggle(
                options = listOf("Auto (mDNS)", "Manual IP"),
                selectedIndex = if (state.mode == Prefs.MODE_AUTO) 0 else 1,
                onSelected = { idx ->
                    vm.setMode(if (idx == 0) Prefs.MODE_AUTO else Prefs.MODE_MANUAL)
                }
            )

            // Discovery / Manual section
            AnimatedVisibility(
                visible = state.mode == Prefs.MODE_AUTO,
                enter = fadeIn() + expandVertically(clip = false),
                exit = fadeOut() + shrinkVertically(clip = false)
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    NeuSectionHeader("Discovered Servers")
                    if (state.discovered.isEmpty()) {
                        NeuCard {
                            Row(verticalAlignment = Alignment.CenterVertically) {
                                Text("🔍", fontSize = 20.sp)
                                Spacer(Modifier.width(8.dp))
                                Text(
                                    "Searching for servers on your network…",
                                    style = MaterialTheme.typography.bodyMedium
                                )
                            }
                        }
                    } else {
                        state.discovered.forEach { d ->
                            val isPaired = d.host == state.pairedHost && d.port == state.pairedPort
                            val isActive = state.status is ConnectionStatus.Connected ||
                                state.status is ConnectionStatus.Connecting
                            DiscoveredServerCard(
                                d = d,
                                isPaired = isPaired,
                                canUnpair = isPaired && !isActive,
                                onPair = { if (!isPaired) pairingTarget = PairingTarget.Auto(d) },
                                onUnpair = { vm.unpair(context) }
                            )
                        }
                    }
                }
            }

            AnimatedVisibility(
                visible = state.mode == Prefs.MODE_MANUAL,
                enter = fadeIn() + expandVertically(clip = false),
                exit = fadeOut() + shrinkVertically(clip = false)
            ) {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    NeuSectionHeader("Manual Connection")
                    NeuCard {
                        Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                            OutlinedTextField(
                                value = manualHost,
                                onValueChange = { manualHost = it },
                                label = { Text("Host / IP") },
                                singleLine = true,
                                modifier = Modifier.fillMaxWidth()
                            )
                            OutlinedTextField(
                                value = manualPort,
                                onValueChange = { manualPort = it.filter { c -> c.isDigit() }.take(5) },
                                label = { Text("Port") },
                                singleLine = true,
                                modifier = Modifier.fillMaxWidth()
                            )
                            NeuButton(
                                onClick = {
                                    pairingTarget = PairingTarget.Manual(
                                        host = manualHost.trim(),
                                        port = manualPort.toIntOrNull() ?: 7010
                                    )
                                },
                                enabled = manualHost.isNotBlank() && manualPort.isNotBlank(),
                                isAccent = true,
                            ) {
                                Text("Pair", color = NeuColors.TextOnAccent)
                            }
                        }
                    }
                }
            }

            // Clipboard access (Shizuku)
            NeuSectionHeader("Clipboard Access")
            NeuCard {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    NeuStatusBadge(
                        label = if (shizukuReady) "Shizuku active" else "Shizuku inactive",
                        color = if (shizukuReady) NeuColors.Connected else NeuColors.TextSecondary,
                    )
                    Text(
                        if (shizukuReady)
                            "Direct clipboard access is active. Starts and stops with the Mac connection."
                        else
                            "Shizuku is not active. Go to Permissions to set it up.",
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }

            // Toggles
            NeuSectionHeader("Features")
            NeuCard {
                Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
                    ToggleRow(
                        title = "Auto send on copy",
                        subtitle = "Sends clipboard to Mac as soon as you copy",
                        checked = state.autoSendEnabled,
                        onCheckedChange = { vm.setAutoSendEnabled(context, it) },
                    )
                }
            }

            // Permissions info
            NeuSectionHeader("Permissions")
            NeuCard {
                Column(verticalArrangement = Arrangement.spacedBy(14.dp)) {
                    PermissionInfoRow(
                        label = "Notifications",
                        description = "Required for the sync service to run in the background",
                        granted = state.notificationPermissionGranted,
                        onManage = {
                            context.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                                putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                            })
                        }
                    )
                    PermissionInfoRow(
                        label = "Media access",
                        description = "Required for sending screenshots to Mac",
                        granted = state.mediaPermissionGranted,
                        onManage = {
                            context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                                data = Uri.parse("package:${context.packageName}")
                            })
                        }
                    )
                }
            }

            // Error message
            state.error?.let { error ->
                NeuCard {
                    Text(
                        "⚠️ $error",
                        color = NeuColors.Error,
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
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
            }
        )
    }
}

@Composable
private fun PermissionRow(
    label: String,
    granted: Boolean,
    onGrant: () -> Unit,
    grantLabel: String = "Grant",
    onRevoke: (() -> Unit)? = null,
    revokeLabel: String = "Manage",
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Text(label, style = MaterialTheme.typography.bodyLarge)
        if (granted) {
            if (onRevoke != null) {
                NeuButton(onClick = onRevoke, isAccent = false) {
                    Text(revokeLabel, color = NeuColors.TextSecondary)
                }
            } else {
                Icon(
                    Icons.Filled.Check,
                    contentDescription = "Granted",
                    tint = NeuColors.Connected,
                    modifier = Modifier.size(24.dp),
                )
            }
        } else {
            NeuButton(onClick = onGrant, isAccent = true) {
                Text(grantLabel, color = NeuColors.TextOnAccent)
            }
        }
    }
}

@Composable
private fun PermissionInfoRow(
    label: String,
    description: String,
    granted: Boolean,
    onManage: () -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                Box(
                    modifier = Modifier
                        .size(8.dp)
                        .clip(CircleShape)
                        .background(if (granted) NeuColors.Connected else NeuColors.Error)
                )
                Text(label, style = MaterialTheme.typography.titleMedium)
            }
            Text(
                description,
                style = MaterialTheme.typography.bodySmall,
                modifier = Modifier.padding(start = 16.dp),
            )
        }
        NeuButton(onClick = onManage) {
            Text("Manage", color = NeuColors.TextPrimary)
        }
    }
}

@Composable
private fun ToggleRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
) {
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.SpaceBetween,
        verticalAlignment = Alignment.CenterVertically,
    ) {
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.titleMedium)
            Text(subtitle, style = MaterialTheme.typography.bodySmall)
        }
        Switch(
            checked = checked,
            onCheckedChange = onCheckedChange,
            colors = SwitchDefaults.colors(
                checkedThumbColor = NeuColors.Accent,
                checkedTrackColor = NeuColors.Accent.copy(alpha = 0.3f),
                uncheckedThumbColor = NeuColors.TextSecondary,
                uncheckedTrackColor = NeuColors.DarkShadow.copy(alpha = 0.3f),
            )
        )
    }
}

@Composable
private fun StatusDot(status: ConnectionStatus) {
    val color = when (status) {
        ConnectionStatus.Disconnected -> NeuColors.Disconnected
        ConnectionStatus.Connecting -> NeuColors.Accent.copy(alpha = 0.6f)
        is ConnectionStatus.Connected -> NeuColors.Connected
        is ConnectionStatus.Paused -> NeuColors.TextSecondary.copy(alpha = 0.5f)
        is ConnectionStatus.Error -> NeuColors.Error
    }
    Box(
        modifier = Modifier
            .size(12.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(color)
    )
}

private fun statusTitle(status: ConnectionStatus): String = when (status) {
    ConnectionStatus.Disconnected -> "Disconnected"
    ConnectionStatus.Connecting -> "Connecting…"
    is ConnectionStatus.Connected -> "Connected"
    is ConnectionStatus.Paused -> "Disconnected"
    is ConnectionStatus.Error -> "Error"
}

private fun statusSubtitle(status: ConnectionStatus, hasPairing: Boolean = false): String = when (status) {
    ConnectionStatus.Disconnected -> if (hasPairing) "Sync stopped" else "Pair with a Mac to start syncing"
    ConnectionStatus.Connecting -> "Establishing connection…"
    is ConnectionStatus.Connected -> status.host
    is ConnectionStatus.Paused -> "Sync stopped"
    is ConnectionStatus.Error -> status.reason
}

@Composable
private fun DiscoveredServerCard(
    d: Discovered,
    isPaired: Boolean,
    canUnpair: Boolean,
    onPair: () -> Unit,
    onUnpair: () -> Unit,
) {
    NeuCard {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Column(Modifier.weight(1f)) {
                Text(d.name, style = MaterialTheme.typography.titleMedium)
                Text(
                    "${d.host}:${d.port}",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            when {
                canUnpair -> {
                    NeuButton(onClick = onUnpair, isAccent = false) {
                        Text("Unpair", color = NeuColors.Error)
                    }
                }
                isPaired -> {
                    NeuStatusBadge(label = "Connected", color = NeuColors.Connected)
                }
                else -> {
                    NeuButton(onClick = onPair, isAccent = true) {
                        Text("Pair", color = NeuColors.TextOnAccent)
                    }
                }
            }
        }
    }
}

@Composable
private fun PairingCodeDialog(
    target: PairingTarget,
    onDismiss: () -> Unit,
    onConfirm: (String) -> Unit
) {
    var code by remember { mutableStateOf("") }
    val focusRequester = remember { FocusRequester() }
    LaunchedEffect(Unit) { focusRequester.requestFocus() }
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
                Spacer(Modifier.height(12.dp))
                OutlinedTextField(
                    value = code,
                    onValueChange = { code = it.filter { c -> c.isDigit() }.take(6) },
                    label = { Text("6-digit code") },
                    singleLine = true,
                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.NumberPassword),
                    keyboardActions = KeyboardActions(onDone = { if (code.length == 6) onConfirm(code) }),
                    textStyle = MaterialTheme.typography.headlineMedium.copy(
                        letterSpacing = 8.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    modifier = Modifier.fillMaxWidth().focusRequester(focusRequester)
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(code) },
                enabled = code.length == 6
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
        }
    )
}

sealed class PairingTarget {
    data class Auto(val discovered: Discovered) : PairingTarget()
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
