package com.sona.android.app.feature.settings

import android.os.Build
import androidx.annotation.StringRes
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.selection.selectableGroup
import androidx.compose.foundation.selection.toggleable
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.BrightnessAuto
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.Colorize
import androidx.compose.material.icons.rounded.DarkMode
import androidx.compose.material.icons.rounded.LightMode
import androidx.compose.material.icons.rounded.Wallpaper
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.ui.component.InsetGroupedCard
import com.sona.android.app.ui.component.SonaOptionItem
import com.sona.android.app.ui.component.SonaOptionSettingRow
import com.sona.android.app.ui.component.SonaSingleChoiceDialog
import com.sona.android.app.ui.component.SonaSwitch
import com.sona.android.app.ui.component.TwoLineItemDivider
import com.sona.android.app.ui.component.springOverscroll
import com.sona.android.app.ui.theme.SonaColorEngine
import com.sona.android.app.ui.theme.resolveDarkTheme
import com.sona.android.app.ui.theme.resolveSonaColorScheme
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode

private sealed interface ColorSwatchEntry {
    val stableKey: String

    data object SystemDynamic : ColorSwatchEntry {
        override val stableKey: String = "system_dynamic"
    }

    data class Preset(val preset: ColorPalettePreset) : ColorSwatchEntry {
        override val stableKey: String = "preset_${preset.wireValue}"
    }

