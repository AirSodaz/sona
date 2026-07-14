package com.sona.android.application.settings

import kotlinx.coroutines.flow.Flow

data class AppearanceSettings(
    val dynamicColorEnabled: Boolean = false,
)

interface AppearanceSettingsPort {
    val settings: Flow<AppearanceSettings>

    suspend fun setDynamicColorEnabled(enabled: Boolean)
}
