package com.sona.android.app.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

// M3 Expressive fallback brand palette (Warm Ink + Vibrant Terracotta / Coral Recording Accent)
val SonaPaper = Color(0xFFFCFBF9)
val SonaSurface = Color(0xFFF3F1ED)
val SonaInk = Color(0xFF1E1C1A)
val SonaTerracotta = Color(0xFF7B4E3B)
val SonaRecordingRed = Color(0xFFE53935)
val SonaRecordingRedDark = Color(0xFFFF5A54)
val SonaRecordingContainerLight = Color(0xFFFFE5E3)
val SonaRecordingContainerDark = Color(0xFF5A1614)

val SonaLightColorScheme = lightColorScheme(
    primary = SonaTerracotta,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFFFDBC9),
    onPrimaryContainer = Color(0xFF2E1509),
    secondary = Color(0xFF6B5B51),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFF5E3D8),
    onSecondaryContainer = Color(0xFF251A13),
    tertiary = Color(0xFF47644B),
    onTertiary = Color.White,
    tertiaryContainer = Color(0xFFC9EBC9),
    onTertiaryContainer = Color(0xFF04210C),
    background = SonaPaper,
    onBackground = SonaInk,
    surface = SonaPaper,
    onSurface = SonaInk,
    surfaceVariant = Color(0xFFEAE5DF),
    onSurfaceVariant = Color(0xFF534D48),
    surfaceContainerLowest = Color.White,
    surfaceContainerLow = Color(0xFFF7F5F2),
    surfaceContainer = SonaSurface,
    surfaceContainerHigh = Color(0xFFEDEAE4),
    surfaceContainerHighest = Color(0xFFE4E0D9),
    outline = Color(0xFF847D77),
    outlineVariant = Color(0xFFD5CFC7),
    error = Color(0xFFBA1A1A),
    onError = Color.White,
    errorContainer = Color(0xFFFFDAD6),
    onErrorContainer = Color(0xFF410002),
    inverseSurface = Color(0xFF33302D),
    inverseOnSurface = Color(0xFFF7F2EC),
    inversePrimary = Color(0xFFF0B89F),
    scrim = Color.Black,
)

val SonaDarkColorScheme = darkColorScheme(
    primary = Color(0xFFF0B89F),
    onPrimary = Color(0xFF482515),
    primaryContainer = Color(0xFF623826),
    onPrimaryContainer = Color(0xFFFFDBC9),
    secondary = Color(0xFFDCC3B5),
    onSecondary = Color(0xFF3E2D24),
    secondaryContainer = Color(0xFF544339),
    onSecondaryContainer = Color(0xFFF9E2D5),
    tertiary = Color(0xFFADCEAF),
    onTertiary = Color(0xFF1A3620),
    tertiaryContainer = Color(0xFF304D35),
    onTertiaryContainer = Color(0xFFC9EBC9),
    background = Color(0xFF141312),
    onBackground = Color(0xFFE9E2DC),
    surface = Color(0xFF141312),
    onSurface = Color(0xFFE9E2DC),
    surfaceVariant = Color(0xFF312D2A),
    onSurfaceVariant = Color(0xFFC9C1BA),
    surfaceContainerLowest = Color(0xFF0E0D0C),
    surfaceContainerLow = Color(0xFF1C1A18),
    surfaceContainer = Color(0xFF211F1C),
    surfaceContainerHigh = Color(0xFF2B2825),
    surfaceContainerHighest = Color(0xFF36332F),
    outline = Color(0xFF938B84),
    outlineVariant = Color(0xFF4B4540),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
    errorContainer = Color(0xFF93000A),
    onErrorContainer = Color(0xFFFFDAD6),
    inverseSurface = Color(0xFFE9E2DC),
    inverseOnSurface = Color(0xFF322F2C),
    inversePrimary = SonaTerracotta,
    scrim = Color.Black,
)

val LocalSonaRecordingColor = staticCompositionLocalOf { SonaRecordingRed }
val LocalSonaRecordingContainerColor = staticCompositionLocalOf { SonaRecordingContainerLight }
