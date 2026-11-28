package com.sona.android.app.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.luminance
import androidx.compose.ui.graphics.toArgb
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import kotlin.math.PI
import kotlin.math.atan2
import kotlin.math.cbrt
import kotlin.math.cos
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt
import kotlin.math.sin
import kotlin.math.sqrt

internal data class OklchColor(
    val l: Double,
    val c: Double,
    val h: Double,
)

private data class TonalSpec(
    val hueDeg: Double,
    val chroma: Double,
)

private data class PaletteSpecs(
    val primary: TonalSpec,
    val secondary: TonalSpec,
    val tertiary: TonalSpec,
    val neutral: TonalSpec,
    val neutralVariant: TonalSpec,
    val error: TonalSpec,
)

internal object SonaColorEngine {
    fun resolveStaticColorScheme(
        settings: AppearanceSettings,
        isDark: Boolean,
    ): ColorScheme {
        val effectiveColorSource = if (settings.colorSource == ColorSource.SYSTEM_DYNAMIC) {
            ColorSource.PRESET
        } else {
            settings.colorSource
        }
        val isBrandDefault = effectiveColorSource == ColorSource.PRESET &&
            settings.palettePreset == ColorPalettePreset.TERRACOTTA &&
            settings.paletteStyle == PaletteStyle.TONAL_SPOT &&
            settings.contrastLevel == ContrastLevel.STANDARD

        val baseScheme = if (isBrandDefault) {
            if (isDark) SonaDarkColorScheme else SonaLightColorScheme
        } else {
            val seedArgb = when (effectiveColorSource) {
                ColorSource.CUSTOM -> settings.customSeedColorArgb
                ColorSource.PRESET, ColorSource.SYSTEM_DYNAMIC -> settings.palettePreset.seedColorArgb
            }
            generateColorScheme(
                seedArgb = seedArgb,
                style = settings.paletteStyle,
                contrastLevel = settings.contrastLevel,
                isDark = isDark,
            )
        }

        return if (isDark && settings.pureBlackDarkEnabled) {
            applyPureBlackSurfaces(baseScheme, settings.contrastLevel)
        } else {
            baseScheme
        }
    }

