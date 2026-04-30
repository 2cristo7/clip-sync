package com.clipsync.ui.components

import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.shape.RoundedCornerShape
import android.content.Context
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material.icons.filled.ContentCopy
import androidx.compose.material.icons.filled.Error
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import com.clipsync.model.AppError
import com.clipsync.model.ErrorAction
import com.clipsync.model.ErrorSeverity
import com.clipsync.ui.theme.NeuButton
import com.clipsync.ui.theme.NeuColors

@Composable
fun ErrorBanner(
    error: AppError,
    onDismiss: () -> Unit,
    onAction: ((ErrorAction) -> Unit)? = null,
) {
    val borderColor = if (error.severity == ErrorSeverity.ERROR) NeuColors.Error else NeuColors.Warning
    val iconColor = borderColor
    val shape = RoundedCornerShape(16.dp)
    var expanded by remember { mutableStateOf(false) }

    Column(
        modifier = Modifier
            .fillMaxWidth()
            .animateContentSize()
            .clip(shape)
            .background(NeuColors.SurfaceRaised)
            .border(BorderStroke(1.5.dp, borderColor.copy(alpha = 0.6f)), shape)
            .padding(horizontal = 14.dp, vertical = 10.dp),
        verticalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        // Header row: icon + summary + dismiss button
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = if (error.severity == ErrorSeverity.ERROR) Icons.Filled.Error else Icons.Filled.Warning,
                contentDescription = error.severity.name,
                tint = iconColor,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(
                text = error.summary,
                color = NeuColors.TextPrimary,
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.weight(1f),
            )
            // Details toggle
            if (error.detail != null || error.suggestion != null) {
                Text(
                    text = if (expanded) "▾ Details" else "▸ Details",
                    color = NeuColors.TextSecondary,
                    fontSize = 12.sp,
                    modifier = Modifier
                        .clickable { expanded = !expanded }
                        .padding(horizontal = 4.dp),
                )
                Spacer(Modifier.width(4.dp))
            }
            IconButton(
                onClick = onDismiss,
                modifier = Modifier.size(28.dp),
            ) {
                Icon(
                    imageVector = Icons.Filled.Close,
                    contentDescription = "Dismiss",
                    tint = NeuColors.TextSecondary,
                    modifier = Modifier.size(16.dp),
                )
            }
        }

        // Expanded detail section
        if (expanded) {
            val context = LocalContext.current
            Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                error.detail?.let {
                    Text(
                        text = it,
                        color = NeuColors.TextSecondary,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                error.suggestion?.let {
                    Text(
                        text = it,
                        color = NeuColors.TextSecondary,
                        style = MaterialTheme.typography.bodySmall,
                        fontWeight = FontWeight.Medium,
                    )
                }
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.End,
                ) {
                    IconButton(
                        onClick = {
                            val clipboardManager =
                                context.getSystemService(Context.CLIPBOARD_SERVICE)
                                    as android.content.ClipboardManager
                            clipboardManager.setPrimaryClip(
                                android.content.ClipData.newPlainText(
                                    "ClipSync Error",
                                    "${error.summary}\n${error.detail.orEmpty()}\n${error.suggestion.orEmpty()}"
                                )
                            )
                        },
                        modifier = Modifier.size(28.dp),
                    ) {
                        Icon(
                            imageVector = Icons.Filled.ContentCopy,
                            contentDescription = "Copy error",
                            tint = NeuColors.TextSecondary,
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }
            }
        }

        // Action button
        if (error.action != null && onAction != null) {
            val actionLabel = when (error.action) {
                is ErrorAction.Retry -> "Retry"
                is ErrorAction.Repair -> "Re-pair"
                is ErrorAction.OpenUrl -> "Open"
            }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.End) {
                NeuButton(
                    onClick = { onAction(error.action) },
                    modifier = Modifier,
                    isAccent = false,
                ) {
                    Text(
                        text = actionLabel,
                        fontSize = 13.sp,
                        color = NeuColors.TextPrimary,
                    )
                }
            }
        }
    }
}
