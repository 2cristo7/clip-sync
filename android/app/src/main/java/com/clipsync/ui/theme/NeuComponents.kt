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
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp

private val cardGradient
    get() = Brush.verticalGradient(listOf(ClayColors.SurfaceGradTop, ClayColors.SurfaceGradBottom))

private val accentGradient
    get() = Brush.verticalGradient(listOf(ClayColors.EmeraldLight, ClayColors.EmeraldDark))

private fun topHighlight(alpha: Float) = Brush.verticalGradient(
    colorStops = arrayOf(
        0f  to Color.White.copy(alpha = alpha),
        0.6f to Color.White.copy(alpha = 0f),
    )
)

@Composable
fun ClayCard(
    modifier: Modifier = Modifier,
    cornerRadius: Dp = 24.dp,
    content: @Composable () -> Unit
) {
    val shape = RoundedCornerShape(cornerRadius)
    Box(
        modifier = modifier
            .fillMaxWidth()
            .shadow(
                elevation = 10.dp,
                shape = shape,
                ambientColor = ClayColors.EmeraldShadow,
                spotColor = ClayColors.EmeraldShadow,
            )
            .clip(shape)
            .background(cardGradient)
            .border(BorderStroke(1.5.dp, topHighlight(0.85f)), shape)
            .padding(16.dp)
    ) {
        content()
    }
}

@Composable
fun ClayButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    isAccent: Boolean = false,
    content: @Composable () -> Unit
) {
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(targetValue = if (isPressed) 0.95f else 1f, label = "btn_scale")
    val shape = RoundedCornerShape(16.dp)
    val elevation = if (isPressed) 3.dp else 8.dp

    Box(
        contentAlignment = Alignment.Center,
        modifier = modifier
            .graphicsLayer { scaleX = scale; scaleY = scale }
            .shadow(
                elevation = elevation,
                shape = shape,
                ambientColor = ClayColors.EmeraldShadow,
                spotColor = ClayColors.EmeraldShadow,
            )
            .clip(shape)
            .background(if (isAccent) accentGradient else cardGradient)
            .border(BorderStroke(1.5.dp, topHighlight(if (isAccent) 0.45f else 0.85f)), shape)
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
fun ClaySegmentedToggle(
    options: List<String>,
    selectedIndex: Int,
    onSelected: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    val containerShape = RoundedCornerShape(20.dp)
    val pillShape = RoundedCornerShape(16.dp)
    Row(
        modifier = modifier
            .fillMaxWidth()
            .shadow(
                elevation = 6.dp,
                shape = containerShape,
                ambientColor = ClayColors.EmeraldShadow,
                spotColor = ClayColors.EmeraldShadow,
            )
            .clip(containerShape)
            .background(cardGradient)
            .border(BorderStroke(1.dp, topHighlight(0.7f)), containerShape)
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
                    .then(if (selected) Modifier.background(accentGradient) else Modifier)
                    .clickable { onSelected(index) }
                    .padding(vertical = 12.dp)
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.bodyLarge.copy(
                        fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal
                    ),
                    color = if (selected) ClayColors.TextOnEmerald else ClayColors.TextSecondary
                )
            }
        }
    }
}

@Composable
fun ClayStatusBadge(
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
                        val paint = Paint().apply { this.color = color; isAntiAlias = true }
                        val glowPaint = Paint().apply {
                            this.color = color.copy(alpha = 0.35f)
                            isAntiAlias = true
                        }
                        val cx = size.width / 2f
                        val cy = size.height / 2f
                        canvas.drawCircle(Offset(cx, cy), size.width / 2f + 3.dp.toPx(), glowPaint)
                        canvas.drawCircle(Offset(cx, cy), size.width / 2f, paint)
                    }
                }
                .padding(6.dp)
        )
        Text(text = label, style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
fun ClaySectionHeader(title: String, modifier: Modifier = Modifier) {
    Text(
        text = title.uppercase(),
        style = MaterialTheme.typography.bodySmall.copy(
            fontWeight    = FontWeight.Bold,
            letterSpacing = 1.5.sp,
            color         = ClayColors.Emerald,
        ),
        modifier = modifier.padding(bottom = 8.dp)
    )
}

// Backward-compatible aliases — existing callers need no changes
@Composable
fun NeuCard(
    modifier: Modifier = Modifier,
    cornerRadius: Dp = 24.dp,
    content: @Composable () -> Unit
) = ClayCard(modifier, cornerRadius, content)

@Composable
fun NeuButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    isAccent: Boolean = false,
    content: @Composable () -> Unit
) = ClayButton(onClick, modifier, enabled, isAccent, content)

@Composable
fun NeuSegmentedToggle(
    options: List<String>,
    selectedIndex: Int,
    onSelected: (Int) -> Unit,
    modifier: Modifier = Modifier
) = ClaySegmentedToggle(options, selectedIndex, onSelected, modifier)

@Composable
fun NeuStatusBadge(label: String, color: Color, modifier: Modifier = Modifier) =
    ClayStatusBadge(label, color, modifier)

@Composable
fun NeuSectionHeader(title: String, modifier: Modifier = Modifier) =
    ClaySectionHeader(title, modifier)