    data object Custom : ColorSwatchEntry {
        override val stableKey: String = "custom"
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AppearanceSettingsPane(
    state: AppearanceSettingsUiState,
    appLanguage: AppLanguage,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onThemeModeChanged: (ThemeMode) -> Unit,
    onPureBlackDarkChanged: (Boolean) -> Unit,
    onSelectSystemDynamicColor: () -> Unit,
    onSelectPalettePreset: (ColorPalettePreset) -> Unit,
    onCustomSeedColorChanged: (Int) -> Unit,
    onPaletteStyleChanged: (PaletteStyle) -> Unit,
    onContrastLevelChanged: (ContrastLevel) -> Unit,
    modifier: Modifier = Modifier,
    extraBottomPadding: Dp = 0.dp,
) {
    val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    val supportsDynamicColor = Build.VERSION.SDK_INT >= Build.VERSION_CODES.S
    val effectiveColorSource = if (state.colorSource == ColorSource.SYSTEM_DYNAMIC && !supportsDynamicColor) {
        ColorSource.PRESET
    } else {
        state.colorSource
    }
    val controlsEnabled = state.isLoaded
    var customColorDialogVisible by rememberSaveable { mutableStateOf(false) }
    var paletteStyleDialogVisible by rememberSaveable { mutableStateOf(false) }
    var contrastDialogVisible by rememberSaveable { mutableStateOf(false) }
    val resolvedDark = resolveDarkTheme(state.themeMode, isSystemInDarkTheme())

    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxWidth()
                .springOverscroll()
                .verticalScroll(rememberScrollState())
                .padding(
                    start = 24.dp,
                    end = 24.dp,
                    top = 20.dp,
                    bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
                )
                .align(Alignment.TopCenter),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            if (state.hasError) {
                Surface(
                    modifier = Modifier.fillMaxWidth(),
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.errorContainer,
                ) {
                    Text(
                        text = stringResource(R.string.appearance_settings_error),
                        color = MaterialTheme.colorScheme.onErrorContainer,
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                    )
                }
            }

            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Text(
                        text = stringResource(R.string.appearance_theme_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 18.dp),
                    )
                    SingleChoiceSegmentedButtonRow(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(horizontal = 18.dp),
                    ) {
                        val modes = ThemeMode.entries
                        modes.forEachIndexed { index, mode ->
                            SegmentedButton(
                                selected = state.themeMode == mode,
                                onClick = { onThemeModeChanged(mode) },
                                enabled = controlsEnabled,
                                shape = SegmentedButtonDefaults.itemShape(
                                    index = index,
                                    count = modes.size,
                                ),
                                icon = {
                                    SegmentedButtonDefaults.Icon(active = state.themeMode == mode) {
                                        Icon(
                                            imageVector = mode.icon(),
                                            contentDescription = null,
                                            modifier = Modifier.size(18.dp),
                                        )
                                    }
                                },
                                label = {
                                    Text(
                                        text = stringResource(mode.labelRes()),
                                        maxLines = 1,
                                        overflow = TextOverflow.Ellipsis,
                                    )
                                },
                            )
                        }
                    }

                    TwoLineItemDivider(startIndent = 18.dp)

                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .toggleable(
                                value = state.pureBlackDarkEnabled,
                                enabled = controlsEnabled,
                                role = Role.Switch,
                                onValueChange = onPureBlackDarkChanged,
                            )
                            .padding(horizontal = 18.dp, vertical = 6.dp),
                        verticalAlignment = Alignment.CenterVertically,
                        horizontalArrangement = Arrangement.SpaceBetween,
                    ) {
                        Column(modifier = Modifier.weight(1f)) {
                            Text(
                                text = stringResource(R.string.appearance_pure_black_title),
                                style = MaterialTheme.typography.bodyLarge,
                                color = MaterialTheme.colorScheme.onSurface,
                            )
                            Text(
                                text = stringResource(R.string.appearance_pure_black_description),
                                style = MaterialTheme.typography.bodySmall,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                        Spacer(Modifier.width(16.dp))
                        SonaSwitch(
                            checked = state.pureBlackDarkEnabled,
                            onCheckedChange = null,
                            enabled = controlsEnabled,
                        )
                    }
                }
            }

            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                Column(
                    modifier = Modifier.padding(vertical = 14.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    Text(
                        text = stringResource(R.string.appearance_color_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(horizontal = 18.dp),
                    )

                    ColorPaletteSwatchCarousel(
                        state = state,
                        effectiveColorSource = effectiveColorSource,
                        supportsDynamicColor = supportsDynamicColor,
                        resolvedDark = resolvedDark,
                        enabled = controlsEnabled,
                        onSelectSystemDynamicColor = onSelectSystemDynamicColor,
                        onSelectPalettePreset = onSelectPalettePreset,
                        onOpenCustomColorPicker = { customColorDialogVisible = true },
                    )

                    TwoLineItemDivider(startIndent = 18.dp)

                    val isSystemDynamic = effectiveColorSource == ColorSource.SYSTEM_DYNAMIC
                    SonaOptionSettingRow(
                        headline = stringResource(R.string.appearance_palette_style_label),
                        selectedValueText = if (isSystemDynamic) {
                            stringResource(R.string.appearance_palette_style_system_hint)
                        } else {
                            stringResource(state.paletteStyle.labelRes())
                        },
                        enabled = controlsEnabled && !isSystemDynamic,
                        onClick = { paletteStyleDialogVisible = true },
                    )

                    TwoLineItemDivider(startIndent = 18.dp)

                    SonaOptionSettingRow(
                        headline = stringResource(R.string.appearance_contrast_label),
                        selectedValueText = stringResource(state.contrastLevel.labelRes()),
                        enabled = controlsEnabled,
                        onClick = { contrastDialogVisible = true },
                    )
                }
            }

            InsetGroupedCard(modifier = Modifier.fillMaxWidth()) {
                LanguageSelector(
                    selectedLanguage = appLanguage,
                    onLanguageChanged = onAppLanguageChanged,
                )
            }
        }
    }

    if (customColorDialogVisible) {
        CustomColorPickerDialog(
            initialSeedArgb = state.customSeedColorArgb,
            paletteStyle = state.paletteStyle,
            contrastLevel = state.contrastLevel,
            pureBlackDarkEnabled = state.pureBlackDarkEnabled,
            isDark = resolvedDark,
            onApply = onCustomSeedColorChanged,
            onDismiss = { customColorDialogVisible = false },
        )
    }

