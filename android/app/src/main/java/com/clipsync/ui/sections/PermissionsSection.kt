package com.clipsync.ui.sections

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.outlined.AdminPanelSettings
import androidx.compose.material.icons.outlined.ContentPaste
import androidx.compose.material.icons.outlined.Tune
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import com.clipsync.ui.SettingsState
import com.clipsync.ui.SettingsViewModel
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuColors
import com.clipsync.ui.theme.NeuManageRow
import com.clipsync.ui.theme.NeuSectionHeader
import com.clipsync.ui.theme.NeuStatusRow
import com.clipsync.ui.theme.NeuToggleRow

@Composable
fun PermissionsSection(
    state: SettingsState,
    vm: SettingsViewModel,
    context: Context,
) {
    val shizukuReady = state.shizukuState == "ready"

    // Clipboard Access (Shizuku)
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

    // Features toggles
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
        },
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
}

// ── PermissionRow — used in the blocking permissions modal in SettingsScreen ─

@Composable
fun PermissionRow(
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
