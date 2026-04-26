package com.clipsync.ui.theme

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

object ClayColors {
    // Backgrounds
    val Background        = Color(0xFFFFFFFF)
    val Surface           = Color(0xFFFFFFFF)
    val SurfaceGradTop    = Color(0xFFFFFFFF)
    val SurfaceGradBottom = Color(0xFFF2FFF8)
    val SurfaceVariant    = Color(0xFFE4F5EC)

    // Emerald greens
    val Emerald       = Color(0xFF2ECC76)
    val EmeraldLight  = Color(0xFF4BDD8A)
    val EmeraldDark   = Color(0xFF1AAD5A)
    val EmeraldShadow = Color(0x261AAD5A)

    // Status
    val Connected    = Color(0xFF2ECC76)
    val Disconnected = Color(0xFFB0C8BC)
    val Error        = Color(0xFFFF6B6B)

    // Text
    val TextPrimary   = Color(0xFF1A2E22)
    val TextSecondary = Color(0xFF5A7A66)
    val TextOnEmerald = Color.White
}

@Suppress("unused")
object NeuColors {
    val Background     get() = ClayColors.Background
    val Surface        get() = ClayColors.Surface
    val SurfaceVariant get() = ClayColors.SurfaceVariant
    val LightShadow    get() = Color.White
    val DarkShadow     get() = ClayColors.Disconnected
    val Accent         get() = ClayColors.Emerald
    val AccentDark     get() = ClayColors.EmeraldDark
    val Connected      get() = ClayColors.Connected
    val Disconnected   get() = ClayColors.Disconnected
    val Error          get() = ClayColors.Error
    val TextPrimary    get() = ClayColors.TextPrimary
    val TextSecondary  get() = ClayColors.TextSecondary
    val TextOnAccent   get() = ClayColors.TextOnEmerald
}

data class ClayColorScheme(
    val emerald: Color       = ClayColors.Emerald,
    val connected: Color     = ClayColors.Connected,
    val disconnected: Color  = ClayColors.Disconnected,
    val textSecondary: Color = ClayColors.TextSecondary,
)

val LocalClayColors = staticCompositionLocalOf { ClayColorScheme() }

private val ClipSyncColorScheme = lightColorScheme(
    primary        = ClayColors.Emerald,
    onPrimary      = ClayColors.TextOnEmerald,
    secondary      = ClayColors.EmeraldDark,
    background     = ClayColors.Background,
    surface        = ClayColors.Surface,
    surfaceVariant = ClayColors.SurfaceVariant,
    onBackground   = ClayColors.TextPrimary,
    onSurface      = ClayColors.TextPrimary,
    error          = ClayColors.Error,
)

private val ClipSyncTypography = Typography(
    headlineLarge = TextStyle(
        fontFamily    = FontFamily.Default,
        fontWeight    = FontWeight.Bold,
        fontSize      = 28.sp,
        letterSpacing = (-0.5).sp,
        color         = ClayColors.TextPrimary,
    ),
    headlineMedium = TextStyle(
        fontFamily    = FontFamily.Default,
        fontWeight    = FontWeight.Bold,
        fontSize      = 22.sp,
        letterSpacing = (-0.3).sp,
        color         = ClayColors.TextPrimary,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize   = 16.sp,
        color      = ClayColors.TextPrimary,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 16.sp,
        color      = ClayColors.TextPrimary,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 14.sp,
        color      = ClayColors.TextSecondary,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 12.sp,
        color      = ClayColors.TextSecondary,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize   = 14.sp,
        color      = ClayColors.TextOnEmerald,
    ),
)

@Composable
fun ClipSyncTheme(content: @Composable () -> Unit) {
    CompositionLocalProvider(LocalClayColors provides ClayColorScheme()) {
        MaterialTheme(
            colorScheme = ClipSyncColorScheme,
            typography  = ClipSyncTypography,
            content     = content,
        )
    }
}
