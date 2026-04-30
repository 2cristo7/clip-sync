package com.clipsync.ui.theme

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Switch
import androidx.compose.material3.SwitchDefaults
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.draw.drawBehind
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.material3.Icon
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

@Composable
fun NeuCard(
    modifier: Modifier = Modifier,
    cornerRadius: Dp = 20.dp,
    content: @Composable () -> Unit
) {
    val shape = RoundedCornerShape(cornerRadius)
    Box(
        modifier = modifier
            .fillMaxWidth()
            .clip(shape)
            .background(NeuColors.SurfaceRaised)
            .border(BorderStroke(1.dp, NeuColors.Border), shape)
            .padding(16.dp)
    ) {
        content()
    }
}

@Composable
fun NeuButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    isAccent: Boolean = false,
    isDestructive: Boolean = false,
    content: @Composable () -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(targetValue = if (isPressed) 0.96f else 1f, label = "btn_scale")
    val shape = RoundedCornerShape(14.dp)
    val bg = when {
        isAccent -> NeuColors.AccentSubtle
        isDestructive -> NeuColors.Error.copy(alpha = 0.12f)
        else -> NeuColors.SurfaceRaised
    }
    val borderColor = when {
        isAccent -> NeuColors.Accent.copy(alpha = 0.35f)
        isDestructive -> NeuColors.Error.copy(alpha = 0.4f)
        else -> NeuColors.Border
    }

    Box(
        contentAlignment = Alignment.Center,
        modifier = modifier
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .clip(shape)
            .background(bg)
            .border(BorderStroke(1.dp, borderColor), shape)
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                enabled = enabled,
                onClick = onClick,
            )
            .padding(horizontal = 20.dp, vertical = 14.dp)
    ) {
        content()
    }
}

@Composable
fun NeuSegmentedToggle(
    options: List<String>,
    selectedIndex: Int,
    onSelected: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    val containerShape = RoundedCornerShape(16.dp)
    val pillShape = RoundedCornerShape(12.dp)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(containerShape)
            .background(NeuColors.SurfaceInset)
            .border(BorderStroke(1.dp, NeuColors.Border), containerShape)
            .padding(4.dp),
        horizontalArrangement = Arrangement.spacedBy(4.dp)
    ) {
        options.forEachIndexed { index, label ->
            val selected = index == selectedIndex
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .clip(pillShape)
                    .then(
                        if (selected) Modifier.background(NeuColors.Accent)
                        else Modifier.background(Color.Transparent)
                    )
                    .clickable { onSelected(index) }
                    .padding(vertical = 12.dp)
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.bodyLarge.copy(
                        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal
                    ),
                    color = if (selected) NeuColors.TextOnAccent else NeuColors.TextSecondary
                )
            }
        }
    }
}

