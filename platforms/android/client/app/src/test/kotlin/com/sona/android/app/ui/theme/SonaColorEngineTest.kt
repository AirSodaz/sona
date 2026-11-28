package com.sona.android.app.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class SonaColorEngineTest {
    @Test
    fun `resolveDarkTheme obeys theme mode overrides`() {
        assertTrue(resolveDarkTheme(ThemeMode.FOLLOW_SYSTEM, systemIsDark = true))
        assertFalse(resolveDarkTheme(ThemeMode.FOLLOW_SYSTEM, systemIsDark = false))
        assertFalse(resolveDarkTheme(ThemeMode.LIGHT, systemIsDark = true))
        assertTrue(resolveDarkTheme(ThemeMode.DARK, systemIsDark = false))
    }

    @Test
    fun `default terracotta preset preserves exact brand light and dark schemes`() {
        val defaults = AppearanceSettings()
        assertEquals(
            SonaLightColorScheme,
            SonaColorEngine.resolveStaticColorScheme(defaults, isDark = false),
        )
        assertEquals(
            SonaDarkColorScheme,
            SonaColorEngine.resolveStaticColorScheme(defaults, isDark = true),
        )
    }

    @Test
    fun `pure black dark mode overrides dark surfaces and preserves outline contrast`() {
        val pureBlack = AppearanceSettings(
            themeMode = ThemeMode.DARK,
            pureBlackDarkEnabled = true,
            palettePreset = ColorPalettePreset.OCEAN_SLATE,
        )
        val darkScheme = SonaColorEngine.resolveStaticColorScheme(pureBlack, isDark = true)
        assertEquals(Color.Black, darkScheme.background)
        assertEquals(Color.Black, darkScheme.surface)
        assertEquals(Color.Black, darkScheme.surfaceDim)
        assertEquals(Color.Black, darkScheme.surfaceContainerLowest)
        assertTrue(
            SonaColorEngine.contrastRatio(darkScheme.outlineVariant, darkScheme.surfaceContainer) >= 1.6,
        )
        assertTrue(
            SonaColorEngine.contrastRatio(darkScheme.onSurfaceVariant, darkScheme.surfaceContainerHighest) >= 4.5,
        )

        val lightScheme = SonaColorEngine.resolveStaticColorScheme(pureBlack, isDark = false)
        assertNotEquals(Color.Black, lightScheme.background)
    }

    @Test
    fun `all preset, style, contrast and polarity combinations satisfy WCAG text contrast`() {
        for (preset in ColorPalettePreset.entries) {
            for (style in PaletteStyle.entries) {
                for (contrast in ContrastLevel.entries) {
                    val minContrast = when (contrast) {
                        ContrastLevel.STANDARD -> 4.5
                        ContrastLevel.MEDIUM -> 5.5
                        ContrastLevel.HIGH -> 7.0
                    }
                    val settings = AppearanceSettings(
                        colorSource = ColorSource.PRESET,
                        palettePreset = preset,
                        paletteStyle = style,
                        contrastLevel = contrast,
                    )
                    for (isDark in listOf(false, true)) {
                        val scheme = SonaColorEngine.resolveStaticColorScheme(settings, isDark = isDark)
                        val label = "$preset/$style/$contrast/dark=$isDark"
                        assertTrue(
                            "onSurface contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onSurface, scheme.surfaceContainerHighest) >= minContrast,
                        )
                        assertTrue(
                            "onSurfaceVariant contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onSurfaceVariant, scheme.surfaceContainerHighest) >= minContrast,
                        )
                        assertTrue(
                            "onPrimary contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onPrimary, scheme.primary) >= minContrast,
                        )
                        assertTrue(
                            "onSecondary contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onSecondary, scheme.secondary) >= minContrast,
                        )
                        assertTrue(
                            "onTertiary contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onTertiary, scheme.tertiary) >= minContrast,
                        )
                        assertTrue(
                            "onError contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onError, scheme.error) >= minContrast,
                        )
                        assertTrue(
                            "onPrimaryContainer contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.onPrimaryContainer, scheme.primaryContainer) >= minContrast,
                        )
                        assertTrue(
                            "inverseOnSurface contrast failed for $label",
                            SonaColorEngine.contrastRatio(scheme.inverseOnSurface, scheme.inverseSurface) >= minContrast,
                        )
                        assertNotEquals(Color.Unspecified, scheme.surfaceDim)
                        assertNotEquals(Color.Unspecified, scheme.surfaceBright)
                        assertNotEquals(Color.Unspecified, scheme.primaryFixed)
                    }
                }
            }
        }
    }

    @Test
    fun `applyContrastAdjustment enforces medium and high contrast on baseline schemes`() {
        val baselineLight = lightColorScheme()
        val baselineDark = darkColorScheme()

        for (contrast in listOf(ContrastLevel.MEDIUM, ContrastLevel.HIGH)) {
            val minContrast = if (contrast == ContrastLevel.MEDIUM) 5.5 else 7.0
            val adjustedLight = SonaColorEngine.applyContrastAdjustment(baselineLight, isDark = false, contrastLevel = contrast)
            val adjustedDark = SonaColorEngine.applyContrastAdjustment(baselineDark, isDark = true, contrastLevel = contrast)

            for ((name, scheme) in listOf("light/$contrast" to adjustedLight, "dark/$contrast" to adjustedDark)) {
                assertTrue(
                    "onPrimary failed for $name",
                    SonaColorEngine.contrastRatio(scheme.onPrimary, scheme.primary) >= minContrast,
                )
                assertTrue(
                    "onSecondary failed for $name",
                    SonaColorEngine.contrastRatio(scheme.onSecondary, scheme.secondary) >= minContrast,
                )
                assertTrue(
                    "onTertiary failed for $name",
                    SonaColorEngine.contrastRatio(scheme.onTertiary, scheme.tertiary) >= minContrast,
                )
                assertTrue(
                    "onError failed for $name",
                    SonaColorEngine.contrastRatio(scheme.onError, scheme.error) >= minContrast,
                )
                assertTrue(
                    "onSurfaceVariant failed for $name",
                    SonaColorEngine.contrastRatio(scheme.onSurfaceVariant, scheme.surfaceContainerHighest) >= minContrast,
                )
                assertTrue(
                    "inverseOnSurface failed for $name",
                    SonaColorEngine.contrastRatio(scheme.inverseOnSurface, scheme.inverseSurface) >= minContrast,
                )
            }
        }
    }

    @Test
    fun `achromatic and extreme custom seeds produce grayscale neutral schemes without chromatic drift`() {
        val achromaticSeeds = listOf(0xFF000000.toInt(), 0xFFFFFFFF.toInt(), 0xFF808080.toInt())
        for (seed in achromaticSeeds) {
            for (style in PaletteStyle.entries) {
                val scheme = SonaColorEngine.generateColorScheme(
                    seedArgb = seed,
                    style = style,
                    contrastLevel = ContrastLevel.STANDARD,
                    isDark = false,
                )
                assertEquals(scheme.primary.red, scheme.primary.green, 0.02f)
                assertEquals(scheme.primary.green, scheme.primary.blue, 0.02f)
                assertEquals(scheme.surfaceContainer.red, scheme.surfaceContainer.blue, 0.02f)
            }
        }

        for (extremeSeed in listOf(0xFFFFFF00.toInt(), 0xFF00FF00.toInt())) {
            val scheme = SonaColorEngine.generateColorScheme(
                seedArgb = extremeSeed,
                style = PaletteStyle.VIBRANT,
                contrastLevel = ContrastLevel.HIGH,
                isDark = true,
            )
            assertTrue(SonaColorEngine.contrastRatio(scheme.onPrimary, scheme.primary) >= 7.0)
            assertTrue(SonaColorEngine.contrastRatio(scheme.onSurface, scheme.surface) >= 7.0)
        }
    }

    @Test
    fun `resolveSonaSemanticColors preserves contrast across all modes`() {
        for (isDark in listOf(false, true)) {
            for (pureBlack in listOf(false, true)) {
                for (contrast in ContrastLevel.entries) {
                    val minContrast = when (contrast) {
                        ContrastLevel.STANDARD -> 4.5
                        ContrastLevel.MEDIUM -> 5.5
                        ContrastLevel.HIGH -> 7.0
                    }
                    val semantic = resolveSonaSemanticColors(
                        isDark = isDark,
                        contrastLevel = contrast,
                        pureBlackDarkEnabled = pureBlack,
                    )
                    assertTrue(
                        "onRecording contrast failed for dark=$isDark, black=$pureBlack, contrast=$contrast",
                        SonaColorEngine.contrastRatio(semantic.onRecording, semantic.recording) >= minContrast,
                    )
                    assertTrue(
                        "onRecordingContainer contrast failed for dark=$isDark, black=$pureBlack, contrast=$contrast",
                        SonaColorEngine.contrastRatio(semantic.onRecordingContainer, semantic.recordingContainer) >= minContrast,
                    )
                }
            }
        }
    }

    @Test
    fun `custom seed color produces distinct scheme from default preset`() {
        val custom = AppearanceSettings(
            colorSource = ColorSource.CUSTOM,
            customSeedColorArgb = 0xFF1976D2.toInt(),
        )
        val scheme = SonaColorEngine.resolveStaticColorScheme(custom, isDark = false)
        assertNotEquals(SonaLightColorScheme.primary, scheme.primary)
        assertTrue(scheme.primary.blue > scheme.primary.red)
    }

    @Test
    fun `system dynamic fallback in static resolver preserves default brand schemes`() {
        val dynamicDefault = AppearanceSettings(colorSource = ColorSource.SYSTEM_DYNAMIC)
        assertEquals(
            SonaLightColorScheme,
            SonaColorEngine.resolveStaticColorScheme(dynamicDefault, isDark = false),
        )
        assertEquals(
            SonaDarkColorScheme,
            SonaColorEngine.resolveStaticColorScheme(dynamicDefault, isDark = true),
        )
    }

    @Test
    fun `pure black dark mode adjusts surfaceVariant and honors high contrast thresholds`() {
        val pureBlackHigh = AppearanceSettings(
            themeMode = ThemeMode.DARK,
            pureBlackDarkEnabled = true,
            palettePreset = ColorPalettePreset.OCEAN_SLATE,
            contrastLevel = ContrastLevel.HIGH,
        )
        val scheme = SonaColorEngine.resolveStaticColorScheme(pureBlackHigh, isDark = true)
        assertEquals(Color(0xFF1E1E1E), scheme.surfaceVariant)
        assertTrue(SonaColorEngine.contrastRatio(scheme.onSurface, scheme.surfaceContainerHighest) >= 7.0)
        assertTrue(SonaColorEngine.contrastRatio(scheme.onSurfaceVariant, scheme.surfaceContainerHighest) >= 7.0)
        assertTrue(SonaColorEngine.contrastRatio(scheme.onSurfaceVariant, scheme.surfaceVariant) >= 7.0)
        assertTrue(SonaColorEngine.contrastRatio(scheme.outline, scheme.surfaceContainer) >= 4.0)
        assertTrue(SonaColorEngine.contrastRatio(scheme.outlineVariant, scheme.surfaceContainer) >= 2.2)
    }

    @Test
    fun `applyContrastAdjustment enforces contrast when dark surfaceVariant is brighter than surfaceContainerHighest`() {
        val trickyDark = darkColorScheme(
            surfaceContainerHighest = Color(0xFF282828),
            surfaceVariant = Color(0xFF49454F),
            onSurfaceVariant = Color(0xFFCAC4D0),
        )
        val adjusted = SonaColorEngine.applyContrastAdjustment(
            scheme = trickyDark,
            isDark = true,
            contrastLevel = ContrastLevel.HIGH,
        )
        assertTrue(SonaColorEngine.contrastRatio(adjusted.onSurfaceVariant, adjusted.surfaceVariant) >= 7.0)
        assertTrue(SonaColorEngine.contrastRatio(adjusted.onSurfaceVariant, adjusted.surfaceContainerHighest) >= 7.0)
    }
}
