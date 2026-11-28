package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.AppLogPort
import com.sona.android.application.settings.DiagnosticsPort
import com.sona.android.application.settings.DiagnosticsSummary
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.GeneralSettings
import com.sona.android.application.settings.GeneralSettingsPort
import com.sona.android.application.settings.StorageUsagePort
import com.sona.android.application.settings.StorageUsageSummary
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

enum class GeneralDiagnosticsNotice {
    CACHE_CLEARED,
    LOGS_EXPORTED,
    LOGS_CLEARED,
    OPERATION_FAILED,
}

data class GeneralDiagnosticsUiState(
    val settings: GeneralSettings = GeneralSettings(),
    val storageSummary: StorageUsageSummary? = null,
    val diagnosticsSummary: DiagnosticsSummary? = null,
    val logSizeBytes: Long = 0L,
    val loadingStorage: Boolean = false,
    val loadingDiagnostics: Boolean = false,
    val clearingCache: Boolean = false,
    val exportingLogs: Boolean = false,
    val notice: GeneralDiagnosticsNotice? = null,
    val lastFreedBytes: Long = 0L,
)

class GeneralDiagnosticsViewModel(
    private val generalSettings: GeneralSettingsPort,
    private val storageUsage: StorageUsagePort,
    private val diagnostics: DiagnosticsPort,
    private val appLogs: AppLogPort,
    private val fileTransfers: FileTransferPort,
    private val permissionChecker: () -> Pair<Boolean, Boolean>,
    private val onUpdateCacheCleared: (() -> Unit)? = null,
) : ViewModel() {
    private val mutableState = MutableStateFlow(GeneralDiagnosticsUiState())
    val state: StateFlow<GeneralDiagnosticsUiState> = mutableState.asStateFlow()

    init {
        viewModelScope.launch {
            generalSettings.settings.collect { settings ->
                mutableState.update { it.copy(settings = settings) }
                appLogs.applyLogLevel(settings.logLevel)
            }
        }
        refresh()
    }

    fun refresh() {
        refreshStorage()
        refreshDiagnostics()
    }

    fun refreshStorage() {
        mutableState.update { it.copy(loadingStorage = true) }
        viewModelScope.launch {
            try {
                val summary = storageUsage.loadSummary()
                val logBytes = appLogs.logSizeBytes()
                mutableState.update {
                    it.copy(
                        storageSummary = summary,
                        logSizeBytes = logBytes,
                        loadingStorage = false,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(
                        loadingStorage = false,
                        notice = GeneralDiagnosticsNotice.OPERATION_FAILED,
                    )
                }
            }
        }
    }

    fun refreshDiagnostics() {
        mutableState.update { it.copy(loadingDiagnostics = true) }
        viewModelScope.launch {
            try {
                val (micGranted, notificationGranted) = permissionChecker()
                val summary = diagnostics.loadSummary(micGranted, notificationGranted)
                val logBytes = appLogs.logSizeBytes()
                mutableState.update {
                    it.copy(
                        diagnosticsSummary = summary,
                        logSizeBytes = logBytes,
                        loadingDiagnostics = false,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(
                        loadingDiagnostics = false,
                        notice = GeneralDiagnosticsNotice.OPERATION_FAILED,
                    )
                }
            }
        }
    }

    fun setAutoCheckUpdates(enabled: Boolean) {
        viewModelScope.launch { generalSettings.setAutoCheckUpdates(enabled) }
    }

    fun setWifiOnlyUpdates(enabled: Boolean) {
        viewModelScope.launch { generalSettings.setWifiOnlyUpdates(enabled) }
    }

    fun setDownloadMirror(mirror: DownloadMirror) {
        viewModelScope.launch { generalSettings.setDownloadMirror(mirror) }
    }

    fun setLogLevel(level: AppLogLevel) {
        appLogs.applyLogLevel(level)
        viewModelScope.launch { generalSettings.setLogLevel(level) }
    }

    fun clearIgnoredUpdateVersion() {
        viewModelScope.launch { generalSettings.setIgnoredUpdateVersionCode(null) }
    }

    fun clearTemporaryCaches() {
        if (mutableState.value.clearingCache) return
        mutableState.update { it.copy(clearingCache = true, notice = null) }
        viewModelScope.launch {
            try {
                val freed = storageUsage.clearTempCaches()
                onUpdateCacheCleared?.invoke()
                val summary = storageUsage.loadSummary()
                mutableState.update {
                    it.copy(
                        clearingCache = false,
                        storageSummary = summary,
                        lastFreedBytes = freed,
                        notice = GeneralDiagnosticsNotice.CACHE_CLEARED,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(
                        clearingCache = false,
                        notice = GeneralDiagnosticsNotice.OPERATION_FAILED,
                    )
                }
            }
        }
    }

    fun exportLogs(destinationUri: String) {
        if (mutableState.value.exportingLogs) return
        mutableState.update { it.copy(exportingLogs = true, notice = null) }
        viewModelScope.launch {
            try {
                val report = buildDiagnosticReportText()
                val logs = appLogs.readRecentLogs()
                val content = buildString {
                    appendLine(report)
                    appendLine()
                    appendLine("=== Application Logs ===")
                    append(if (logs.isBlank()) "(no logs recorded)" else logs)
                }
                fileTransfers.publishText(content, destinationUri)
                mutableState.update {
                    it.copy(
                        exportingLogs = false,
                        notice = GeneralDiagnosticsNotice.LOGS_EXPORTED,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update {
                    it.copy(
                        exportingLogs = false,
                        notice = GeneralDiagnosticsNotice.OPERATION_FAILED,
                    )
                }
            }
        }
    }

    fun clearLogs() {
        viewModelScope.launch {
            try {
                appLogs.clearLogs()
                mutableState.update {
                    it.copy(
                        logSizeBytes = 0L,
                        notice = GeneralDiagnosticsNotice.LOGS_CLEARED,
                    )
                }
                refreshDiagnostics()
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update { it.copy(notice = GeneralDiagnosticsNotice.OPERATION_FAILED) }
            }
        }
    }

    fun dismissNotice() {
        mutableState.update { it.copy(notice = null) }
    }

    fun buildDiagnosticReportText(): String {
        val diag = mutableState.value.diagnosticsSummary
        val storage = mutableState.value.storageSummary
        val settings = mutableState.value.settings
        return buildString {
            appendLine("=== Sona Android Diagnostics ===")
            appendLine("Download Mirror: ${settings.downloadMirror.wireValue}")
            appendLine("Auto Check Updates: ${settings.autoCheckUpdates}")
            appendLine("Wi-Fi Only Updates: ${settings.wifiOnlyUpdates}")
            appendLine("Log Level: ${settings.logLevel.wireValue}")
            if (diag != null) {
                appendLine("Overall Status: ${diag.overallStatus}")
                appendLine("Device ABI: ${diag.deviceAbi} (${diag.deviceTier}, ${diag.cpuCores} cores)")
                appendLine("Mic Permission: ${diag.microphonePermissionGranted}")
                appendLine("Notification Permission: ${diag.notificationPermissionGranted}")
                appendLine("Live Model: ${diag.liveModelCheck.label} [${diag.liveModelCheck.detail}]")
                appendLine("Batch Model: ${diag.batchModelCheck.label} [${diag.batchModelCheck.detail}]")
                appendLine("Scanned At: ${diag.scannedAt}")
            }
            if (storage != null) {
                appendLine("App Storage Total: ${storage.totalAppBytes} bytes")
                appendLine("Recordings: ${storage.audioBytes} bytes | Models: ${storage.localModelsBytes} bytes | DB: ${storage.databaseBytes} bytes")
                appendLine("Update Cache: ${storage.updateCacheBytes} bytes | Temp Cache: ${storage.tempCacheBytes} bytes")
            }
        }.trimEnd()
    }

    companion object {
        fun factory(
            generalSettings: GeneralSettingsPort,
            storageUsage: StorageUsagePort,
            diagnostics: DiagnosticsPort,
            appLogs: AppLogPort,
            fileTransfers: FileTransferPort,
            permissionChecker: () -> Pair<Boolean, Boolean>,
            onUpdateCacheCleared: (() -> Unit)? = null,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(GeneralDiagnosticsViewModel::class.java))
                return GeneralDiagnosticsViewModel(
                    generalSettings = generalSettings,
                    storageUsage = storageUsage,
                    diagnostics = diagnostics,
                    appLogs = appLogs,
                    fileTransfers = fileTransfers,
                    permissionChecker = permissionChecker,
                    onUpdateCacheCleared = onUpdateCacheCleared,
                ) as T
            }
        }
    }
}
