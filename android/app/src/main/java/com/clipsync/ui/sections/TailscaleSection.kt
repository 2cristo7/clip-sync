package com.clipsync.ui.sections

import android.content.Context
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.outlined.VpnKey
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.clipsync.ui.SettingsState
import com.clipsync.ui.SettingsViewModel
import com.clipsync.ui.TailscaleState
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuCard
import com.clipsync.ui.theme.NeuColors
import com.clipsync.ui.theme.NeuSectionHeader
import com.clipsync.ui.theme.NeuStatusBadge
import com.clipsync.ui.theme.NeuStatusRow

@Composable
fun TailscaleSection(
    state: SettingsState,
    vm: SettingsViewModel,
    context: Context,
) {
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
}
