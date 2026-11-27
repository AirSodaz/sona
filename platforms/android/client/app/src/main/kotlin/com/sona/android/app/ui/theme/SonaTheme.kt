package com.sona.android.app.ui.theme

import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.platform.LocalContext

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

    val recordingColor = if (darkTheme) SonaRecordingRedDark else SonaRecordingRed
    val recordingContainer = if (darkTheme) SonaRecordingContainerDark else SonaRecordingContainerLight

    CompositionLocalProvider(
        LocalSonaRecordingColor provides recordingColor,
        LocalSonaRecordingContainerColor provides recordingContainer,
    ) {
        MaterialTheme(
            colorScheme = colorScheme,
            typography = SonaTypography,
            shapes = SonaShapes,
            content = content,
        )
    }
}