    fun generateColorScheme(
        seedArgb: Int,
        style: PaletteStyle,
        contrastLevel: ContrastLevel,
        isDark: Boolean,
    ): ColorScheme {
        val specs = buildPaletteSpecs(seedArgb = seedArgb, style = style)
        val c = contrastLevel.contrastValue
        val minTextContrast = contrastThreshold(contrastLevel)

        return if (!isDark) {
            val background = toneColor(specs.neutral, 98.0)
            val surface = toneColor(specs.neutral, 98.0)
            val surfaceContainerLowest = toneColor(specs.neutral, 100.0)
            val surfaceContainerLow = toneColor(specs.neutral, lerp(96.0, 94.0, c))
            val surfaceContainer = toneColor(specs.neutral, lerp(94.0, 91.0, c))
            val surfaceContainerHigh = toneColor(specs.neutral, lerp(92.0, 88.0, c))
            val surfaceContainerHighest = toneColor(specs.neutral, lerp(90.0, 84.0, c))

            val onSurface = ensureContrast(
                toneColor(specs.neutral, lerp(10.0, 4.0, c)),
                surfaceContainerHighest,
                minTextContrast,
                preferLighter = false,
            )
            val onSurfaceVariant = ensureContrast(
                toneColor(specs.neutralVariant, lerp(30.0, 14.0, c)),
                surfaceContainerHighest,
                minTextContrast,
                preferLighter = false,
            )

            val (primary, onPrimary) = ensureAccentAndLabel(
                accent = toneColor(specs.primary, lerp(40.0, 22.0, c)),
                label = toneColor(specs.primary, 100.0),
                surface = surface,
                minContrast = minTextContrast,
                isDark = false,
            )
            val primaryContainer = toneColor(specs.primary, lerp(90.0, 74.0, c))
            val onPrimaryContainer = ensureContrast(
                toneColor(specs.primary, lerp(10.0, 2.0, c)),
                primaryContainer,
                minTextContrast,
                preferLighter = false,
            )

            val (secondary, onSecondary) = ensureAccentAndLabel(
                accent = toneColor(specs.secondary, lerp(40.0, 22.0, c)),
                label = toneColor(specs.secondary, 100.0),
                surface = surface,
                minContrast = minTextContrast,
                isDark = false,
            )
            val secondaryContainer = toneColor(specs.secondary, lerp(90.0, 74.0, c))
            val onSecondaryContainer = ensureContrast(
                toneColor(specs.secondary, lerp(10.0, 2.0, c)),
                secondaryContainer,
                minTextContrast,
                preferLighter = false,
            )

            val (tertiary, onTertiary) = ensureAccentAndLabel(
                accent = toneColor(specs.tertiary, lerp(40.0, 22.0, c)),
                label = toneColor(specs.tertiary, 100.0),
                surface = surface,
                minContrast = minTextContrast,
                isDark = false,
            )
            val tertiaryContainer = toneColor(specs.tertiary, lerp(90.0, 74.0, c))
            val onTertiaryContainer = ensureContrast(
                toneColor(specs.tertiary, lerp(10.0, 2.0, c)),
                tertiaryContainer,
                minTextContrast,
                preferLighter = false,
            )

            val (error, onError) = ensureAccentAndLabel(
                accent = toneColor(specs.error, lerp(40.0, 22.0, c)),
                label = toneColor(specs.error, 100.0),
                surface = surface,
                minContrast = minTextContrast,
                isDark = false,
            )
            val errorContainer = toneColor(specs.error, lerp(90.0, 76.0, c))
            val onErrorContainer = ensureContrast(
                toneColor(specs.error, lerp(10.0, 2.0, c)),
                errorContainer,
                minTextContrast,
                preferLighter = false,
            )

            val inverseSurface = toneColor(specs.neutral, 20.0)
            val inverseOnSurface = ensureContrast(
                toneColor(specs.neutral, 95.0),
                inverseSurface,
                minTextContrast,
                preferLighter = true,
            )

            val primaryFixed = toneColor(specs.primary, 90.0)
            val onPrimaryFixed = ensureContrast(toneColor(specs.primary, 10.0), primaryFixed, minTextContrast, preferLighter = false)
            val primaryFixedDim = toneColor(specs.primary, 80.0)
            val onPrimaryFixedVariant = ensureContrast(toneColor(specs.primary, 30.0), primaryFixedDim, minTextContrast, preferLighter = false)

            val secondaryFixed = toneColor(specs.secondary, 90.0)
            val onSecondaryFixed = ensureContrast(toneColor(specs.secondary, 10.0), secondaryFixed, minTextContrast, preferLighter = false)
            val secondaryFixedDim = toneColor(specs.secondary, 80.0)
            val onSecondaryFixedVariant = ensureContrast(toneColor(specs.secondary, 30.0), secondaryFixedDim, minTextContrast, preferLighter = false)

            val tertiaryFixed = toneColor(specs.tertiary, 90.0)
            val onTertiaryFixed = ensureContrast(toneColor(specs.tertiary, 10.0), tertiaryFixed, minTextContrast, preferLighter = false)
            val tertiaryFixedDim = toneColor(specs.tertiary, 80.0)
            val onTertiaryFixedVariant = ensureContrast(toneColor(specs.tertiary, 30.0), tertiaryFixedDim, minTextContrast, preferLighter = false)

            lightColorScheme(
                primary = primary,
                onPrimary = onPrimary,
                primaryContainer = primaryContainer,
                onPrimaryContainer = onPrimaryContainer,
                secondary = secondary,
                onSecondary = onSecondary,
                secondaryContainer = secondaryContainer,
                onSecondaryContainer = onSecondaryContainer,
                tertiary = tertiary,
                onTertiary = onTertiary,
                tertiaryContainer = tertiaryContainer,
                onTertiaryContainer = onTertiaryContainer,
                background = background,
                onBackground = onSurface,
                surface = surface,
                onSurface = onSurface,
                surfaceVariant = toneColor(specs.neutralVariant, lerp(90.0, 82.0, c)),
                onSurfaceVariant = onSurfaceVariant,
                surfaceDim = toneColor(specs.neutral, lerp(87.0, 82.0, c)),
                surfaceBright = surface,
                surfaceContainerLowest = surfaceContainerLowest,
                surfaceContainerLow = surfaceContainerLow,
                surfaceContainer = surfaceContainer,
                surfaceContainerHigh = surfaceContainerHigh,
                surfaceContainerHighest = surfaceContainerHighest,
                outline = toneColor(specs.neutralVariant, lerp(50.0, 26.0, c)),
                outlineVariant = toneColor(specs.neutralVariant, lerp(80.0, 48.0, c)),
                error = error,
                onError = onError,
                errorContainer = errorContainer,
                onErrorContainer = onErrorContainer,
                inverseSurface = inverseSurface,
                inverseOnSurface = inverseOnSurface,
                inversePrimary = toneColor(specs.primary, 80.0),
                scrim = Color.Black,
                primaryFixed = primaryFixed,
                onPrimaryFixed = onPrimaryFixed,
                primaryFixedDim = primaryFixedDim,
                onPrimaryFixedVariant = onPrimaryFixedVariant,
                secondaryFixed = secondaryFixed,
                onSecondaryFixed = onSecondaryFixed,
                secondaryFixedDim = secondaryFixedDim,
                onSecondaryFixedVariant = onSecondaryFixedVariant,
                tertiaryFixed = tertiaryFixed,
                onTertiaryFixed = onTertiaryFixed,
                tertiaryFixedDim = tertiaryFixedDim,
                onTertiaryFixedVariant = onTertiaryFixedVariant,
            )
        } else {
            val background = toneColor(specs.neutral, 6.0)
            val surface = toneColor(specs.neutral, 6.0)
            val surfaceContainerLowest = toneColor(specs.neutral, 4.0)
            val surfaceContainerLow = toneColor(specs.neutral, 10.0)
            val surfaceContainer = toneColor(specs.neutral, 12.0)
            val surfaceContainerHigh = toneColor(specs.neutral, 17.0)
            val surfaceContainerHighest = toneColor(specs.neutral, 22.0)

            val onSurface = ensureContrast(
                toneColor(specs.neutral, lerp(90.0, 96.0, c)),
                surfaceContainerHighest,
                minTextContrast,
                preferLighter = true,
            )
            val onSurfaceVariant = ensureContrast(
                toneColor(specs.neutralVariant, lerp(80.0, 92.0, c)),
                surfaceContainerHighest,
                minTextContrast,
                preferLighter = true,
            )

            val (primary, onPrimary) = ensureAccentAndLabel(
                accent = toneColor(specs.primary, lerp(80.0, 91.0, c)),
                label = toneColor(specs.primary, lerp(20.0, 8.0, c)),
                surface = surfaceContainerHighest,
                minContrast = minTextContrast,
                isDark = true,
            )
            val primaryContainer = toneColor(specs.primary, lerp(30.0, 35.0, c))
            val onPrimaryContainer = ensureContrast(
                toneColor(specs.primary, lerp(90.0, 98.0, c)),
                primaryContainer,
                minTextContrast,
                preferLighter = true,
            )

            val (secondary, onSecondary) = ensureAccentAndLabel(
                accent = toneColor(specs.secondary, lerp(80.0, 91.0, c)),
                label = toneColor(specs.secondary, lerp(20.0, 8.0, c)),
                surface = surfaceContainerHighest,
                minContrast = minTextContrast,
                isDark = true,
            )
            val secondaryContainer = toneColor(specs.secondary, lerp(30.0, 35.0, c))
            val onSecondaryContainer = ensureContrast(
                toneColor(specs.secondary, lerp(90.0, 98.0, c)),
                secondaryContainer,
                minTextContrast,
                preferLighter = true,
            )

            val (tertiary, onTertiary) = ensureAccentAndLabel(
                accent = toneColor(specs.tertiary, lerp(80.0, 91.0, c)),
                label = toneColor(specs.tertiary, lerp(20.0, 8.0, c)),
                surface = surfaceContainerHighest,
                minContrast = minTextContrast,
                isDark = true,
            )
            val tertiaryContainer = toneColor(specs.tertiary, lerp(30.0, 35.0, c))
            val onTertiaryContainer = ensureContrast(
                toneColor(specs.tertiary, lerp(90.0, 98.0, c)),
                tertiaryContainer,
                minTextContrast,
                preferLighter = true,
            )

            val (error, onError) = ensureAccentAndLabel(
                accent = toneColor(specs.error, lerp(80.0, 90.0, c)),
                label = toneColor(specs.error, lerp(20.0, 8.0, c)),
                surface = surfaceContainerHighest,
                minContrast = minTextContrast,
                isDark = true,
            )
            val errorContainer = toneColor(specs.error, lerp(30.0, 35.0, c))
            val onErrorContainer = ensureContrast(
                toneColor(specs.error, lerp(90.0, 98.0, c)),
                errorContainer,
                minTextContrast,
                preferLighter = true,
            )

            val inverseSurface = toneColor(specs.neutral, 90.0)
            val inverseOnSurface = ensureContrast(
                toneColor(specs.neutral, 20.0),
                inverseSurface,
                minTextContrast,
                preferLighter = false,
            )

            val primaryFixed = toneColor(specs.primary, 90.0)
            val onPrimaryFixed = ensureContrast(toneColor(specs.primary, 10.0), primaryFixed, minTextContrast, preferLighter = false)
            val primaryFixedDim = toneColor(specs.primary, 80.0)
            val onPrimaryFixedVariant = ensureContrast(toneColor(specs.primary, 30.0), primaryFixedDim, minTextContrast, preferLighter = false)

            val secondaryFixed = toneColor(specs.secondary, 90.0)
            val onSecondaryFixed = ensureContrast(toneColor(specs.secondary, 10.0), secondaryFixed, minTextContrast, preferLighter = false)
            val secondaryFixedDim = toneColor(specs.secondary, 80.0)
            val onSecondaryFixedVariant = ensureContrast(toneColor(specs.secondary, 30.0), secondaryFixedDim, minTextContrast, preferLighter = false)

            val tertiaryFixed = toneColor(specs.tertiary, 90.0)
            val onTertiaryFixed = ensureContrast(toneColor(specs.tertiary, 10.0), tertiaryFixed, minTextContrast, preferLighter = false)
            val tertiaryFixedDim = toneColor(specs.tertiary, 80.0)
            val onTertiaryFixedVariant = ensureContrast(toneColor(specs.tertiary, 30.0), tertiaryFixedDim, minTextContrast, preferLighter = false)

            darkColorScheme(
                primary = primary,
                onPrimary = onPrimary,
                primaryContainer = primaryContainer,
                onPrimaryContainer = onPrimaryContainer,
                secondary = secondary,
                onSecondary = onSecondary,
                secondaryContainer = secondaryContainer,
                onSecondaryContainer = onSecondaryContainer,
                tertiary = tertiary,
                onTertiary = onTertiary,
                tertiaryContainer = tertiaryContainer,
                onTertiaryContainer = onTertiaryContainer,
                background = background,
                onBackground = onSurface,
                surface = surface,
                onSurface = onSurface,
                surfaceVariant = toneColor(specs.neutralVariant, lerp(24.0, 30.0, c)),
                onSurfaceVariant = onSurfaceVariant,
                surfaceDim = surface,
                surfaceBright = toneColor(specs.neutral, 24.0),
                surfaceContainerLowest = surfaceContainerLowest,
                surfaceContainerLow = surfaceContainerLow,
                surfaceContainer = surfaceContainer,
                surfaceContainerHigh = surfaceContainerHigh,
                surfaceContainerHighest = surfaceContainerHighest,
                outline = toneColor(specs.neutralVariant, lerp(60.0, 82.0, c)),
                outlineVariant = toneColor(specs.neutralVariant, lerp(30.0, 56.0, c)),
                error = error,
                onError = onError,
                errorContainer = errorContainer,
                onErrorContainer = onErrorContainer,
                inverseSurface = inverseSurface,
                inverseOnSurface = inverseOnSurface,
                inversePrimary = toneColor(specs.primary, 40.0),
                scrim = Color.Black,
                primaryFixed = primaryFixed,
                onPrimaryFixed = onPrimaryFixed,
                primaryFixedDim = primaryFixedDim,
                onPrimaryFixedVariant = onPrimaryFixedVariant,
                secondaryFixed = secondaryFixed,
                onSecondaryFixed = onSecondaryFixed,
                secondaryFixedDim = secondaryFixedDim,
                onSecondaryFixedVariant = onSecondaryFixedVariant,
                tertiaryFixed = tertiaryFixed,
                onTertiaryFixed = onTertiaryFixed,
                tertiaryFixedDim = tertiaryFixedDim,
                onTertiaryFixedVariant = onTertiaryFixedVariant,
            )
        }
    }

