package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.settings.AppBuildInfo
import com.sona.android.application.settings.AppReleaseInfo
import com.sona.android.application.settings.AppUpdateCheckResult
import com.sona.android.application.settings.CheckForAppUpdate
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

sealed interface AboutUpdateStatus {
    data object Idle : AboutUpdateStatus
    data object Checking : AboutUpdateStatus
    data class UpToDate(val latest: AppReleaseInfo) : AboutUpdateStatus
    data class UpdateAvailable(val release: AppReleaseInfo) : AboutUpdateStatus
    data object Error : AboutUpdateStatus
}

data class AboutSettingsUiState(
    val build: AppBuildInfo,
    val updateStatus: AboutUpdateStatus = AboutUpdateStatus.Idle,
)

class AboutSettingsViewModel(
    build: AppBuildInfo,
    private val checkForAppUpdate: CheckForAppUpdate,
) : ViewModel() {
    private val mutableState = MutableStateFlow(AboutSettingsUiState(build = build))
    val state: StateFlow<AboutSettingsUiState> = mutableState.asStateFlow()
    private var automaticCheckStarted = false

    fun checkIfNeeded() {
        if (automaticCheckStarted) return
        automaticCheckStarted = true
        checkForUpdates()
    }

    fun checkForUpdates() {
        if (mutableState.value.updateStatus is AboutUpdateStatus.Checking) return
        automaticCheckStarted = true
        mutableState.update { it.copy(updateStatus = AboutUpdateStatus.Checking) }
        viewModelScope.launch {
            try {
                val result = checkForAppUpdate(mutableState.value.build)
                mutableState.update { current ->
                    current.copy(
                        updateStatus = when (result) {
                            is AppUpdateCheckResult.UpToDate ->
                                AboutUpdateStatus.UpToDate(result.latest)
                            is AppUpdateCheckResult.UpdateAvailable ->
                                AboutUpdateStatus.UpdateAvailable(result.latest)
                        },
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update { it.copy(updateStatus = AboutUpdateStatus.Error) }
            }
        }
    }

    companion object {
        fun factory(
            build: AppBuildInfo,
            checkForAppUpdate: CheckForAppUpdate,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(AboutSettingsViewModel::class.java))
                return AboutSettingsViewModel(build, checkForAppUpdate) as T
            }
        }
    }
}
