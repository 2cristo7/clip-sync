package com.clipsync.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.compositionLocalOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

data class NeuPalette(
    val background: Color,
    val surface: Color,
    val surfaceRaised: Color,
    val surfaceInset: Color,
    val border: Color,
    val accent: Color,
    val accentLight: Color,
    val accentDark: Color,
    val accentGlow: Color,
    val accentSubtle: Color,
    val connected: Color,
    val disconnected: Color,
    val error: Color,
    val warning: Color,
    val textPrimary: Color,
    val textSecondary: Color,
    val textOnAccent: Color,
    val isDark: Boolean,
)

val DarkPalette = NeuPalette(
    background     = Color(0xFF181D2A),
    surface        = Color(0xFF1F2535),
    surfaceRaised  = Color(0xFF252C3E),
    surfaceInset   = Color(0xFF141825),
    border         = Color(0xFF2E3548),
    accent         = Color(0xFF5AEAAA),
    accentLight    = Color(0xFF7DF2C0),
    accentDark     = Color(0xFF38D690),
    accentGlow     = Color(0x305AEAAA),
    accentSubtle   = Color(0x185AEAAA),
    connected      = Color(0xFF5AEAAA),
    disconnected   = Color(0xFF4A5568),
    error          = Color(0xFFFF6B7A),
    warning        = Color(0xFFFFBB5C),
    textPrimary    = Color(0xFFE4E8F0),
    textSecondary  = Color(0xFF7B869C),
    textOnAccent   = Color(0xFF0A1A12),
    isDark         = true,
)

val LightPalette = NeuPalette(
    background     = Color(0xFFF2F4F8),
    surface        = Color(0xFFFFFFFF),
    surfaceRaised  = Color(0xFFFFFFFF),
    surfaceInset   = Color(0xFFE8ECF2),
    border         = Color(0xFFD8DEE8),
    accent         = Color(0xFF2ECC76),
    accentLight    = Color(0xFF4BDD8A),
    accentDark     = Color(0xFF1AAD5A),
    accentGlow     = Color(0x302ECC76),
    accentSubtle   = Color(0x182ECC76),
    connected      = Color(0xFF2ECC76),
    disconnected   = Color(0xFFB0B8C8),
    error          = Color(0xFFE8534E),
    warning        = Color(0xFFE89B2D),
    textPrimary    = Color(0xFF1A2030),
    textSecondary  = Color(0xFF6B7588),
    textOnAccent   = Color.White,
    isDark         = false,
)

val LocalPalette = staticCompositionLocalOf { DarkPalette }

object NeuColors {
    private var p: NeuPalette by mutableStateOf(DarkPalette)

    internal fun update(palette: NeuPalette) { p = palette }

    val Background     get() = p.background
    val Surface        get() = p.surface
    val SurfaceRaised  get() = p.surfaceRaised
    val SurfaceInset   get() = p.surfaceInset
    val Border         get() = p.border
    val Accent         get() = p.accent
    val AccentLight    get() = p.accentLight
    val AccentDark     get() = p.accentDark
    val AccentGlow     get() = p.accentGlow
    val AccentSubtle   get() = p.accentSubtle
    val Connected      get() = p.connected
    val Disconnected   get() = p.disconnected
    val Error          get() = p.error
    val Warning        get() = p.warning
    val TextPrimary    get() = p.textPrimary
    val TextSecondary  get() = p.textSecondary
    val TextOnAccent   get() = p.textOnAccent
    val SurfaceVariant get() = p.surfaceRaised
    val LightShadow    get() = p.border
    val DarkShadow     get() = p.surfaceInset
}

@Suppress("unused")
object ClayColors {
    val Background     get() = NeuColors.Background
    val Surface        get() = NeuColors.Surface
    val SurfaceVariant get() = NeuColors.SurfaceVariant
    val SurfaceGradTop get() = NeuColors.SurfaceRaised
    val SurfaceGradBottom get() = NeuColors.SurfaceInset
    val Emerald        get() = NeuColors.Accent
    val EmeraldLight   get() = NeuColors.AccentLight
    val EmeraldDark    get() = NeuColors.AccentDark
    val EmeraldShadow  get() = NeuColors.AccentGlow
    val Connected      get() = NeuColors.Connected
    val Disconnected   get() = NeuColors.Disconnected
    val Error          get() = NeuColors.Error
    val TextPrimary    get() = NeuColors.TextPrimary
    val TextSecondary  get() = NeuColors.TextSecondary
    val TextOnEmerald  get() = NeuColors.TextOnAccent
}

private fun buildColorScheme(p: NeuPalette) = if (p.isDark) {
    darkColorScheme(
        primary        = p.accent,
        onPrimary      = p.textOnAccent,
        secondary      = p.accentDark,
        background     = p.background,
        surface        = p.surface,
        surfaceVariant = p.surfaceRaised,
        onBackground   = p.textPrimary,
        onSurface      = p.textPrimary,
        error          = p.error,
    )
} else {
    lightColorScheme(
        primary        = p.accent,
        onPrimary      = p.textOnAccent,
        secondary      = p.accentDark,
        background     = p.background,
        surface        = p.surface,
        surfaceVariant = p.surfaceRaised,
        onBackground   = p.textPrimary,
        onSurface      = p.textPrimary,
        error          = p.error,
    )
}

private fun buildTypography(p: NeuPalette) = Typography(
    headlineLarge = TextStyle(
        fontFamily    = FontFamily.Default,
        fontWeight    = FontWeight.Bold,
        fontSize      = 28.sp,
        letterSpacing = (-0.5).sp,
        color         = p.textPrimary,
    ),
    headlineMedium = TextStyle(
        fontFamily    = FontFamily.Default,
        fontWeight    = FontWeight.Bold,
        fontSize      = 22.sp,
        letterSpacing = (-0.3).sp,
        color         = p.textPrimary,
    ),
    titleMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize   = 16.sp,
        color      = p.textPrimary,
    ),
    bodyLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 16.sp,
        color      = p.textPrimary,
    ),
    bodyMedium = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 14.sp,
        color      = p.textSecondary,
    ),
    bodySmall = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Normal,
        fontSize   = 12.sp,
        color      = p.textSecondary,
    ),
    labelLarge = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.SemiBold,
        fontSize   = 14.sp,
        color      = p.textOnAccent,
    ),
)

@Composable
fun ClipSyncTheme(
    isDark: Boolean = true,
    content: @Composable () -> Unit,
) {
    val palette = if (isDark) DarkPalette else LightPalette
    NeuColors.update(palette)

    CompositionLocalProvider(LocalPalette provides palette) {
        MaterialTheme(
            colorScheme = buildColorScheme(palette),
            typography  = buildTypography(palette),
            content     = content,
        )
    }
}
