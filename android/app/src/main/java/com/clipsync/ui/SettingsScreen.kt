package com.clipsync.ui

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Divider
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import com.clipsync.discovery.Discovered
import com.clipsync.storage.Prefs

@Composable
fun SettingsScreen(vm: SettingsViewModel = viewModel()) {
    val context = LocalContext.current
    val state by vm.state.collectAsState()
    var pairingTarget by remember { mutableStateOf<PairingTarget?>(null) }
    var manualHost by rememberSaveable { mutableStateOf("") }
    var manualPort by rememberSaveable { mutableStateOf("7010") }

    LaunchedEffect(Unit) { vm.bootstrap(context) }

    Column(Modifier.fillMaxWidth().padding(16.dp)) {
        Text("ClipSync", style = androidx.compose.material3.MaterialTheme.typography.headlineMedium)
        Spacer(Modifier.height(8.dp))
        StatusRow(state.status)
        Spacer(Modifier.height(16.dp))
        Divider()
        Spacer(Modifier.height(16.dp))
        Text("Mode", style = androidx.compose.material3.MaterialTheme.typography.titleMedium)
        Row(verticalAlignment = Alignment.CenterVertically) {
            RadioButton(
                selected = state.mode == Prefs.MODE_AUTO,
                onClick = { vm.setMode(Prefs.MODE_AUTO) }
            )
            Text("Auto (mDNS)")
            Spacer(Modifier.height(0.dp))
            RadioButton(
                selected = state.mode == Prefs.MODE_MANUAL,
                onClick = { vm.setMode(Prefs.MODE_MANUAL) }
            )
            Text("Manual IP")
        }
        Spacer(Modifier.height(16.dp))

        if (state.mode == Prefs.MODE_AUTO) {
            Text("Discovered servers", style = androidx.compose.material3.MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            if (state.discovered.isEmpty()) {
                Text("Searching…")
            } else {
                state.discovered.forEach { d ->
                    Row(
                        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
                        horizontalArrangement = Arrangement.SpaceBetween,
                        verticalAlignment = Alignment.CenterVertically
                    ) {
                        Column {
                            Text(d.name)
                            Text(
                                "${d.host}:${d.port}",
                                style = androidx.compose.material3.MaterialTheme.typography.bodySmall
                            )
                        }
                        Button(onClick = { pairingTarget = PairingTarget.Auto(d) }) {
                            Text("Pair")
                        }
                    }
                }
            }
        } else {
            Text("Manual connection", style = androidx.compose.material3.MaterialTheme.typography.titleMedium)
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = manualHost,
                onValueChange = { manualHost = it },
                label = { Text("Host / IP") },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))
            OutlinedTextField(
                value = manualPort,
                onValueChange = { manualPort = it.filter { c -> c.isDigit() }.take(5) },
                label = { Text("Port") },
                modifier = Modifier.fillMaxWidth()
            )
            Spacer(Modifier.height(8.dp))
            Button(
                enabled = manualHost.isNotBlank() && manualPort.isNotBlank(),
                onClick = {
                    pairingTarget = PairingTarget.Manual(
                        host = manualHost.trim(),
                        port = manualPort.toIntOrNull() ?: 7010
                    )
                }
            ) {
                Text("Pair")
            }
        }

        state.error?.let {
            Spacer(Modifier.height(16.dp))
            Text("Error: $it", color = androidx.compose.material3.MaterialTheme.colorScheme.error)
        }
    }

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
private fun StatusRow(status: ConnectionStatus) {
    val label = when (status) {
        ConnectionStatus.Disconnected -> "Disconnected"
        ConnectionStatus.Connecting -> "Connecting…"
        is ConnectionStatus.Connected -> "Connected to ${status.host}"
        is ConnectionStatus.Error -> "Error: ${status.reason}"
    }
    Text(label, style = androidx.compose.material3.MaterialTheme.typography.bodyLarge)
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
        title = { Text("Enter pairing code") },
        text = {
            Column {
                Text(hostLabel)
                Spacer(Modifier.height(8.dp))
                OutlinedTextField(
                    value = code,
                    onValueChange = { code = it.filter { c -> c.isDigit() }.take(6) },
                    label = { Text("6-digit code") }
                )
            }
        },
        confirmButton = {
            TextButton(
                onClick = { onConfirm(code) },
                enabled = code.length == 6
            ) { Text("Pair") }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text("Cancel") }
        }
    )
}

sealed class PairingTarget {
    data class Auto(val discovered: Discovered) : PairingTarget()
    data class Manual(val host: String, val port: Int) : PairingTarget()
}
