package com.sona.android.app.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

// Brand palette (Terracotta / Warm Earth Tones)
val SonaPaper = Color(0xFFFBFBFA)
val SonaSurface = Color(0xFFF3F3F2)
val SonaInk = Color(0xFF37352F)
val SonaTerracotta = Color(0xFF5C4D43)
val SonaRecordingRed = Color(0xFFE03E3E)

val SonaLightColorScheme = lightColorScheme(
    primary = SonaTerracotta,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFF1E5DB),
    onPrimaryContainer = Color(0xFF2B211C),
    secondary = Color(0xFF75655A),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFF2E8E1),
    onSecondaryContainer = SonaInk,
    tertiary = Color(0xFF8C5E47),
    onTertiary = Color.White,
    tertiaryContainer = Color(0xFFFFDCD2),
    onTertiaryContainer = Color(0xFF3B150A),
    background = SonaPaper,
    onBackground = SonaInk,
    surface = SonaPaper,
    onSurface = SonaInk,
    surfaceVariant = Color(0xFFEAE9E7),
    onSurfaceVariant = Color(0xFF787774),
    surfaceContainer = SonaSurface,
    surfaceContainerHigh = Color(0xFFEAE9E7),
    surfaceContainerLow = Color(0xFFF7F6F5),
    surfaceContainerLowest = Color.White,
    outline = Color(0xFF9B9A97),
    outlineVariant = Color(0xFFD8D6D1),
    error = SonaRecordingRed,
    onError = Color.White,
    errorContainer = Color(0xFFFFDAD6),
    onErrorContainer = Color(0xFF410002),
    inverseSurface = Color(0xFF32302D),
    inverseOnSurface = Color(0xFFF6F0EB),
    inversePrimary = Color(0xFFD9C2B2),
    scrim = Color.Black
)

val SonaDarkColorScheme = darkColorScheme(
    primary = Color(0xFFD9C2B2),
    onPrimary = Color(0xFF3A2F29),
    primaryContainer = Color(0xFF524238),
    onPrimaryContainer = Color(0xFFF2E3D9),
    secondary = Color(0xFFD0C2B9),
    onSecondary = Color(0xFF382F2A),
    secondaryContainer = Color(0xFF4B4039),
    onSecondaryContainer = Color(0xFFEDE1DA),
    tertiary = Color(0xFFFFB59B),
    onTertiary = Color(0xFF532B1A),
    tertiaryContainer = Color(0xFF6F442F),
    onTertiaryContainer = Color(0xFFFFDCD2),
    background = Color(0xFF191919),
    onBackground = Color(0xFFD4D4D4),
    surface = Color(0xFF191919),
    onSurface = Color(0xFFD4D4D4),
    surfaceVariant = Color(0xFF2C2C2C),
    onSurfaceVariant = Color(0xFFB2B0AC),
    surfaceContainer = Color(0xFF202020),
    surfaceContainerHigh = Color(0xFF2C2C2C),
    surfaceContainerLow = Color(0xFF1F1F1F),
    surfaceContainerLowest = Color(0xFF0F0F0F),
    outline = Color(0xFF777570),
    outlineVariant = Color(0xFF474747),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
    inverseSurface = Color(0xFFE6E1E0),
    inverseOnSurface = Color(0xFF32302F),
    inversePrimary = SonaTerracotta,
    scrim = Color.Black
)

val LocalSonaRecordingColor = staticCompositionLocalOf { SonaRecordingRed }