@Composable
fun NeuStatusBadge(
    label: String,
    color: Color,
    modifier: Modifier = Modifier
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
        modifier = modifier
    ) {
        Box(
            modifier = Modifier
                .drawBehind {
                    drawIntoCanvas { canvas ->
                        val paint = Paint().also { p ->
                            p.asFrameworkPaint().apply {
                                isAntiAlias = true
                                this.color = color.toArgb()
                            }
                        }
                        val cx = size.width / 2f
                        val cy = size.height / 2f
                        canvas.drawCircle(Offset(cx, cy), size.width / 2f, paint)
                    }
                }
                .padding(6.dp)
        )
        Text(text = label, style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
fun NeuSectionHeader(
    title: String,
    modifier: Modifier = Modifier,
    textAlign: TextAlign = TextAlign.Start,
    icon: ImageVector? = null,
) {
    val arrangement = if (textAlign == TextAlign.Center) Arrangement.Center else Arrangement.Start
    Row(
        modifier = modifier.padding(bottom = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = arrangement,
    ) {
        if (icon != null) {
            Icon(
                imageVector = icon,
                contentDescription = null,
                tint = NeuColors.Accent,
                modifier = Modifier.size(13.dp),
            )
            Box(Modifier.size(5.dp))
        }
        Text(
            text = title,
            style = MaterialTheme.typography.bodySmall.copy(
                fontWeight    = FontWeight.SemiBold,
                fontSize      = 13.sp,
                letterSpacing = 0.sp,
                color         = NeuColors.TextPrimary,
            ),
        )
    }
}

// Status pill — active = solid green, inactive = outlined gray or red
@Composable
fun NeuStatusPill(
    label: String,
    active: Boolean,
    inactiveIsError: Boolean = false,
    modifier: Modifier = Modifier,
) {
    val shape = RoundedCornerShape(50)
    if (active) {
        Row(
            modifier = modifier
                .clip(shape)
                .background(NeuColors.Connected)
                .padding(horizontal = 12.dp, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Box(Modifier.size(6.dp).clip(CircleShape).background(NeuColors.TextOnAccent.copy(alpha = 0.5f)))
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.SemiBold),
                color = NeuColors.TextOnAccent,
            )
        }
    } else {
        val color = if (inactiveIsError) NeuColors.Error else NeuColors.TextSecondary
        Row(
            modifier = modifier
                .clip(shape)
                .border(BorderStroke(1.dp, color.copy(alpha = 0.45f)), shape)
                .padding(horizontal = 12.dp, vertical = 5.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Box(Modifier.size(6.dp).clip(CircleShape).background(color.copy(alpha = 0.6f)))
            Text(
                text = label,
                style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Medium),
                color = color,
            )
        }
    }
}

// Flat status row with inline pill — no card
@Composable
fun NeuStatusRow(
    title: String,
    subtitle: String,
    active: Boolean,
    activeLabel: String = "Active",
    inactiveLabel: String = "Inactive",
    inactiveIsError: Boolean = false,
    modifier: Modifier = Modifier,
    divider: Boolean = true,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 4.dp, vertical = 14.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = NeuColors.TextSecondary,
                )
            }
            NeuStatusPill(
                label = if (active) activeLabel else inactiveLabel,
                active = active,
                inactiveIsError = inactiveIsError,
            )
        }
        if (divider) HorizontalDivider(color = NeuColors.Border.copy(alpha = 0.5f), thickness = 0.5.dp)
    }
}

// Standalone toggle row — no card, hairline divider
@Composable
fun NeuToggleRow(
    title: String,
    subtitle: String,
    checked: Boolean,
    onCheckedChange: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    divider: Boolean = true,
) {
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .clickable(
                    interactionSource = remember { MutableInteractionSource() },
                    indication = null,
                ) { onCheckedChange(!checked) }
                .padding(horizontal = 4.dp, vertical = 10.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Text(title, style = MaterialTheme.typography.titleMedium)
                Text(
                    subtitle,
                    style = MaterialTheme.typography.bodySmall,
                    color = NeuColors.TextSecondary,
                )
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
        if (divider) HorizontalDivider(color = NeuColors.Border.copy(alpha = 0.5f), thickness = 0.5.dp)
    }
}

// Manage row — status dot + label/description left, outline pill button right
@Composable
fun NeuManageRow(
    label: String,
    description: String,
    granted: Boolean,
    onManage: () -> Unit,
    manageLabel: String = "Manage",
    modifier: Modifier = Modifier,
    divider: Boolean = true,
) {
    val dotColor = if (granted) NeuColors.Connected else NeuColors.Error
    val pillShape = RoundedCornerShape(50)
    Column(modifier = modifier.fillMaxWidth()) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 4.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Column(Modifier.weight(1f).padding(end = 12.dp)) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Box(
                        modifier = Modifier
                            .size(7.dp)
                            .clip(CircleShape)
                            .background(dotColor)
                    )
                    Text(label, style = MaterialTheme.typography.titleMedium)
                }
                Text(
                    description,
                    style = MaterialTheme.typography.bodySmall,
                    color = NeuColors.TextSecondary,
                    modifier = Modifier.padding(start = 15.dp),
                )
            }
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .clip(pillShape)
                    .border(BorderStroke(1.dp, NeuColors.Border), pillShape)
                    .clickable(onClick = onManage)
                    .padding(horizontal = 14.dp, vertical = 7.dp)
            ) {
                Text(
                    manageLabel,
                    style = MaterialTheme.typography.labelMedium.copy(fontWeight = FontWeight.Medium),
                    color = NeuColors.TextSecondary,
                )
            }
        }
        if (divider) HorizontalDivider(color = NeuColors.Border.copy(alpha = 0.5f), thickness = 0.5.dp)
    }
}
