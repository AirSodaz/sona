package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.settings.AppearanceSettings
import com.sona.android.application.settings.AppearanceSettingsPort
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ColorSource
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

data class AppearanceSettingsUiState(
    val themeMode: ThemeMode = ThemeMode.FOLLOW_SYSTEM,
    val pureBlackDarkEnabled: Boolean = false,
    val colorSource: ColorSource = ColorSource.PRESET,
    val palettePreset: ColorPalettePreset = ColorPalettePreset.TERRACOTTA,
    val customSeedColorArgb: Int = AppearanceSettings.DEFAULT_CUSTOM_SEED_COLOR_ARGB,
    val paletteStyle: PaletteStyle = PaletteStyle.TONAL_SPOT,
    val contrastLevel: ContrastLevel = ContrastLevel.STANDARD,
    val isLoaded: Boolean = false,
    val operationInProgress: Boolean = false,
    val hasError: Boolean = false,
) {
    val dynamicColorEnabled: Boolean
        get() = colorSource == ColorSource.SYSTEM_DYNAMIC

    fun toDomainSettings(): AppearanceSettings = AppearanceSettings(
        themeMode = themeMode,
        pureBlackDarkEnabled = pureBlackDarkEnabled,
        colorSource = colorSource,
        palettePreset = palettePreset,
        customSeedColorArgb = customSeedColorArgb,
        paletteStyle = paletteStyle,
        contrastLevel = contrastLevel,
    )
}