    fun applyContrastAdjustment(
        scheme: ColorScheme,
        isDark: Boolean,
        contrastLevel: ContrastLevel,
    ): ColorScheme {
        if (contrastLevel == ContrastLevel.STANDARD) return scheme
        val minTextContrast = contrastThreshold(contrastLevel)
        val preferLighter = isDark
        val worstCaseSurface = scheme.surfaceContainerHighest

        val (primary, onPrimary) = ensureAccentAndLabel(
            accent = scheme.primary,
            label = scheme.onPrimary,
            surface = if (isDark) worstCaseSurface else scheme.surface,
            minContrast = minTextContrast,
            isDark = isDark,
        )
        val (secondary, onSecondary) = ensureAccentAndLabel(
            accent = scheme.secondary,
            label = scheme.onSecondary,
            surface = if (isDark) worstCaseSurface else scheme.surface,
            minContrast = minTextContrast,
            isDark = isDark,
        )
        val (tertiary, onTertiary) = ensureAccentAndLabel(
            accent = scheme.tertiary,
            label = scheme.onTertiary,
            surface = if (isDark) worstCaseSurface else scheme.surface,
            minContrast = minTextContrast,
            isDark = isDark,
        )
        val (error, onError) = ensureAccentAndLabel(
            accent = scheme.error,
            label = scheme.onError,
            surface = if (isDark) worstCaseSurface else scheme.surface,
            minContrast = minTextContrast,
            isDark = isDark,
        )

        val onSurface = ensureContrast(scheme.onSurface, worstCaseSurface, minTextContrast, preferLighter)
        var onSurfaceVariant = ensureContrast(scheme.onSurfaceVariant, worstCaseSurface, minTextContrast, preferLighter)
        if (scheme.surfaceVariant != Color.Unspecified) {
            onSurfaceVariant = ensureContrast(onSurfaceVariant, scheme.surfaceVariant, minTextContrast, preferLighter)
        }
        val onPrimaryContainer = ensureContrast(scheme.onPrimaryContainer, scheme.primaryContainer, minTextContrast, preferLighter)
        val onSecondaryContainer = ensureContrast(scheme.onSecondaryContainer, scheme.secondaryContainer, minTextContrast, preferLighter)
        val onTertiaryContainer = ensureContrast(scheme.onTertiaryContainer, scheme.tertiaryContainer, minTextContrast, preferLighter)
        val onErrorContainer = ensureContrast(scheme.onErrorContainer, scheme.errorContainer, minTextContrast, preferLighter)
        val inverseOnSurface = ensureContrast(scheme.inverseOnSurface, scheme.inverseSurface, minTextContrast, !preferLighter)
        val outline = ensureContrast(scheme.outline, scheme.surfaceContainer, if (contrastLevel == ContrastLevel.HIGH) 4.0 else 3.0, preferLighter)
        val outlineVariant = ensureContrast(scheme.outlineVariant, scheme.surfaceContainer, if (contrastLevel == ContrastLevel.HIGH) 2.2 else 1.6, preferLighter)

        return scheme.copy(
            primary = primary,
            onPrimary = onPrimary,
            secondary = secondary,
            onSecondary = onSecondary,
            tertiary = tertiary,
            onTertiary = onTertiary,
            error = error,
            onError = onError,
            onPrimaryContainer = onPrimaryContainer,
            onSecondaryContainer = onSecondaryContainer,
            onTertiaryContainer = onTertiaryContainer,
            onErrorContainer = onErrorContainer,
            onBackground = onSurface,
            onSurface = onSurface,
            onSurfaceVariant = onSurfaceVariant,
            inverseOnSurface = inverseOnSurface,
            outline = outline,
            outlineVariant = outlineVariant,
        )
    }

