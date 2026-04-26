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
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
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
    content: @Composable () -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(targetValue = if (isPressed) 0.96f else 1f, label = "btn_scale")
    val shape = RoundedCornerShape(14.dp)
    val bg = if (isAccent) NeuColors.Accent else NeuColors.SurfaceRaised
    val borderColor = if (isAccent) NeuColors.AccentLight.copy(alpha = 0.4f) else NeuColors.Border

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
fun NeuSectionHeader(title: String, modifier: Modifier = Modifier) {
    Text(
        text = title.uppercase(),
        style = MaterialTheme.typography.bodySmall.copy(
            fontWeight    = FontWeight.Bold,
            letterSpacing = 1.5.sp,
            color         = NeuColors.Accent,
        ),
        modifier = modifier.padding(bottom = 8.dp)
    )
}

// Backward-compatible aliases
@Composable
fun ClayCard(
    modifier: Modifier = Modifier,
    cornerRadius: Dp = 20.dp,
    content: @Composable () -> Unit
) = NeuCard(modifier, cornerRadius, content)

@Composable
fun ClayButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    isAccent: Boolean = false,
    content: @Composable () -> Unit
) = NeuButton(onClick, modifier, enabled, isAccent, content)

@Composable
fun ClaySegmentedToggle(
    options: List<String>,
    selectedIndex: Int,
    onSelected: (Int) -> Unit,
    modifier: Modifier = Modifier
) = NeuSegmentedToggle(options, selectedIndex, onSelected, modifier)

@Composable
fun ClayStatusBadge(label: String, color: Color, modifier: Modifier = Modifier) =
    NeuStatusBadge(label, color, modifier)

@Composable
fun ClaySectionHeader(title: String, modifier: Modifier = Modifier) =
    NeuSectionHeader(title, modifier)
