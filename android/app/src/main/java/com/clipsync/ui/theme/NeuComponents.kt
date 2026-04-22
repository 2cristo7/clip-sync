package com.clipsync.ui.theme

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.interaction.collectIsPressedAsState
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
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
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import me.nikhilchaudhari.library.neumorphic
import me.nikhilchaudhari.library.shapes.Punched
import me.nikhilchaudhari.library.shapes.Pressed

/**
 * Reusable neumorphic UI components for the ClipSync design system.
 *
 * Uses [neumorphic-compose](https://github.com/CuriousNikhil/neumorphic-compose)
 * for the core shadow rendering, wrapped in composable helpers with consistent
 * styling (radius, shadows, elevation) so the app looks cohesive.
 */

/**
 * A card with an extruded (punched) neumorphic look.
 */
@Composable
fun NeuCard(
    modifier: Modifier = Modifier,
    cornerRadius: Dp = 16.dp,
    content: @Composable () -> Unit
) {
    val neu = LocalNeuColors.current
    Box(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(cornerRadius))
            .neumorphic(
                neuShape = Punched.Rounded(radius = cornerRadius),
                lightShadowColor = neu.lightShadow,
                darkShadowColor = neu.darkShadow,
                elevation = 6.dp,
                strokeWidth = 6.dp,
            )
            .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(cornerRadius))
            .padding(16.dp)
    ) {
        content()
    }
}

/**
 * A neumorphic button with a pressed state that switches from punched to pressed.
 */
@Composable
fun NeuButton(
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
    enabled: Boolean = true,
    isAccent: Boolean = false,
    content: @Composable () -> Unit
) {
    val neu = LocalNeuColors.current
    val interactionSource = remember { MutableInteractionSource() }
    val isPressed by interactionSource.collectIsPressedAsState()
    val scale by animateFloatAsState(
        targetValue = if (isPressed) 0.96f else 1f,
        label = "btn_scale"
    )

    val bgColor = if (isAccent) neu.accent else MaterialTheme.colorScheme.surface
    val textColor = if (isAccent) NeuColors.TextOnAccent else NeuColors.TextPrimary

    Box(
        contentAlignment = Alignment.Center,
        modifier = modifier
            .graphicsLayer {
                scaleX = scale
                scaleY = scale
            }
            .clip(RoundedCornerShape(12.dp))
            .neumorphic(
                neuShape = if (isPressed) Pressed.Rounded(radius = 12.dp)
                           else Punched.Rounded(radius = 12.dp),
                lightShadowColor = if (isAccent) NeuColors.AccentDark else neu.lightShadow,
                darkShadowColor = if (isAccent) Color(0xFF4A42C0) else neu.darkShadow,
                elevation = if (isPressed) 2.dp else 4.dp,
                strokeWidth = 4.dp,
            )
            .background(bgColor, RoundedCornerShape(12.dp))
            .clickable(
                interactionSource = interactionSource,
                indication = null,
                enabled = enabled,
                onClick = onClick
            )
            .padding(horizontal = 20.dp, vertical = 12.dp)
    ) {
        content()
    }
}

/**
 * A neumorphic toggle that visually switches between punched (off) and
 * pressed (on) states, similar to a physical push switch.
 */
@Composable
fun NeuSegmentedToggle(
    options: List<String>,
    selectedIndex: Int,
    onSelected: (Int) -> Unit,
    modifier: Modifier = Modifier
) {
    val neu = LocalNeuColors.current
    Row(
        modifier = modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(12.dp))
            .background(MaterialTheme.colorScheme.surface, RoundedCornerShape(12.dp)),
        horizontalArrangement = Arrangement.spacedBy(0.dp)
    ) {
        options.forEachIndexed { index, label ->
            val selected = index == selectedIndex
            Box(
                contentAlignment = Alignment.Center,
                modifier = Modifier
                    .weight(1f)
                    .clip(RoundedCornerShape(12.dp))
                    .neumorphic(
                        neuShape = if (selected) Pressed.Rounded(radius = 12.dp)
                                   else Punched.Rounded(radius = 12.dp),
                        lightShadowColor = neu.lightShadow,
                        darkShadowColor = neu.darkShadow,
                        elevation = if (selected) 2.dp else 4.dp,
                        strokeWidth = 4.dp,
                    )
                    .background(
                        if (selected) NeuColors.SurfaceVariant
                        else MaterialTheme.colorScheme.surface,
                        RoundedCornerShape(12.dp)
                    )
                    .clickable { onSelected(index) }
                    .padding(vertical = 12.dp)
            ) {
                Text(
                    text = label,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (selected) NeuColors.Accent else NeuColors.TextSecondary
                )
            }
        }
    }
}

/**
 * Status indicator dot with neumorphic inset effect.
 */
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
                        val paint = Paint().apply {
                            this.color = color
                            isAntiAlias = true
                        }
                        // Glow behind the dot
                        val glowPaint = Paint().apply {
                            this.color = color.copy(alpha = 0.3f)
                            isAntiAlias = true
                        }
                        canvas.drawCircle(
                            center = Offset(size.width / 2, size.height / 2),
                            radius = size.width / 2 + 2.dp.toPx(),
                            paint = glowPaint
                        )
                        canvas.drawCircle(
                            center = Offset(size.width / 2, size.height / 2),
                            radius = size.width / 2,
                            paint = paint
                        )
                    }
                }
                .padding(4.dp)
        )
        Text(
            text = label,
            style = MaterialTheme.typography.bodyLarge,
        )
    }
}

/**
 * Section header for settings groups.
 */
@Composable
fun NeuSectionHeader(
    title: String,
    modifier: Modifier = Modifier
) {
    Text(
        text = title.uppercase(),
        style = MaterialTheme.typography.bodySmall.copy(
            fontWeight = androidx.compose.ui.text.font.FontWeight.Bold,
            letterSpacing = 1.5.sp,
            color = NeuColors.TextSecondary
        ),
        modifier = modifier.padding(bottom = 8.dp)
    )
}
