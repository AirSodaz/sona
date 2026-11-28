package com.sona.android.app.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Immutable
import androidx.compose.runtime.staticCompositionLocalOf
import androidx.compose.ui.graphics.Color

// M3 Expressive fallback brand palette (Warm Ink + Vibrant Terracotta / Coral Recording Accent)
val SonaPaper = Color(0xFFFCFBF9)
val SonaSurface = Color(0xFFF3F1ED)
val SonaInk = Color(0xFF1E1C1A)
val SonaTerracotta = Color(0xFF7B4E3B)
val SonaRecordingRed = Color(0xFFD32F2F)
val SonaRecordingRedDark = Color(0xFFFF5A54)
val SonaRecordingContainerLight = Color(0xFFFFE5E3)
val SonaRecordingContainerDark = Color(0xFF5A1614)

@Immutable
data class SonaSemanticColors(
    val recording: Color,
    val onRecording: Color,
    val recordingContainer: Color,
    val onRecordingContainer: Color,
    val statusIdle: Color,
    val statusWarning: Color,
)

val SonaLightSemanticColors = SonaSemanticColors(
    recording = SonaRecordingRed,
    onRecording = Color.White,
    recordingContainer = SonaRecordingContainerLight,
    onRecordingContainer = Color(0xFF410002),
    statusIdle = Color(0xFF15803D),
    statusWarning = Color(0xFFB45309),
)

val SonaDarkSemanticColors = SonaSemanticColors(
    recording = SonaRecordingRedDark,
    onRecording = Color(0xFF2B0202),
    recordingContainer = SonaRecordingContainerDark,
    onRecordingContainer = Color(0xFFFFDAD6),
    statusIdle = Color(0xFF4ADE80),
    statusWarning = Color(0xFFFBBF24),
)

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
    surfaceDim = Color(0xFFDFDBD4),
    surfaceBright = SonaPaper,
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
    primaryFixed = Color(0xFFFFDBC9),
    onPrimaryFixed = Color(0xFF2E1509),
    primaryFixedDim = Color(0xFFF0B89F),
    onPrimaryFixedVariant = Color(0xFF623826),
    secondaryFixed = Color(0xFFF5E3D8),
    onSecondaryFixed = Color(0xFF251A13),
    secondaryFixedDim = Color(0xFFDCC3B5),
    onSecondaryFixedVariant = Color(0xFF544339),
    tertiaryFixed = Color(0xFFC9EBC9),
    onTertiaryFixed = Color(0xFF04210C),
    tertiaryFixedDim = Color(0xFFADCEAF),
    onTertiaryFixedVariant = Color(0xFF304D35),
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
    surfaceDim = Color(0xFF141312),
    surfaceBright = Color(0xFF3B3834),
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
    primaryFixed = Color(0xFFFFDBC9),
    onPrimaryFixed = Color(0xFF2E1509),
    primaryFixedDim = Color(0xFFF0B89F),
    onPrimaryFixedVariant = Color(0xFF623826),
    secondaryFixed = Color(0xFFF5E3D8),
    onSecondaryFixed = Color(0xFF251A13),
    secondaryFixedDim = Color(0xFFDCC3B5),
    onSecondaryFixedVariant = Color(0xFF544339),
    tertiaryFixed = Color(0xFFC9EBC9),
    onTertiaryFixed = Color(0xFF04210C),
    tertiaryFixedDim = Color(0xFFADCEAF),
    onTertiaryFixedVariant = Color(0xFF304D35),
)

val LocalSonaRecordingColor = staticCompositionLocalOf { SonaRecordingRed }
val LocalSonaRecordingContainerColor = staticCompositionLocalOf { SonaRecordingContainerLight }
val LocalSonaSemanticColors = staticCompositionLocalOf { SonaLightSemanticColors }
