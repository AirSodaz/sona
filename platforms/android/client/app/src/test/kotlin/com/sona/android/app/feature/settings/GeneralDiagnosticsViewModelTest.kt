package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.AppLogPort
import com.sona.android.application.settings.DiagnosticsModelCheck
import com.sona.android.application.settings.DiagnosticsPort
import com.sona.android.application.settings.DiagnosticsSummary
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.GeneralSettings
import com.sona.android.application.settings.GeneralSettingsPort
import com.sona.android.application.settings.StorageUsagePort
import com.sona.android.application.settings.StorageUsageSummary
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class GeneralDiagnosticsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `loads storage, diagnostics, updates mirror and log level, and clears caches`() = runTest {
        val repo = InMemoryGeneralSettingsPort()
        val logger = InMemoryAppLogPort()
        val storage = InMemoryStoragePort()
        val transfers = InMemoryFileTransferPort()
        var cacheClearedCallbackCount = 0
        val vm = GeneralDiagnosticsViewModel(
            generalSettings = repo,
            storageUsage = storage,
            diagnostics = DiagnosticsPort { mic, notif ->
                DiagnosticsSummary(
                    overallStatus = if (mic) "ready" else "attention",
                    deviceAbi = "arm64-v8a",
                    deviceTier = "high",
                    cpuCores = 8,
                    totalMemoryBytes = 8_000_000_000L,
                    availableStorageBytes = 20_000_000_000L,
                    microphonePermissionGranted = mic,
                    notificationPermissionGranted = notif,
                    liveModelCheck = DiagnosticsModelCheck("live", "Volcengine", true, "credential_configured"),
                    batchModelCheck = DiagnosticsModelCheck("batch", "SenseVoice", true, "installed"),
                    logFileSizeBytes = 2048L,
                    scannedAt = "2026-11-28T12:00:00Z",
                )
            },
            appLogs = logger,
            fileTransfers = transfers,
            permissionChecker = { true to true },
            onUpdateCacheCleared = { cacheClearedCallbackCount += 1 },
        )

        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals("ready", vm.state.value.diagnosticsSummary?.overallStatus)
        assertEquals(2048L, vm.state.value.logSizeBytes)

        vm.setDownloadMirror(DownloadMirror.GH_NET)
        vm.setLogLevel(AppLogLevel.DEBUG)
        vm.clearTemporaryCaches()
        vm.exportLogs("content://test/logs.txt")
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, cacheClearedCallbackCount)
        assertEquals(DownloadMirror.GH_NET, vm.state.value.settings.downloadMirror)
        assertEquals(AppLogLevel.DEBUG, logger.currentLogLevel())
        assertTrue(transfers.publishedTexts.single().contains("Sona Android Diagnostics"))
        assertEquals(GeneralDiagnosticsNotice.LOGS_EXPORTED, vm.state.value.notice)
    }

    private class InMemoryGeneralSettingsPort : GeneralSettingsPort {
        private val flow = MutableStateFlow(GeneralSettings())
        override val settings: Flow<GeneralSettings> = flow
        override suspend fun load(): GeneralSettings = flow.value
        override suspend fun setAutoCheckUpdates(enabled: Boolean) {
            flow.value = flow.value.copy(autoCheckUpdates = enabled)
        }
        override suspend fun setWifiOnlyUpdates(enabled: Boolean) {
            flow.value = flow.value.copy(wifiOnlyUpdates = enabled)
        }
        override suspend fun setIgnoredUpdateVersionCode(versionCode: Int?) {
            flow.value = flow.value.copy(ignoredUpdateVersionCode = versionCode)
        }
        override suspend fun setDownloadMirror(mirror: DownloadMirror) {
            flow.value = flow.value.copy(downloadMirror = mirror)
        }
        override suspend fun setLogLevel(level: AppLogLevel) {
            flow.value = flow.value.copy(logLevel = level)
        }
    }

    private class InMemoryStoragePort : StorageUsagePort {
        private var cacheBytes = 4096L
        override suspend fun loadSummary(): StorageUsageSummary = StorageUsageSummary(
            totalAppBytes = 10_000L + cacheBytes,
            audioBytes = 4_000L,
            databaseBytes = 2_000L,
            localModelsBytes = 4_000L,
            updateCacheBytes = cacheBytes,
            tempCacheBytes = 0L,
            availableDeviceBytes = 50_000_000L,
        )
        override suspend fun clearTempCaches(): Long {
            val freed = cacheBytes
            cacheBytes = 0L
            return freed
        }
    }

    private class InMemoryAppLogPort : AppLogPort {
        private var level = AppLogLevel.INFO
        private var size = 2048L
        override fun currentLogLevel(): AppLogLevel = level
        override fun applyLogLevel(level: AppLogLevel) {
            this.level = level
        }
        override suspend fun logSizeBytes(): Long = size
        override suspend fun readRecentLogs(maxBytes: Int): String = "sample log line"
        override suspend fun clearLogs() {
            size = 0L
        }
    }

    private class InMemoryFileTransferPort : FileTransferPort {
        val publishedTexts = mutableListOf<String>()
        override suspend fun stageImport(sourceUri: String): String = ""
        override suspend fun publishExport(stagedPath: String, destinationUri: String) = Unit
        override suspend fun createExportStagingPath(fileName: String): String = ""
        override suspend fun cleanup(path: String) = Unit
        override suspend fun publishText(text: String, destinationUri: String) {
            publishedTexts += text
        }
    }
}
