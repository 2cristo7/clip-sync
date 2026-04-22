package com.clipsync.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

/**
 * ClipSync neumorphic-inspired colour palette.
 *
 * Neumorphism works best on a warm, muted, light surface where the light and
 * dark shadows create the characteristic "extruded" look. We pair this with
 * an accent purple for interactive elements.
 */
object NeuColors {
    // Surface / background
    val Background = Color(0xFFECE9E6)
    val Surface = Color(0xFFECE9E6)
    val SurfaceVariant = Color(0xFFE2DFDC)

    // Shadows
    val LightShadow = Color(0xFFFFFFFF)
    val DarkShadow = Color(0xFFBABECC)

    // Accent
    val Accent = Color(0xFF6C63FF)
    val AccentDark = Color(0xFF5A52E0)

    // Status
    val Connected = Color(0xFF4CAF50)
    val Disconnected = Color(0xFF9E9E9E)
    val Error = Color(0xFFE57373)

    // Text
    val TextPrimary = Color(0xFF2D3436)
    val TextSecondary = Color(0xFF636E72)
    val TextOnAccent = Color.White

    // FAB clay colours
    val ClayGradientStart = Color(0xFFA8E6CF)
    val ClayGradientEnd = Color(0xFF7BC8A4)
    val ClayShadow = Color(0x40000000)
    val ClayHighlight = Color(0x40FFFFFF)
}

/** Extra colour tokens exposed via [LocalNeuColors]. */
data class NeuColorScheme(
    val lightShadow: Color = NeuColors.LightShadow,
    val darkShadow: Color = NeuColors.DarkShadow,
    val connected: Color = NeuColors.Connected,
    val disconnected: Color = NeuColors.Disconnected,
    val textSecondary: Color = NeuColors.TextSecondary,
    val accent: Color = NeuColors.Accent,
)

val LocalNeuColors = staticCompositionLocalOf { NeuColorScheme() }

private val ClipSyncColorScheme = lightColorScheme(
    primary = NeuColors.Accent,
    onPrimary = NeuColors.TextOnAccent,
    secondary = NeuColors.AccentDark,
    background = NeuColors.Background,
    surface = NeuColors.Surface,
    surfaceVariant = NeuColors.SurfaceVariant,
    onBackground = NeuColors.TextPrimary,
    onSurface = NeuColors.TextPrimary,
    error = NeuColors.Error,
)

private val ClipSyncTypography = Typography(
    headlineLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Bold,
        fontSize = 28.sp,
        letterSpacing = (-0.5).sp,
        color = NeuColors.TextPrimary,
    ),
    headlineMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Bold,
        fontSize = 22.sp,
        letterSpacing = (-0.3).sp,
        color = NeuColors.TextPrimary,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize = 16.sp,
        color = NeuColors.TextPrimary,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize = 16.sp,
        color = NeuColors.TextPrimary,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize = 14.sp,
        color = NeuColors.TextSecondary,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize = 12.sp,
        color = NeuColors.TextSecondary,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize = 14.sp,
        color = NeuColors.TextOnAccent,
    ),
)

@Composable
fun ClipSyncTheme(content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalNeuColors provides NeuColorScheme()) {
        MaterialTheme(
            colorScheme = ClipSyncColorScheme,
            typography = ClipSyncTypography,
            content = content,
        )
    }
}