    if (paletteStyleDialogVisible) {
        val styleOptions = PaletteStyle.entries.map { style ->
            SonaOptionItem(
                value = style,
                label = stringResource(style.labelRes()),
                supportingText = stringResource(style.descriptionRes()),
            )
        }
        SonaSingleChoiceDialog(
            title = stringResource(R.string.appearance_palette_style_label),
            options = styleOptions,
            selectedValue = state.paletteStyle,
            onSelect = onPaletteStyleChanged,
            onDismiss = { paletteStyleDialogVisible = false },
        )
    }

    if (contrastDialogVisible) {
        val contrastOptions = ContrastLevel.entries.map { level ->
            SonaOptionItem(
                value = level,
                label = stringResource(level.labelRes()),
                supportingText = stringResource(level.descriptionRes()),
            )
        }
        SonaSingleChoiceDialog(
            title = stringResource(R.string.appearance_contrast_label),
            options = contrastOptions,
            selectedValue = state.contrastLevel,
            onSelect = onContrastLevelChanged,
            onDismiss = { contrastDialogVisible = false },
        )
    }
}

@Composable
private fun ColorPaletteSwatchCarousel(
    state: AppearanceSettingsUiState,
    effectiveColorSource: ColorSource,
    supportsDynamicColor: Boolean,
    resolvedDark: Boolean,
    enabled: Boolean,
    onSelectSystemDynamicColor: () -> Unit,
    onSelectPalettePreset: (ColorPalettePreset) -> Unit,
    onOpenCustomColorPicker: () -> Unit,
) {
    val context = LocalContext.current
    val entries = remember(supportsDynamicColor) {
        buildList {
            if (supportsDynamicColor) {
                add(ColorSwatchEntry.SystemDynamic)
            }
            ColorPalettePreset.entries.forEach { preset ->
                add(ColorSwatchEntry.Preset(preset))
            }
            add(ColorSwatchEntry.Custom)
        }
    }

    LazyRow(
        modifier = Modifier.selectableGroup(),
        contentPadding = PaddingValues(horizontal = 18.dp, vertical = 4.dp),
        horizontalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(entries, key = { it.stableKey }) { entry ->
            when (entry) {
                ColorSwatchEntry.SystemDynamic -> {
                    val dynamicScheme = remember(
                        resolvedDark,
                        state.contrastLevel,
                        state.pureBlackDarkEnabled,
                        context,
                    ) {
                        if (supportsDynamicColor) {
                            resolveSonaColorScheme(
                                settings = AppearanceSettings(
                                    themeMode = state.themeMode,
                                    pureBlackDarkEnabled = state.pureBlackDarkEnabled,
                                    colorSource = ColorSource.SYSTEM_DYNAMIC,
                                    contrastLevel = state.contrastLevel,
                                ),
                                isDark = resolvedDark,
                                context = context,
                            )
                        } else {
                            null
                        }
                    }
                    if (dynamicScheme != null) {
                        PaletteSwatchItem(
                            label = stringResource(R.string.color_source_system_dynamic),
                            scheme = dynamicScheme,
                            selected = effectiveColorSource == ColorSource.SYSTEM_DYNAMIC,
                            enabled = enabled,
                            badgeIcon = Icons.Rounded.Wallpaper,
                            onClick = onSelectSystemDynamicColor,
                        )
                    }
                }
                is ColorSwatchEntry.Preset -> {
                    val previewScheme = remember(
                        entry.preset,
                        state.paletteStyle,
                        state.contrastLevel,
                        state.pureBlackDarkEnabled,
                        resolvedDark,
                    ) {
                        SonaColorEngine.resolveStaticColorScheme(
                            settings = AppearanceSettings(
                                themeMode = state.themeMode,
                                pureBlackDarkEnabled = state.pureBlackDarkEnabled,
                                colorSource = ColorSource.PRESET,
                                palettePreset = entry.preset,
                                paletteStyle = state.paletteStyle,
                                contrastLevel = state.contrastLevel,
                            ),
                            isDark = resolvedDark,
                        )
                    }
                    PaletteSwatchItem(
                        label = stringResource(entry.preset.labelRes()),
                        scheme = previewScheme,
                        selected = effectiveColorSource == ColorSource.PRESET &&
                            state.palettePreset == entry.preset,
                        enabled = enabled,
                        badgeIcon = null,
                        onClick = { onSelectPalettePreset(entry.preset) },
                    )
                }
                ColorSwatchEntry.Custom -> {
                    val customScheme = remember(
                        state.customSeedColorArgb,
                        state.paletteStyle,
                        state.contrastLevel,
                        state.pureBlackDarkEnabled,
                        resolvedDark,
                    ) {
                        SonaColorEngine.resolveStaticColorScheme(
                            settings = AppearanceSettings(
                                themeMode = state.themeMode,
                                pureBlackDarkEnabled = state.pureBlackDarkEnabled,
                                colorSource = ColorSource.CUSTOM,
                                customSeedColorArgb = state.customSeedColorArgb,
                                paletteStyle = state.paletteStyle,
                                contrastLevel = state.contrastLevel,
                            ),
                            isDark = resolvedDark,
                        )
                    }
                    PaletteSwatchItem(
                        label = stringResource(R.string.color_source_custom),
                        scheme = customScheme,
                        selected = effectiveColorSource == ColorSource.CUSTOM,
                        enabled = enabled,
                        badgeIcon = Icons.Rounded.Colorize,
                        onClick = onOpenCustomColorPicker,
                    )
                }
            }
        }
    }
}

