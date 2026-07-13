package com.sona.android.app.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext

private val SonaPaper = Color(0xFFFBFBFA)
private val SonaSurface = Color(0xFFF3F3F2)
private val SonaInk = Color(0xFF37352F)
private val SonaTerracotta = Color(0xFF5C4D43)
private val SonaRecording = Color(0xFFE03E3E)

private val SonaLightColorScheme = lightColorScheme(
    primary = SonaTerracotta,
    onPrimary = Color.White,
    primaryContainer = Color(0xFFF1E5DB),
    onPrimaryContainer = Color(0xFF2B211C),
    secondary = Color(0xFF75655A),
    onSecondary = Color.White,
    secondaryContainer = Color(0xFFF2E8E1),
    onSecondaryContainer = SonaInk,
    background = SonaPaper,
    onBackground = SonaInk,
    surface = SonaPaper,
    onSurface = SonaInk,
    surfaceContainer = SonaSurface,
    surfaceContainerHigh = Color(0xFFEAE9E7),
    onSurfaceVariant = Color(0xFF787774),
    outline = Color(0xFF9B9A97),
    outlineVariant = Color(0xFFD8D6D1),
    error = SonaRecording,
    onError = Color.White,
)

private val SonaDarkColorScheme = darkColorScheme(
    primary = Color(0xFFD9C2B2),
    onPrimary = Color(0xFF3A2F29),
    primaryContainer = Color(0xFF524238),
    onPrimaryContainer = Color(0xFFF2E3D9),
    secondary = Color(0xFFD0C2B9),
    onSecondary = Color(0xFF382F2A),
    secondaryContainer = Color(0xFF4B4039),
    onSecondaryContainer = Color(0xFFEDE1DA),
    background = Color(0xFF191919),
    onBackground = Color(0xFFD4D4D4),
    surface = Color(0xFF191919),
    onSurface = Color(0xFFD4D4D4),
    surfaceContainer = Color(0xFF202020),
    surfaceContainerHigh = Color(0xFF2C2C2C),
    onSurfaceVariant = Color(0xFFB2B0AC),
    outline = Color(0xFF777570),
    outlineVariant = Color(0xFF474747),
    error = Color(0xFFFFB4AB),
    onError = Color(0xFF690005),
)

@Composable
fun SonaTheme(
    dynamicColorEnabled: Boolean,
    darkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val colorScheme = when {
        dynamicColorEnabled && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S && darkTheme ->
            dynamicDarkColorScheme(context)
        dynamicColorEnabled && Build.VERSION.SDK_INT >= Build.VERSION_CODES.S ->
            dynamicLightColorScheme(context)
        darkTheme -> SonaDarkColorScheme
        else -> SonaLightColorScheme
    }

    MaterialTheme(
        colorScheme = colorScheme,
        content = content,
    )
}
