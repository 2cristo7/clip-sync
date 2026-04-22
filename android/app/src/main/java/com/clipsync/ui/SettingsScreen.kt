package com.clipsync.ui

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.clipsync.discovery.Discovered
import com.clipsync.storage.Prefs
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuCard
import com.clipsync.ui.theme.NeuColors
import com.clipsync.ui.theme.NeuSectionHeader
import com.clipsync.ui.theme.NeuSegmentedToggle
import com.clipsync.ui.theme.NeuStatusBadge

@Composable
fun SettingsScreen(vm: SettingsViewModel = viewModel()) {
    val context = LocalContext.current
    val state by vm.state.collectAsState()
    var pairingTarget by remember { mutableStateOf<PairingTarget?>(null) }
    var manualHost by rememberSaveable { mutableStateOf("") }
    var manualPort by rememberSaveable { mutableStateOf("7010") }

    LaunchedEffect(Unit) { vm.bootstrap(context) }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(NeuColors.Background)
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp)
        ) {
            // Header
            Spacer(Modifier.height(8.dp))
            Text(
                "ClipSync",
                style = MaterialTheme.typography.headlineLarge,
                fontWeight = FontWeight.Bold,
            )

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
                                statusSubtitle(state.status),
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }

                    if (state.status !is ConnectionStatus.Disconnected) {
                        val isPaused = state.status is ConnectionStatus.Paused
                        NeuButton(
                            onClick = { vm.setSyncEnabled(context, isPaused) },
                            modifier = Modifier.fillMaxWidth()
                        ) {
                            Text(
                                if (isPaused) "Resume Sync" else "Pause Sync",
                                color = if (isPaused) NeuColors.Accent else NeuColors.TextSecondary
                            )
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
                enter = fadeIn() + slideInVertically(),
                exit = fadeOut() + slideOutVertically()
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
                            DiscoveredServerCard(d) { pairingTarget = PairingTarget.Auto(d) }
                        }
                    }
                }
            }

            AnimatedVisibility(
                visible = state.mode == Prefs.MODE_MANUAL,
                enter = fadeIn() + slideInVertically(),
                exit = fadeOut() + slideOutVertically()
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

            // Overlay toggle
            NeuSectionHeader("Clipboard Overlay")
            NeuCard {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column(Modifier.weight(1f)) {
                            Text(
                                "Send to Mac FAB",
                                style = MaterialTheme.typography.titleMedium,
                            )
                            Text(
                                "Shows a floating button when you copy something",
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                        Switch(
                            checked = state.overlayEnabled,
                            onCheckedChange = { vm.setOverlayEnabled(context, it) },
                            colors = SwitchDefaults.colors(
                                checkedThumbColor = NeuColors.Accent,
                                checkedTrackColor = NeuColors.Accent.copy(alpha = 0.3f),
                                uncheckedThumbColor = NeuColors.TextSecondary,
                                uncheckedTrackColor = NeuColors.DarkShadow.copy(alpha = 0.3f),
                            )
                        )
                    }
                    if (state.overlayEnabled && !Settings.canDrawOverlays(context)) {
                        NeuButton(
                            onClick = {
                                val intent = Intent(
                                    Settings.ACTION_MANAGE_OVERLAY_PERMISSION,
                                    Uri.parse("package:${context.packageName}")
                                )
                                context.startActivity(intent)
                            },
                            isAccent = true,
                        ) {
                            Text("Grant overlay permission", color = NeuColors.TextOnAccent)
                        }
                    }
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
    is ConnectionStatus.Paused -> "Paused"
    is ConnectionStatus.Error -> "Error"
}

private fun statusSubtitle(status: ConnectionStatus): String = when (status) {
    ConnectionStatus.Disconnected -> "Pair with a Mac to start syncing"
    ConnectionStatus.Connecting -> "Establishing connection…"
    is ConnectionStatus.Connected -> status.host
    is ConnectionStatus.Paused -> status.host
    is ConnectionStatus.Error -> status.reason
}

@Composable
private fun DiscoveredServerCard(d: Discovered, onPair: () -> Unit) {
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
            NeuButton(onClick = onPair, isAccent = true) {
                Text("Pair", color = NeuColors.TextOnAccent)
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
    val hostLabel = when (target) {
        is PairingTarget.Auto -> "${target.discovered.host}:${target.discovered.port}"
        is PairingTarget.Manual -> "${target.host}:${target.port}"
    }
    AlertDialog(
        onDismissRequest = onDismiss,
        containerColor = NeuColors.Background,
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
                    textStyle = MaterialTheme.typography.headlineMedium.copy(
                        letterSpacing = 8.sp,
                        fontWeight = FontWeight.Bold,
                    ),
                    modifier = Modifier.fillMaxWidth()
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