@Composable
private fun PaletteSwatchItem(
    label: String,
    scheme: ColorScheme,
    selected: Boolean,
    enabled: Boolean,
    badgeIcon: ImageVector?,
    onClick: () -> Unit,
) {
    Column(
        modifier = Modifier
            .widthIn(min = 72.dp, max = 88.dp)
            .clip(RoundedCornerShape(16.dp))
            .selectable(
                selected = selected,
                enabled = enabled,
                role = Role.RadioButton,
                onClick = onClick,
            )
            .padding(vertical = 6.dp, horizontal = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Surface(
            modifier = Modifier.size(56.dp),
            shape = RoundedCornerShape(18.dp),
            color = scheme.surfaceContainerHigh,
            border = if (selected) {
                BorderStroke(2.dp, MaterialTheme.colorScheme.primary)
            } else {
                BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
            },
        ) {
            Box(contentAlignment = Alignment.Center) {
                Canvas(
                    modifier = Modifier
                        .size(38.dp)
                        .clip(CircleShape),
                ) {
                    drawArc(
                        color = scheme.primary,
                        startAngle = 180f,
                        sweepAngle = 180f,
                        useCenter = true,
                    )
                    drawArc(
                        color = scheme.primaryContainer,
                        startAngle = 90f,
                        sweepAngle = 90f,
                        useCenter = true,
                    )
                    drawArc(
                        color = scheme.tertiary,
                        startAngle = 0f,
                        sweepAngle = 90f,
                        useCenter = true,
                    )
                }

                if (selected) {
                    Box(
                        modifier = Modifier
                            .size(22.dp)
                            .clip(CircleShape)
                            .background(MaterialTheme.colorScheme.primary)
                            .border(1.5.dp, MaterialTheme.colorScheme.surface, CircleShape),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Check,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.onPrimary,
                            modifier = Modifier.size(14.dp),
                        )
                    }
                } else if (badgeIcon != null) {
                    Box(
                        modifier = Modifier
                            .size(20.dp)
                            .clip(CircleShape)
                            .background(scheme.surface.copy(alpha = 0.88f)),
                        contentAlignment = Alignment.Center,
                    ) {
                        Icon(
                            imageVector = badgeIcon,
                            contentDescription = null,
                            tint = scheme.onSurface,
                            modifier = Modifier.size(12.dp),
                        )
                    }
                }
            }
        }

        Text(
            text = label,
            style = MaterialTheme.typography.labelSmall,
            fontWeight = if (selected) FontWeight.SemiBold else FontWeight.Normal,
            color = if (selected) {
                MaterialTheme.colorScheme.primary
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            },
            maxLines = 2,
            overflow = TextOverflow.Ellipsis,
            textAlign = TextAlign.Center,
        )
    }
}

