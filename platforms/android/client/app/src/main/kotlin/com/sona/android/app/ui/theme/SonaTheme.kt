package com.sona.android.app.ui.theme

import android.content.Context
import android.os.Build
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MotionScheme
import androidx.compose.material3.dynamicDarkColorScheme
import androidx.compose.material3.dynamicLightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.ThemeMode

internal fun resolveDarkTheme(
    themeMode: ThemeMode,
    systemIsDark: Boolean,
): Boolean = when (themeMode) {
    ThemeMode.FOLLOW_SYSTEM -> systemIsDark
    ThemeMode.LIGHT -> false
    ThemeMode.DARK -> true
}

internal fun resolveSonaColorScheme(
    settings: AppearanceSettings,
    isDark: Boolean,
    context: Context?,
): ColorScheme {
    if (
        settings.colorSource == ColorSource.SYSTEM_DYNAMIC &&
        context != null &&
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    ) {
        val dynamicBase = if (isDark) {
            dynamicDarkColorScheme(context)
        } else {
            dynamicLightColorScheme(context)
        }
        val contrastAdjusted = SonaColorEngine.applyContrastAdjustment(
            scheme = dynamicBase,
            isDark = isDark,
            contrastLevel = settings.contrastLevel,
        )
        return if (isDark && settings.pureBlackDarkEnabled) {
            SonaColorEngine.applyPureBlackSurfaces(contrastAdjusted, settings.contrastLevel)
        } else {
            contrastAdjusted
        }
    }

    return SonaColorEngine.resolveStaticColorScheme(
        settings = settings,
        isDark = isDark,
    )
}

internal fun resolveSonaSemanticColors(
    isDark: Boolean,
    contrastLevel: ContrastLevel,
    pureBlackDarkEnabled: Boolean,
): SonaSemanticColors {
    val base = if (isDark) SonaDarkSemanticColors else SonaLightSemanticColors
    if (contrastLevel == ContrastLevel.STANDARD && !(isDark && pureBlackDarkEnabled)) {
        return base
    }
    return if (!isDark) {
        when (contrastLevel) {
            ContrastLevel.STANDARD -> base
            ContrastLevel.MEDIUM -> base.copy(
                recording = Color(0xFFB71C1C),
                onRecording = Color.White,
                onRecordingContainer = Color(0xFF330001),
                statusIdle = Color(0xFF14532D),
                statusWarning = Color(0xFF92400E),
            )
            ContrastLevel.HIGH -> base.copy(
                recording = Color(0xFF9E1414),
                onRecording = Color.White,
                onRecordingContainer = Color(0xFF280001),
                statusIdle = Color(0xFF0F3D22),
                statusWarning = Color(0xFF78350F),
            )
        }
    } else {
        val recording = when (contrastLevel) {
            ContrastLevel.STANDARD, ContrastLevel.MEDIUM -> base.recording
            ContrastLevel.HIGH -> Color(0xFFFF8984)
        }
        val onRecording = when (contrastLevel) {
            ContrastLevel.STANDARD, ContrastLevel.MEDIUM -> base.onRecording
            ContrastLevel.HIGH -> Color(0xFF1E0000)
        }
        val recordingContainer = if (pureBlackDarkEnabled) {
            Color(0xFF3E0E0C)
        } else {
            base.recordingContainer
        }
        val onRecordingContainer = when (contrastLevel) {
            ContrastLevel.STANDARD -> base.onRecordingContainer
            ContrastLevel.MEDIUM -> Color(0xFFFFE5E3)
            ContrastLevel.HIGH -> Color.White
        }
        val statusIdle = when (contrastLevel) {
            ContrastLevel.STANDARD -> base.statusIdle
            ContrastLevel.MEDIUM -> Color(0xFF86EFAC)
            ContrastLevel.HIGH -> Color(0xFFBBF7D0)
        }
        val statusWarning = when (contrastLevel) {
            ContrastLevel.STANDARD -> base.statusWarning
            ContrastLevel.MEDIUM -> Color(0xFFFCD34D)
            ContrastLevel.HIGH -> Color(0xFFFDE68A)
        }
        base.copy(
            recording = recording,
            onRecording = onRecording,
            recordingContainer = recordingContainer,
            onRecordingContainer = onRecordingContainer,
            statusIdle = statusIdle,
            statusWarning = statusWarning,
        )
    }
}

@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SonaTheme(
    appearanceSettings: AppearanceSettings = AppearanceSettings(),
    systemIsDark: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit,
) {
    val context = LocalContext.current
    val darkTheme = resolveDarkTheme(
        themeMode = appearanceSettings.themeMode,
        systemIsDark = systemIsDark,
    )
    val colorScheme = resolveSonaColorScheme(
        settings = appearanceSettings,
        isDark = darkTheme,
        context = context,
    )
    val semanticColors = resolveSonaSemanticColors(
        isDark = darkTheme,
        contrastLevel = appearanceSettings.contrastLevel,
        pureBlackDarkEnabled = appearanceSettings.pureBlackDarkEnabled,
    )

    CompositionLocalProvider(
        LocalSonaSemanticColors provides semanticColors,
        LocalSonaRecordingColor provides semanticColors.recording,
        LocalSonaRecordingContainerColor provides semanticColors.recordingContainer,
    ) {
        MaterialTheme(
            colorScheme = colorScheme,
            motionScheme = MotionScheme.expressive(),
            typography = SonaTypography,
            shapes = SonaShapes,
            content = content,
        )
    }
}
