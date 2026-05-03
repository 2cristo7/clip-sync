package com.clipsync.ui.sections

import android.content.Context
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.Router
import androidx.compose.material.icons.outlined.SettingsEthernet
import androidx.compose.material.icons.outlined.Wifi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.clipsync.discovery.Discovered
import com.clipsync.storage.Prefs
import com.clipsync.ui.ConnectionStatus
import com.clipsync.ui.PairingTarget
import com.clipsync.ui.SettingsState
import com.clipsync.ui.SettingsViewModel
import com.clipsync.ui.TailscaleState
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuCard
import com.clipsync.ui.theme.NeuColors
import com.clipsync.ui.theme.NeuSectionHeader
import com.clipsync.ui.theme.NeuSegmentedToggle
import com.clipsync.ui.theme.NeuStatusBadge
import com.clipsync.ui.theme.NeuStatusPill

@Composable
fun ConnectionSection(
    state: SettingsState,
    vm: SettingsViewModel,
    context: Context,
    onPairingTarget: (PairingTarget) -> Unit,
) {
    var manualHost by rememberSaveable { mutableStateOf("") }
    var manualPort by rememberSaveable { mutableStateOf(Prefs.DEFAULT_PORT.toString()) }

    val canAuto = state.isOnWifi
    val canManual = state.isTailscaleVpnActive
    val effectiveMode = when {
        canAuto && canManual -> state.mode
        canAuto -> Prefs.MODE_AUTO
        canManual -> Prefs.MODE_MANUAL
        else -> ""
    }

    NeuSectionHeader(
        "Connection Mode",
        modifier = Modifier.fillMaxWidth(),
        textAlign = TextAlign.Center,
        icon = Icons.Outlined.Wifi,
    )

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
            },
        )
    } else if (canAuto) {
        Text(
            "WiFi — auto discovery",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.fillMaxWidth(),
            textAlign = TextAlign.Center,
        )
    } else if (canManual) {
        Text(
            "Tailscale VPN — manual IP",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.fillMaxWidth(),
            textAlign = TextAlign.Center,
        )
    }

    // Status card
    NeuCard {
        Column(verticalArrangement = Arrangement.spacedBy(16.dp)) {
            Row(
                verticalAlignment = Alignment.CenterVertically,
                modifier = Modifier.fillMaxWidth(),
            ) {
                ConnectionStatusDot(state.status)
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
                        isAccent = true,
                    ) {
                        Text("Connect", color = NeuColors.Accent)
                    }
                } else {
                    NeuButton(
                        onClick = { vm.stopSync(context) },
                        modifier = Modifier.fillMaxWidth(),
                        isDestructive = true,
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
        verticalArrangement = Arrangement.spacedBy(12.dp),
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
                            style = MaterialTheme.typography.bodyMedium,
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
                        onPair = { if (!isPaired) onPairingTarget(PairingTarget.Auto(d)) },
                        onUnpair = { vm.unpair(context) },
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
                    Column(
                        Modifier
                            .weight(1f)
                            .padding(end = 12.dp),
                    ) {
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
                                .clickable(
                                    interactionSource = remember { MutableInteractionSource() },
                                    indication = null,
                                ) { vm.unpair(context) }
                                .padding(horizontal = 14.dp, vertical = 7.dp),
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
                    Regex("^[a-zA-Z0-9]([a-zA-Z0-9\\-\\.]*[a-zA-Z0-9])?\$"),
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
                            modifier = Modifier.fillMaxWidth(),
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
                            modifier = Modifier.fillMaxWidth(),
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
                                onPairingTarget(
                                    PairingTarget.Manual(
                                        host = hostTrimmed,
                                        port = portNum ?: Prefs.DEFAULT_PORT,
                                    )
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
}

// ── Private helpers used only by ConnectionSection ──────────────────────────

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
            verticalAlignment = Alignment.CenterVertically,
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
private fun ConnectionStatusDot(status: ConnectionStatus) {
    val color = when (status) {
        ConnectionStatus.Disconnected -> NeuColors.Disconnected
        ConnectionStatus.Connecting -> NeuColors.Accent.copy(alpha = 0.6f)
        is ConnectionStatus.Connected -> NeuColors.Connected
        is ConnectionStatus.Paused -> NeuColors.TextSecondary.copy(alpha = 0.5f)
        is ConnectionStatus.Error -> NeuColors.Error
    }
    val description = when (status) {
        ConnectionStatus.Disconnected -> "Status: Disconnected"
        ConnectionStatus.Connecting -> "Status: Connecting"
        is ConnectionStatus.Connected -> "Status: Connected to ${status.host}"
        is ConnectionStatus.Paused -> "Status: Paused"
        is ConnectionStatus.Error -> "Status: Error — ${status.reason}"
    }
    Box(
        modifier = Modifier
            .size(12.dp)
            .clip(RoundedCornerShape(6.dp))
            .background(color)
            .semantics { contentDescription = description },
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