@Composable
private fun LanguageSelector(
    selectedLanguage: AppLanguage,
    onLanguageChanged: (AppLanguage) -> Unit,
) {
    var dialogVisible by rememberSaveable { mutableStateOf(false) }

    SonaOptionSettingRow(
        headline = stringResource(R.string.language_label),
        selectedValueText = stringResource(selectedLanguage.labelRes),
        onClick = { dialogVisible = true },
    )

    if (dialogVisible) {
        val languageOptions = AppLanguage.entries.map { language ->
            SonaOptionItem(
                value = language,
                label = stringResource(language.labelRes),
            )
        }
        SonaSingleChoiceDialog(
            title = stringResource(R.string.language_label),
            options = languageOptions,
            selectedValue = selectedLanguage,
            onSelect = { language ->
                if (language != selectedLanguage) {
                    onLanguageChanged(language)
                }
            },
            onDismiss = { dialogVisible = false },
        )
    }
}

@StringRes
internal fun ThemeMode.labelRes(): Int = when (this) {
    ThemeMode.FOLLOW_SYSTEM -> R.string.theme_mode_system
    ThemeMode.LIGHT -> R.string.theme_mode_light
    ThemeMode.DARK -> R.string.theme_mode_dark
}

internal fun ThemeMode.icon(): ImageVector = when (this) {
    ThemeMode.FOLLOW_SYSTEM -> Icons.Rounded.BrightnessAuto
    ThemeMode.LIGHT -> Icons.Rounded.LightMode
    ThemeMode.DARK -> Icons.Rounded.DarkMode
}

@StringRes
internal fun ColorPalettePreset.labelRes(): Int = when (this) {
    ColorPalettePreset.TERRACOTTA -> R.string.color_preset_terracotta
    ColorPalettePreset.STONE_INK -> R.string.color_preset_stone_ink
    ColorPalettePreset.SAGE_FOREST -> R.string.color_preset_sage_forest
    ColorPalettePreset.OCEAN_SLATE -> R.string.color_preset_ocean_slate
    ColorPalettePreset.LAVENDER_IRIS -> R.string.color_preset_lavender_iris
    ColorPalettePreset.AMBER_OCHRE -> R.string.color_preset_amber_ochre
    ColorPalettePreset.ROSE_CLAY -> R.string.color_preset_rose_clay
}

@StringRes
internal fun PaletteStyle.labelRes(): Int = when (this) {
    PaletteStyle.TONAL_SPOT -> R.string.palette_style_tonal_spot
    PaletteStyle.VIBRANT -> R.string.palette_style_vibrant
    PaletteStyle.EXPRESSIVE -> R.string.palette_style_expressive
    PaletteStyle.NEUTRAL -> R.string.palette_style_neutral
    PaletteStyle.MONOCHROME -> R.string.palette_style_monochrome
}

@StringRes
private fun PaletteStyle.descriptionRes(): Int = when (this) {
    PaletteStyle.TONAL_SPOT -> R.string.palette_style_tonal_spot_desc
    PaletteStyle.VIBRANT -> R.string.palette_style_vibrant_desc
    PaletteStyle.EXPRESSIVE -> R.string.palette_style_expressive_desc
    PaletteStyle.NEUTRAL -> R.string.palette_style_neutral_desc
    PaletteStyle.MONOCHROME -> R.string.palette_style_monochrome_desc
}

@StringRes
internal fun ContrastLevel.labelRes(): Int = when (this) {
    ContrastLevel.STANDARD -> R.string.contrast_standard
    ContrastLevel.MEDIUM -> R.string.contrast_medium
    ContrastLevel.HIGH -> R.string.contrast_high
}

@StringRes
private fun ContrastLevel.descriptionRes(): Int = when (this) {
    ContrastLevel.STANDARD -> R.string.contrast_standard_desc
    ContrastLevel.MEDIUM -> R.string.contrast_medium_desc
    ContrastLevel.HIGH -> R.string.contrast_high_desc
}
