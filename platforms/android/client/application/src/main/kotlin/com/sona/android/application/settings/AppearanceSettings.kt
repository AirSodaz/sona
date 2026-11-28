package com.sona.android.application.settings

import kotlinx.coroutines.flow.Flow

enum class ThemeMode(val wireValue: String) {
    FOLLOW_SYSTEM("system"),
    LIGHT("light"),
    DARK("dark"),
    ;

    companion object {
        fun fromWireValue(value: String?): ThemeMode =
            entries.firstOrNull { it.wireValue == value } ?: FOLLOW_SYSTEM
    }
}

enum class ColorSource(val wireValue: String) {
    SYSTEM_DYNAMIC("system_dynamic"),
    PRESET("preset"),
    CUSTOM("custom"),
    ;

    companion object {
        fun fromWireValue(value: String?): ColorSource =
            entries.firstOrNull { it.wireValue == value } ?: PRESET
    }
}

enum class ColorPalettePreset(
    val wireValue: String,
    val seedColorArgb: Int,
) {
    TERRACOTTA("terracotta", 0xFF7B4E3B.toInt()),
    STONE_INK("stone_ink", 0xFF5C4D43.toInt()),
    SAGE_FOREST("sage_forest", 0xFF3E6B4C.toInt()),
    OCEAN_SLATE("ocean_slate", 0xFF2B638B.toInt()),
    LAVENDER_IRIS("lavender_iris", 0xFF6B4FA2.toInt()),
    AMBER_OCHRE("amber_ochre", 0xFF965A1E.toInt()),
    ROSE_CLAY("rose_clay", 0xFF9C4157.toInt()),
    ;

    companion object {
        fun fromWireValue(value: String?): ColorPalettePreset =
            entries.firstOrNull { it.wireValue == value } ?: TERRACOTTA
    }
}

enum class PaletteStyle(val wireValue: String) {
    TONAL_SPOT("tonal_spot"),
    VIBRANT("vibrant"),
    EXPRESSIVE("expressive"),
    NEUTRAL("neutral"),
    MONOCHROME("monochrome"),
    ;

    companion object {
        fun fromWireValue(value: String?): PaletteStyle =
            entries.firstOrNull { it.wireValue == value } ?: TONAL_SPOT
    }
}

enum class ContrastLevel(
    val wireValue: String,
    val contrastValue: Double,
) {
    STANDARD("standard", 0.0),
    MEDIUM("medium", 0.5),
    HIGH("high", 1.0),
    ;

    companion object {
        fun fromWireValue(value: String?): ContrastLevel =
            entries.firstOrNull { it.wireValue == value } ?: STANDARD
    }
}

data class AppearanceSettings(
    val themeMode: ThemeMode = ThemeMode.FOLLOW_SYSTEM,
    val pureBlackDarkEnabled: Boolean = false,
    val colorSource: ColorSource = ColorSource.PRESET,
    val palettePreset: ColorPalettePreset = ColorPalettePreset.TERRACOTTA,
    val customSeedColorArgb: Int = DEFAULT_CUSTOM_SEED_COLOR_ARGB,
    val paletteStyle: PaletteStyle = PaletteStyle.TONAL_SPOT,
    val contrastLevel: ContrastLevel = ContrastLevel.STANDARD,
) {
    val dynamicColorEnabled: Boolean
        get() = colorSource == ColorSource.SYSTEM_DYNAMIC

    companion object {
        const val DEFAULT_CUSTOM_SEED_COLOR_ARGB: Int = 0xFF2B638B.toInt()
    }
}

interface AppearanceSettingsPort {
    val settings: Flow<AppearanceSettings>

    suspend fun setThemeMode(mode: ThemeMode)

    suspend fun setPureBlackDarkEnabled(enabled: Boolean)

    suspend fun selectSystemDynamicColor()

    suspend fun selectPalettePreset(preset: ColorPalettePreset)

    suspend fun setCustomSeedColor(argb: Int)

    suspend fun setPaletteStyle(style: PaletteStyle)

    suspend fun setContrastLevel(level: ContrastLevel)
}