    fun applyPureBlackSurfaces(
        scheme: ColorScheme,
        contrastLevel: ContrastLevel = ContrastLevel.STANDARD,
    ): ColorScheme {
        val pureSurfaceContainer = Color(0xFF121212)
        val pureSurfaceVariant = Color(0xFF1E1E1E)
        val pureSurfaceContainerHighest = Color(0xFF232323)
        val minTextContrast = contrastThreshold(contrastLevel)
        val minOutlineContrast = if (contrastLevel == ContrastLevel.HIGH) 4.0 else 3.0
        val minOutlineVariantContrast = if (contrastLevel == ContrastLevel.HIGH) 2.2 else 1.6

        val boostedOutlineVariant = ensureContrast(
            scheme.outlineVariant,
            pureSurfaceContainer,
            minRatio = minOutlineVariantContrast,
            preferLighter = true,
        )
        val checkedOutline = ensureContrast(
            scheme.outline,
            pureSurfaceContainer,
            minRatio = minOutlineContrast,
            preferLighter = true,
        )
        val checkedOnSurface = ensureContrast(
            scheme.onSurface,
            pureSurfaceContainerHighest,
            minRatio = minTextContrast,
            preferLighter = true,
        )
        val checkedOnSurfaceVariant = ensureContrast(
            scheme.onSurfaceVariant,
            pureSurfaceContainerHighest,
            minRatio = minTextContrast,
            preferLighter = true,
        )
        return scheme.copy(
            background = Color.Black,
            surface = Color.Black,
            surfaceDim = Color.Black,
            surfaceBright = Color(0xFF1F1F1F),
            surfaceVariant = pureSurfaceVariant,
            surfaceContainerLowest = Color.Black,
            surfaceContainerLow = Color(0xFF0A0A0A),
            surfaceContainer = pureSurfaceContainer,
            surfaceContainerHigh = Color(0xFF1A1A1A),
            surfaceContainerHighest = pureSurfaceContainerHighest,
            onBackground = checkedOnSurface,
            onSurface = checkedOnSurface,
            onSurfaceVariant = checkedOnSurfaceVariant,
            outline = checkedOutline,
            outlineVariant = boostedOutlineVariant,
        )
    }

