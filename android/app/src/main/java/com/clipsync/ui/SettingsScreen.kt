package com.clipsync.ui

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.text.TextUtils
import androidx.compose.animation.animateContentSize
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
import androidx.compose.material.icons.outlined.AdminPanelSettings
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Router
import androidx.compose.material.icons.outlined.SettingsEthernet
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material.icons.outlined.VpnKey
import androidx.compose.material.icons.outlined.Wifi
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
import androidx.compose.ui.layout.LayoutCoordinates
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.positionInWindow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.clipsync.discovery.Discovered
import com.clipsync.storage.Prefs
import com.clipsync.ui.ShizukuInstallState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuCard
import com.clipsync.ui.theme.NeuColors
import com.clipsync.ui.theme.NeuManageRow
import com.clipsync.ui.theme.NeuSectionHeader
import com.clipsync.ui.theme.NeuSegmentedToggle
import com.clipsync.ui.theme.NeuStatusBadge
import com.clipsync.ui.theme.NeuStatusPill
import com.clipsync.ui.theme.NeuStatusRow
import com.clipsync.ui.theme.NeuToggleRow

@Composable
fun SettingsScreen(
    isDark: Boolean = true,
    onToggleTheme: (cx: Float, cy: Float) -> Unit = { _, _ -> },
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
                        .onGloballyPositioned { themeBtnCoords = it }
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

            // Connection mode — only show toggle when both options available
            val canAuto = state.isOnWifi
            val canManual = state.isTailscaleVpnActive
            val effectiveMode = when {
                canAuto && canManual -> state.mode
                canAuto -> Prefs.MODE_AUTO
                canManual -> Prefs.MODE_MANUAL
                else -> ""
            }

            NeuSectionHeader("Connection Mode", modifier = Modifier.fillMaxWidth(), textAlign = androidx.compose.ui.text.style.TextAlign.Center, icon = Icons.Outlined.Wifi)
            if (!canAuto && !canManual) {
                NeuCard {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        Text(
                            "No connection available",
                            style = MaterialTheme.typography.titleSmall,
                            color = NeuColors.Error,
                        )
                        Text(
                            "Connect to the same WiFi as your Mac, or activate Tailscale VPN to sync remotely.",
                            style = MaterialTheme.typography.bodySmall,
                        )
                        if (state.tailscaleState is TailscaleState.Installed) {
                            NeuButton(
                                onClick = {
                                    val intent = context.packageManager
                                        .getLaunchIntentForPackage("com.tailscale.ipn")
                                    if (intent != null) context.startActivity(intent)
                                },
                                isAccent = true,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Open Tailscale", color = NeuColors.Accent)
                            }
                        } else if (state.tailscaleState is TailscaleState.NotInstalled) {
                            NeuButton(
                                onClick = { vm.openTailscalePlayStore(context) },
                                isAccent = true,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Text("Install Tailscale", color = NeuColors.Accent)
                            }
                        }
                    }
                }
            }

            if (canAuto && canManual) {
                NeuSegmentedToggle(
                    options = listOf("Auto (mDNS)", "Manual IP"),
                    selectedIndex = if (state.mode == Prefs.MODE_AUTO) 0 else 1,
                    onSelected = { idx ->
                        vm.setMode(context, if (idx == 0) Prefs.MODE_AUTO else Prefs.MODE_MANUAL)
                    }
                )
            } else if (canAuto) {
                Text(
                    "WiFi — auto discovery",
                    style = MaterialTheme.typography.titleLarge,
                    modifier = Modifier.fillMaxWidth(),
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
            } else if (canManual) {
                Text(
                    "Tailscale VPN — manual IP",
                    style = MaterialTheme.typography.titleLarge,
                    modifier = Modifier.fillMaxWidth(),
                    textAlign = androidx.compose.ui.text.style.TextAlign.Center,
                )
            }

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
                        val pairedToTailscale = state.pairedHost?.let { h ->
                            h.split(".").let { p ->
                                p.size == 4 && p[0].toIntOrNull() == 100 && ((p[1].toIntOrNull() ?: -1) in 64..127)
                            }
                        } ?: false
                        val connectBlocked = pairedToTailscale && !state.isTailscaleVpnActive
                        if (!isActive) {
                            if (connectBlocked) {
                                Text(
                                    "Tailscale VPN not active — open Tailscale to connect",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = NeuColors.Error,
                                )
                            }
                            NeuButton(
                                onClick = { vm.startSync(context) },
                                modifier = Modifier.fillMaxWidth(),
                                enabled = !connectBlocked,
                                isAccent = true
                            ) {
                                Text("Connect", color = NeuColors.Accent)
                            }
                        } else {
                            NeuButton(
                                onClick = { vm.stopSync(context) },
                                modifier = Modifier.fillMaxWidth(),
                                isDestructive = true
                            ) {
                                Text("Stop Sync", color = NeuColors.Error)
                            }
                        }
                    }
                }
            }

            // Discovery / Manual section
            Column(
                modifier = Modifier.animateContentSize(),
                verticalArrangement = Arrangement.spacedBy(12.dp)
            ) {
                if (effectiveMode == Prefs.MODE_AUTO) {
                    NeuSectionHeader("Discovered Servers", icon = Icons.Outlined.Router)
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
                } else if (effectiveMode == Prefs.MODE_MANUAL) {
                    NeuSectionHeader("Manual Connection", icon = Icons.Outlined.SettingsEthernet)
                    if (state.hasPairing && state.mode == Prefs.MODE_MANUAL) {
                        val isActive = state.status is ConnectionStatus.Connected ||
                            state.status is ConnectionStatus.Connecting
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 4.dp, vertical = 14.dp),
                            horizontalArrangement = Arrangement.SpaceBetween,
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                                Text(
                                    state.pairedHost ?: "Manual",
                                    style = MaterialTheme.typography.titleMedium,
                                )
                                Text(
                                    "${state.pairedHost}:${state.pairedPort}",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = NeuColors.TextSecondary,
                                )
                            }
                            if (isActive) {
                                NeuStatusPill(label = "Connected", active = true)
                            } else {
                                val pillShape = RoundedCornerShape(50)
                                Box(
                                    contentAlignment = Alignment.Center,
                                    modifier = Modifier
                                        .clip(pillShape)
                                        .border(BorderStroke(1.dp, NeuColors.Error.copy(alpha = 0.5f)), pillShape)
                                        .clickable { vm.unpair(context) }
                                        .padding(horizontal = 14.dp, vertical = 7.dp)
                                ) {
                                    Text(
                                        "Unpair",
                                        style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Medium),
                                        color = NeuColors.Error,
                                    )
                                }
                            }
                        }
                    } else {
                        val hostTrimmed = manualHost.trim()
                        val portNum = manualPort.toIntOrNull()
                        val hostValid = hostTrimmed.isNotBlank() && hostTrimmed.matches(
                            Regex("^[a-zA-Z0-9]([a-zA-Z0-9\\-\\.]*[a-zA-Z0-9])?\$")
                        )
                        val portValid = portNum != null && portNum in 1..65535
                        val isTailscaleIp = hostTrimmed.split(".").let { parts ->
                            parts.size == 4
                                && (parts[0].toIntOrNull() == 100)
                                && ((parts[1].toIntOrNull() ?: -1) in 64..127)
                        }
                        val vpnBlocked = isTailscaleIp && !state.isTailscaleVpnActive
                        NeuCard {
                            Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                                OutlinedTextField(
                                    value = manualHost,
                                    onValueChange = { manualHost = it },
                                    label = { Text("Host / IP") },
                                    singleLine = true,
                                    isError = (manualHost.isNotEmpty() && !hostValid) || vpnBlocked,
                                    supportingText = if (vpnBlocked) {
                                        { Text("Tailscale VPN is not active", color = NeuColors.Error) }
                                    } else if (manualHost.isNotEmpty() && !hostValid) {
                                        { Text("Enter a valid IP or hostname") }
                                    } else if (isTailscaleIp && state.isTailscaleVpnActive) {
                                        { Text("Tailscale IP detected — VPN active", color = NeuColors.Connected) }
                                    } else null,
                                    modifier = Modifier.fillMaxWidth()
                                )
                                OutlinedTextField(
                                    value = manualPort,
                                    onValueChange = { manualPort = it.filter { c -> c.isDigit() }.take(5) },
                                    label = { Text("Port") },
                                    singleLine = true,
                                    isError = manualPort.isNotEmpty() && !portValid,
                                    supportingText = if (manualPort.isNotEmpty() && !portValid) {
                                        { Text("Port must be 1–65535") }
                                    } else null,
                                    keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number),
                                    modifier = Modifier.fillMaxWidth()
                                )
                                if (vpnBlocked) {
                                    NeuCard {
                                        Row(
                                            verticalAlignment = Alignment.CenterVertically,
                                            horizontalArrangement = Arrangement.spacedBy(8.dp),
                                        ) {
                                            Text("⚠️", fontSize = 20.sp)
                                            Column(Modifier.weight(1f)) {
                                                Text(
                                                    "Tailscale VPN not active",
                                                    style = MaterialTheme.typography.titleSmall,
                                                    color = NeuColors.Error,
                                                )
                                                Text(
                                                    "Open Tailscale and connect before pairing.",
                                                    style = MaterialTheme.typography.bodySmall,
                                                )
                                            }
                                        }
                                    }
                                }
                                NeuButton(
                                    onClick = {
                                        pairingTarget = PairingTarget.Manual(
                                            host = hostTrimmed,
                                            port = portNum ?: 7010
                                        )
                                    },
                                    enabled = hostValid && portValid && !vpnBlocked,
                                    isAccent = true,
                                ) {
                                    Text("Pair", color = NeuColors.Accent)
                                }
                            }
                        }
                    }
                }
            }

            // Tailscale status
            if (state.tailscaleState is TailscaleState.Installed) {
                NeuSectionHeader("Tailscale", icon = Icons.Outlined.VpnKey)
                NeuStatusRow(
                    title = "Tailscale",
                    subtitle = if (state.isTailscaleVpnActive) "VPN active" else "VPN off",
                    active = state.isTailscaleVpnActive,
                    activeLabel = "VPN active",
                    inactiveLabel = "VPN off",
                    inactiveIsError = true,
                    divider = false,
                )
            } else if (state.tailscaleState is TailscaleState.NotInstalled && state.isOnMobileData) {
                NeuSectionHeader("Tailscale", icon = Icons.Outlined.VpnKey)
                NeuCard {
                    Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                        NeuStatusBadge(label = "Not installed", color = NeuColors.TextSecondary)
                        Text(
                            "Install Tailscale to sync from anywhere, not just your home WiFi.",
                            style = MaterialTheme.typography.bodySmall,
                        )
                        NeuButton(
                            onClick = { vm.openTailscalePlayStore(context) },
                            isAccent = true,
                            modifier = Modifier.fillMaxWidth(),
                        ) {
                            Text("Install Tailscale", color = NeuColors.Accent)
                        }
                    }
                }
            }

            // Clipboard access (Shizuku)
            NeuSectionHeader("Clipboard Access", icon = Icons.Outlined.ContentPaste)
            NeuStatusRow(
                title = "Shizuku",
                subtitle = if (shizukuReady)
                    "Direct clipboard access is active. Starts and stops with the Mac connection."
                else
                    "Not active. Go to Permissions to set it up.",
                active = shizukuReady,
                activeLabel = "Active",
                inactiveLabel = "Inactive",
                divider = false,
            )

            // Toggles
            NeuSectionHeader("Features", icon = Icons.Outlined.Tune)
            NeuToggleRow(
                title = "Auto send on copy",
                subtitle = "Sends clipboard to Mac as soon as you copy",
                checked = state.autoSendEnabled,
                onCheckedChange = { vm.setAutoSendEnabled(context, it) },
                divider = false,
            )

            // Permissions info
            NeuSectionHeader("Permissions", icon = Icons.Outlined.AdminPanelSettings)
            NeuManageRow(
                label = "Notifications",
                description = "Required for the sync service to run in the background",
                granted = state.notificationPermissionGranted,
                onManage = {
                    context.startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).apply {
                        putExtra(Settings.EXTRA_APP_PACKAGE, context.packageName)
                    })
                }
            )
            NeuManageRow(
                label = "Media access",
                description = "Required for sending screenshots to Mac",
                granted = state.mediaPermissionGranted,
                onManage = {
                    context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS).apply {
                        data = Uri.parse("package:${context.packageName}")
                    })
                },
                divider = false,
            )

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
                Text(grantLabel, color = NeuColors.Accent)
            }
        }
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
                        Text("Pair", color = NeuColors.Accent)
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
                    supportingText = {
                        Text("${code.length}/6 digits")
                    },
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