class AppearanceSettingsViewModel(
    private val settings: AppearanceSettingsPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(AppearanceSettingsUiState())
    val state: StateFlow<AppearanceSettingsUiState> = mutableState.asStateFlow()

    private val writeMutex = Mutex()
    private var confirmedSettings = AppearanceSettings()
    private var pendingMutationCount = 0

    init {
        viewModelScope.launch {
            settings.settings
                .catch { error ->
                    if (error is CancellationException) throw error
                    mutableState.update { it.copy(isLoaded = true, hasError = true) }
                }
                .collect { appearanceSettings ->
                    mutableState.update { current ->
                        if (pendingMutationCount > 0) {
                            current.copy(isLoaded = true)
                        } else {
                            val matchesConfirmed = current.isLoaded && appearanceSettings == confirmedSettings
                            confirmedSettings = appearanceSettings
                            current.applyDomainSettings(appearanceSettings).copy(
                                isLoaded = true,
                                hasError = if (matchesConfirmed) current.hasError else false,
                            )
                        }
                    }
                }
        }
    }

    fun setThemeMode(mode: ThemeMode) {
        if (mutableState.value.themeMode == mode) return
        performOptimisticUpdate(
            transform = { it.copy(themeMode = mode) },
            applyDomain = { it.copy(themeMode = mode) },
            persist = { settings.setThemeMode(mode) },
        )
    }

    fun setPureBlackDarkEnabled(enabled: Boolean) {
        if (mutableState.value.pureBlackDarkEnabled == enabled) return
        performOptimisticUpdate(
            transform = { it.copy(pureBlackDarkEnabled = enabled) },
            applyDomain = { it.copy(pureBlackDarkEnabled = enabled) },
            persist = { settings.setPureBlackDarkEnabled(enabled) },
        )
    }

    fun selectSystemDynamicColor() {
        if (mutableState.value.colorSource == ColorSource.SYSTEM_DYNAMIC) return
        performOptimisticUpdate(
            transform = { it.copy(colorSource = ColorSource.SYSTEM_DYNAMIC) },
            applyDomain = { it.copy(colorSource = ColorSource.SYSTEM_DYNAMIC) },
            persist = { settings.selectSystemDynamicColor() },
        )
    }

    fun selectPalettePreset(preset: ColorPalettePreset) {
        val current = mutableState.value
        if (current.colorSource == ColorSource.PRESET && current.palettePreset == preset) return
        performOptimisticUpdate(
            transform = {
                it.copy(
                    colorSource = ColorSource.PRESET,
                    palettePreset = preset,
                )
            },
            applyDomain = {
                it.copy(
                    colorSource = ColorSource.PRESET,
                    palettePreset = preset,
                )
            },
            persist = { settings.selectPalettePreset(preset) },
        )
    }

    fun setCustomSeedColor(argb: Int) {
        val normalizedArgb = argb or 0xFF000000.toInt()
        val current = mutableState.value
        if (current.colorSource == ColorSource.CUSTOM && current.customSeedColorArgb == normalizedArgb) {
            return
        }
        performOptimisticUpdate(
            transform = {
                it.copy(
                    colorSource = ColorSource.CUSTOM,
                    customSeedColorArgb = normalizedArgb,
                )
            },
            applyDomain = {
                it.copy(
                    colorSource = ColorSource.CUSTOM,
                    customSeedColorArgb = normalizedArgb,
                )
            },
            persist = { settings.setCustomSeedColor(normalizedArgb) },
        )
    }

    fun setPaletteStyle(style: PaletteStyle) {
        if (mutableState.value.paletteStyle == style) return
        performOptimisticUpdate(
            transform = { it.copy(paletteStyle = style) },
            applyDomain = { it.copy(paletteStyle = style) },
            persist = { settings.setPaletteStyle(style) },
        )
    }

    fun setContrastLevel(level: ContrastLevel) {
        if (mutableState.value.contrastLevel == level) return
        performOptimisticUpdate(
            transform = { it.copy(contrastLevel = level) },
            applyDomain = { it.copy(contrastLevel = level) },
            persist = { settings.setContrastLevel(level) },
        )
    }

    private fun performOptimisticUpdate(
        transform: (AppearanceSettingsUiState) -> AppearanceSettingsUiState,
        applyDomain: (AppearanceSettings) -> AppearanceSettings,
        persist: suspend () -> Unit,
    ) {
        val current = mutableState.value
        if (!current.isLoaded) return

        val startingNewBatch = pendingMutationCount == 0
        pendingMutationCount += 1
        mutableState.value = transform(current).copy(
            operationInProgress = true,
            hasError = if (startingNewBatch) false else current.hasError,
        )
        viewModelScope.launch {
            writeMutex.withLock {
                try {
                    persist()
                    confirmedSettings = applyDomain(confirmedSettings)
                    pendingMutationCount = (pendingMutationCount - 1).coerceAtLeast(0)
                    if (pendingMutationCount == 0) {
                        mutableState.update { state ->
                            state.applyDomainSettings(confirmedSettings).copy(
                                operationInProgress = false,
                            )
                        }
                    }
                } catch (error: CancellationException) {
                    pendingMutationCount = (pendingMutationCount - 1).coerceAtLeast(0)
                    if (pendingMutationCount == 0) {
                        mutableState.update { state ->
                            state.applyDomainSettings(confirmedSettings).copy(
                                operationInProgress = false,
                            )
                        }
                    }
                    throw error
                } catch (_: Exception) {
                    pendingMutationCount = (pendingMutationCount - 1).coerceAtLeast(0)
                    mutableState.update { state ->
                        if (pendingMutationCount == 0) {
                            state.applyDomainSettings(confirmedSettings).copy(
                                operationInProgress = false,
                                hasError = true,
                            )
                        } else {
                            state.copy(hasError = true)
                        }
                    }
                }
            }
        }
    }

    private fun AppearanceSettingsUiState.applyDomainSettings(
        domain: AppearanceSettings,
    ): AppearanceSettingsUiState = copy(
        themeMode = domain.themeMode,
        pureBlackDarkEnabled = domain.pureBlackDarkEnabled,
        colorSource = domain.colorSource,
        palettePreset = domain.palettePreset,
        customSeedColorArgb = domain.customSeedColorArgb,
        paletteStyle = domain.paletteStyle,
        contrastLevel = domain.contrastLevel,
    )

    companion object {
        fun factory(settings: AppearanceSettingsPort): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(AppearanceSettingsViewModel::class.java))
                    return AppearanceSettingsViewModel(settings) as T
                }
            }
    }
}