    internal fun argbToOklch(argb: Int): OklchColor {
        val opaque = argb or 0xFF000000.toInt()
        val r = srgbChannelToLinear(((opaque ushr 16) and 0xFF) / 255.0)
        val g = srgbChannelToLinear(((opaque ushr 8) and 0xFF) / 255.0)
        val b = srgbChannelToLinear((opaque and 0xFF) / 255.0)

        val l = cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
        val m = cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
        val s = cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)

        val okL = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s
        val okA = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s
        val okB = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s

        val chroma = sqrt(okA * okA + okB * okB)
        val hueRad = atan2(okB, okA)
        val hueDeg = ((hueRad * 180.0 / PI) % 360.0 + 360.0) % 360.0
        return OklchColor(l = okL, c = chroma, h = hueDeg)
    }

    internal fun oklchToColor(l: Double, c: Double, hueDeg: Double): Color {
        val hueRad = normalizeHue(hueDeg) * PI / 180.0
        return oklchToColor(l = l, c = c, cosH = cos(hueRad), sinH = sin(hueRad))
    }

    private fun oklchToColor(l: Double, c: Double, cosH: Double, sinH: Double): Color {
        if (l <= 0.0) return Color.Black
        if (l >= 1.0) return Color.White

        if (inSrgbGamut(l, c * cosH, c * sinH)) {
            return linearOklabToColor(l, c * cosH, c * sinH)
        }

        var low = 0.0
        var high = c.coerceAtLeast(0.0)
        repeat(16) {
            val mid = (low + high) * 0.5
            if (inSrgbGamut(l, mid * cosH, mid * sinH)) {
                low = mid
            } else {
                high = mid
            }
        }
        return linearOklabToColor(l, low * cosH, low * sinH)
    }

    internal fun contrastRatio(foreground: Color, background: Color): Double {
        val fgLum = foreground.luminance().toDouble()
        val bgLum = background.luminance().toDouble()
        val lighter = max(fgLum, bgLum)
        val darker = min(fgLum, bgLum)
        return (lighter + 0.05) / (darker + 0.05)
    }

    private fun contrastThreshold(contrastLevel: ContrastLevel): Double = when (contrastLevel) {
        ContrastLevel.STANDARD -> 4.5
        ContrastLevel.MEDIUM -> 5.5
        ContrastLevel.HIGH -> 7.0
    }

    private fun buildPaletteSpecs(seedArgb: Int, style: PaletteStyle): PaletteSpecs {
        val seed = argbToOklch(seedArgb)
        val isAchromatic = seed.c < 0.015
        if (isAchromatic || style == PaletteStyle.MONOCHROME) {
            return PaletteSpecs(
                primary = TonalSpec(0.0, 0.0),
                secondary = TonalSpec(0.0, 0.0),
                tertiary = TonalSpec(0.0, 0.0),
                neutral = TonalSpec(0.0, 0.0),
                neutralVariant = TonalSpec(0.0, 0.0),
                error = TonalSpec(27.0, 0.16),
            )
        }

        val hue = seed.h
        val baseChroma = seed.c

        return when (style) {
            PaletteStyle.TONAL_SPOT -> PaletteSpecs(
                primary = TonalSpec(hue, baseChroma.coerceIn(0.08, 0.145)),
                secondary = TonalSpec(hue, 0.048),
                tertiary = TonalSpec(normalizeHue(hue + 60.0), 0.078),
                neutral = TonalSpec(hue, 0.012),
                neutralVariant = TonalSpec(hue, 0.024),
                error = TonalSpec(27.0, 0.19),
            )
            PaletteStyle.VIBRANT -> PaletteSpecs(
                primary = TonalSpec(hue, baseChroma.coerceIn(0.14, 0.24)),
                secondary = TonalSpec(normalizeHue(hue + 15.0), 0.088),
                tertiary = TonalSpec(normalizeHue(hue + 32.0), 0.12),
                neutral = TonalSpec(hue, 0.018),
                neutralVariant = TonalSpec(hue, 0.032),
                error = TonalSpec(27.0, 0.21),
            )
            PaletteStyle.EXPRESSIVE -> PaletteSpecs(
                primary = TonalSpec(hue, baseChroma.coerceIn(0.13, 0.21)),
                secondary = TonalSpec(normalizeHue(hue + 40.0), 0.095),
                tertiary = TonalSpec(normalizeHue(hue + 150.0), 0.125),
                neutral = TonalSpec(normalizeHue(hue + 15.0), 0.020),
                neutralVariant = TonalSpec(normalizeHue(hue + 15.0), 0.036),
                error = TonalSpec(27.0, 0.20),
            )
            PaletteStyle.NEUTRAL -> PaletteSpecs(
                primary = TonalSpec(hue, 0.048),
                secondary = TonalSpec(hue, 0.028),
                tertiary = TonalSpec(normalizeHue(hue + 30.0), 0.055),
                neutral = TonalSpec(hue, 0.008),
                neutralVariant = TonalSpec(hue, 0.014),
                error = TonalSpec(27.0, 0.16),
            )
            PaletteStyle.MONOCHROME -> error("Handled above")
        }
    }

    private fun toneColor(spec: TonalSpec, tone: Double): Color {
        val clampedTone = tone.coerceIn(0.0, 100.0)
        if (clampedTone <= 0.0) return Color.Black
        if (clampedTone >= 100.0) return Color.White

        val hueRad = normalizeHue(spec.hueDeg) * PI / 180.0
        val cosH = cos(hueRad)
        val sinH = sin(hueRad)
        val targetY = toneToRelativeLuminance(clampedTone)
        var low = 0.0
        var high = 1.0
        repeat(18) {
            val mid = (low + high) * 0.5
            val candidate = oklchToColor(mid, spec.chroma, cosH, sinH)
            val lum = candidate.luminance().toDouble()
            if (lum < targetY) {
                low = mid
            } else {
                high = mid
            }
        }
        return oklchToColor((low + high) * 0.5, spec.chroma, cosH, sinH)
    }

    private fun ensureAccentAndLabel(
        accent: Color,
        label: Color,
        surface: Color,
        minContrast: Double,
        isDark: Boolean,
    ): Pair<Color, Color> {
        var adjustedAccent = ensureContrast(accent, surface, minContrast, preferLighter = isDark)
        var adjustedLabel = ensureContrast(label, adjustedAccent, minContrast, preferLighter = !isDark)
        if (contrastRatio(adjustedLabel, adjustedAccent) < minContrast) {
            adjustedAccent = ensureContrast(adjustedAccent, adjustedLabel, minContrast, preferLighter = isDark)
            if (contrastRatio(adjustedAccent, surface) < minContrast) {
                adjustedLabel = ensureContrast(label, adjustedAccent, minContrast, preferLighter = isDark)
            }
        }
        return adjustedAccent to adjustedLabel
    }

    private fun ensureContrast(
        foreground: Color,
        background: Color,
        minRatio: Double,
        preferLighter: Boolean,
    ): Color {
        if (contrastRatio(foreground, background) >= minRatio) return foreground

        val fgOklch = argbToOklch(foreground.toArgb())
        val startL = fgOklch.l
        val targetL = if (preferLighter) 1.0 else 0.0

        val extreme = oklchToColor(targetL, fgOklch.c, fgOklch.h)
        if (contrastRatio(extreme, background) < minRatio) {
            val achromatic = if (preferLighter) Color.White else Color.Black
            val fallbackOpposite = if (preferLighter) Color.Black else Color.White
            return if (contrastRatio(achromatic, background) >= contrastRatio(fallbackOpposite, background)) {
                achromatic
            } else {
                fallbackOpposite
            }
        }

        val hueRad = normalizeHue(fgOklch.h) * PI / 180.0
        val cosH = cos(hueRad)
        val sinH = sin(hueRad)
        var low = 0.0
        var high = 1.0
        repeat(16) {
            val mid = (low + high) * 0.5
            val candidateL = lerp(startL, targetL, mid)
            val candidate = oklchToColor(candidateL, fgOklch.c, cosH, sinH)
            if (contrastRatio(candidate, background) >= minRatio) {
                high = mid
            } else {
                low = mid
            }
        }
        return oklchToColor(lerp(startL, targetL, high), fgOklch.c, cosH, sinH)
    }

    private fun toneToRelativeLuminance(tone: Double): Double =
        if (tone <= 8.0) {
            tone / 903.2962962962963
        } else {
            ((tone + 16.0) / 116.0).pow(3.0)
        }

    private fun inSrgbGamut(l: Double, a: Double, b: Double): Boolean {
        val (lr, lg, lb) = oklabToLinearRgb(l, a, b)
        val eps = 1e-4
        return lr in -eps..(1.0 + eps) &&
            lg in -eps..(1.0 + eps) &&
            lb in -eps..(1.0 + eps)
    }

    private fun linearOklabToColor(l: Double, a: Double, b: Double): Color {
        val (lr, lg, lb) = oklabToLinearRgb(l, a, b)
        val r = (linearToSrgbChannel(lr.coerceIn(0.0, 1.0)) * 255.0).roundToInt().coerceIn(0, 255)
        val g = (linearToSrgbChannel(lg.coerceIn(0.0, 1.0)) * 255.0).roundToInt().coerceIn(0, 255)
        val bl = (linearToSrgbChannel(lb.coerceIn(0.0, 1.0)) * 255.0).roundToInt().coerceIn(0, 255)
        return Color(red = r, green = g, blue = bl)
    }

    private fun oklabToLinearRgb(l: Double, a: Double, b: Double): Triple<Double, Double, Double> {
        val l_ = l + 0.3963377774 * a + 0.2158037573 * b
        val m_ = l - 0.1055613458 * a - 0.0638541728 * b
        val s_ = l - 0.0894841775 * a - 1.2914855480 * b

        val ll = l_ * l_ * l_
        val mm = m_ * m_ * m_
        val ss = s_ * s_ * s_

        val r = +4.0767416621 * ll - 3.3077115913 * mm + 0.2309699292 * ss
        val g = -1.2684380046 * ll + 2.6097574011 * mm - 0.3413193965 * ss
        val bl = -0.0041960863 * ll - 0.7034186147 * mm + 1.7076147010 * ss
        return Triple(r, g, bl)
    }

    private fun srgbChannelToLinear(c: Double): Double =
        if (c <= 0.04045) c / 12.92 else ((c + 0.055) / 1.055).pow(2.4)

    private fun linearToSrgbChannel(c: Double): Double =
        if (c <= 0.0031308) 12.92 * c else 1.055 * c.pow(1.0 / 2.4) - 0.055

    private fun normalizeHue(hue: Double): Double = ((hue % 360.0) + 360.0) % 360.0

    private fun lerp(start: Double, end: Double, fraction: Double): Double =
        start + (end - start) * fraction
}
