package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.settings.ApkInstallerPort
import com.sona.android.application.settings.AppApkAsset
import com.sona.android.application.settings.AppBuildInfo
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.AppReleaseInfo
import com.sona.android.application.settings.AppUpdateChannel
import com.sona.android.application.settings.AppUpdateDownloadPort
import com.sona.android.application.settings.AppUpdateDownloadProgress
import com.sona.android.application.settings.AppUpdateDownloadStage
import com.sona.android.application.settings.AppUpdateErrorCategory
import com.sona.android.application.settings.AppUpdatePort
import com.sona.android.application.settings.CheckForAppUpdate
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.GeneralSettings
import com.sona.android.application.settings.GeneralSettingsPort
import com.sona.android.application.settings.NetworkStatusPort
import java.net.SocketTimeoutException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test

class AboutSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `automatic check runs once and publishes an available update`() = runTest {
        val updates = FakeUpdatePort(release(versionCode = 12))
        val viewModel = viewModel(updates)

        viewModel.checkIfNeeded()
        viewModel.checkIfNeeded()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, updates.calls)
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.UpdateAvailable)
    }

    @Test
    fun `startup check respects autoCheckUpdates and ignored version code`() = runTest {
        val updates = FakeUpdatePort(release(versionCode = 12))
        val settings = FakeGeneralSettingsPort(
            GeneralSettings(autoCheckUpdates = false, ignoredUpdateVersionCode = 12),
        )
        val disabledVm = viewModel(updates, settings = settings)
        disabledVm.checkOnStartupIfEnabled()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(0, updates.calls)

        settings.setAutoCheckUpdates(true)
        val enabledVm = viewModel(updates, settings = settings)
        enabledVm.checkOnStartupIfEnabled()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, updates.calls)
        val status = enabledVm.state.value.updateStatus as AboutUpdateStatus.UpdateAvailable
        assertTrue(status.isIgnored)
        assertFalse(enabledVm.state.value.hasActionableUpdateBadge)
        assertFalse(enabledVm.state.value.showStartupBanner)
    }

    @Test
    fun `manual retry checks again after an error`() = runTest {
        val updates = FakeUpdatePort(release()).apply {
            failure = SocketTimeoutException("network detail")
        }
        val viewModel = viewModel(updates)

        viewModel.checkIfNeeded()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.Error)

        updates.failure = null
        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(2, updates.calls)
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.UpdateAvailable)
    }

    @Test
    fun `concurrent checks are suppressed`() = runTest {
        val updates = FakeUpdatePort(release()).apply { gate = CompletableDeferred() }
        val viewModel = viewModel(updates)

        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertEquals(1, updates.calls)
        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.Checking)
        updates.gate?.complete(Unit)
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
    }

    @Test
    fun `equal version is reported as current`() = runTest {
        val updates = FakeUpdatePort(release(versionCode = 11))
        val viewModel = viewModel(updates)

        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertTrue(viewModel.state.value.updateStatus is AboutUpdateStatus.UpToDate)
    }

    @Test
    fun `update check is skipped when channel does not support updates`() = runTest {
        val updates = FakeUpdatePort(release())
        val devBuild = build().copy(channel = AppUpdateChannel.DEV)
        val viewModel = AboutSettingsViewModel(
            build = devBuild,
            checkForAppUpdate = CheckForAppUpdate(updates),
        )

        viewModel.checkIfNeeded()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(0, updates.calls)

        viewModel.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertEquals(0, updates.calls)
    }

    @Test
    fun `metered network prompts for confirmation or blocks when wifiOnlyUpdates is enabled`() = runTest {
        val updates = FakeUpdatePort(releaseWithAsset())
        val settings = FakeGeneralSettingsPort(GeneralSettings(wifiOnlyUpdates = false))
        val installer = FakeApkInstaller(canInstall = true)
        val downloader = FakeDownloader()
        val vm = viewModel(
            updates = updates,
            downloader = downloader,
            installer = installer,
            settings = settings,
            networkStatus = NetworkStatusPort { true },
        )

        vm.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        vm.startDownload()
        assertTrue(vm.state.value.showCellularConfirmDialog)

        vm.confirmCellularDownload()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        assertFalse(vm.state.value.showCellularConfirmDialog)
        assertTrue(vm.state.value.updateStatus is AboutUpdateStatus.ReadyToInstall)
        assertEquals("/cache/sona-update-12.apk", installer.installedPath)

        settings.setWifiOnlyUpdates(true)
        val wifiOnlyVm = viewModel(
            updates = updates,
            downloader = downloader,
            installer = installer,
            settings = settings,
            networkStatus = NetworkStatusPort { true },
        )
        wifiOnlyVm.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        wifiOnlyVm.startDownload()
        val error = wifiOnlyVm.state.value.updateStatus as AboutUpdateStatus.Error
        assertEquals(AppUpdateErrorCategory.WIFI_REQUIRED, error.category)
    }

    @Test
    fun `install permission flow resumes automatically on host resume`() = runTest {
        val updates = FakeUpdatePort(releaseWithAsset())
        val installer = FakeApkInstaller(canInstall = false)
        val downloader = FakeDownloader()
        val vm = viewModel(
            updates = updates,
            downloader = downloader,
            installer = installer,
            networkStatus = NetworkStatusPort { false },
        )

        vm.checkForUpdates()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()
        vm.startDownload()
        mainDispatcherRule.dispatcher.scheduler.runCurrent()

        assertTrue(vm.state.value.showInstallPermissionDialog)
        assertNull(installer.installedPath)

        vm.openInstallPermissionSettings()
        assertTrue(installer.openedSettings)
        installer.canInstall = true
        vm.onHostResumed()

        assertEquals("/cache/sona-update-12.apk", installer.installedPath)
        assertFalse(vm.state.value.showInstallPermissionDialog)
    }

    private fun viewModel(
        updates: AppUpdatePort,
        downloader: AppUpdateDownloadPort? = null,
        installer: ApkInstallerPort? = null,
        settings: GeneralSettingsPort? = null,
        networkStatus: NetworkStatusPort? = null,
    ) = AboutSettingsViewModel(
        build = build(),
        checkForAppUpdate = CheckForAppUpdate(updates),
        updateDownloader = downloader,
        apkInstaller = installer,
        generalSettings = settings,
        networkStatus = networkStatus,
    )

    private class FakeUpdatePort(
        private val release: AppReleaseInfo,
    ) : AppUpdatePort {
        var calls = 0
        var failure: Throwable? = null
        var gate: CompletableDeferred<Unit>? = null

        override suspend fun latestRelease(
            channel: AppUpdateChannel,
            mirror: DownloadMirror,
        ): AppReleaseInfo {
            calls += 1
            gate?.await()
            failure?.let { throw it }
            return release
        }
    }

    private class FakeDownloader : AppUpdateDownloadPort {
        override suspend fun downloadApk(
            asset: AppApkAsset,
            versionCode: Int,
            mirror: DownloadMirror,
            onProgress: (AppUpdateDownloadProgress) -> Unit,
        ): String {
            onProgress(AppUpdateDownloadProgress(512L, 1024L, AppUpdateDownloadStage.DOWNLOADING))
            onProgress(AppUpdateDownloadProgress(1024L, 1024L, AppUpdateDownloadStage.VERIFYING))
            onProgress(AppUpdateDownloadProgress(1024L, 1024L, AppUpdateDownloadStage.READY))
            return "/cache/sona-update-$versionCode.apk"
        }

        override fun findCachedApk(release: AppReleaseInfo, supportedAbis: List<String>) = null
        override suspend fun cleanupStaleUpdates(currentVersionCode: Int) = Unit
        override suspend fun clearCachedUpdates(): Long = 0L
    }

    private class FakeApkInstaller(var canInstall: Boolean) : ApkInstallerPort {
        var installedPath: String? = null
        var openedSettings = false

        override fun supportedAbis(): List<String> = listOf("arm64-v8a", "x86_64")
        override fun canRequestPackageInstalls(): Boolean = canInstall
        override fun installApk(apkPath: String) {
            installedPath = apkPath
        }
        override fun openInstallPermissionSettings() {
            openedSettings = true
        }
    }

    private class FakeGeneralSettingsPort(
        initial: GeneralSettings = GeneralSettings(),
    ) : GeneralSettingsPort {
        private val state = MutableStateFlow(initial)
        override val settings: Flow<GeneralSettings> = state
        override suspend fun load(): GeneralSettings = state.value
        override suspend fun setAutoCheckUpdates(enabled: Boolean) {
            state.value = state.value.copy(autoCheckUpdates = enabled)
        }
        override suspend fun setWifiOnlyUpdates(enabled: Boolean) {
            state.value = state.value.copy(wifiOnlyUpdates = enabled)
        }
        override suspend fun setIgnoredUpdateVersionCode(versionCode: Int?) {
            state.value = state.value.copy(ignoredUpdateVersionCode = versionCode)
        }
        override suspend fun setDownloadMirror(mirror: DownloadMirror) {
            state.value = state.value.copy(downloadMirror = mirror)
        }
        override suspend fun setLogLevel(level: AppLogLevel) {
            state.value = state.value.copy(logLevel = level)
        }
    }

    companion object {
        private fun build() = AppBuildInfo(
            appName = "Sona",
            versionName = "1.2.3",
            versionCode = 11,
            channel = AppUpdateChannel.STABLE,
        )

        private fun release(versionCode: Int = 12) = AppReleaseInfo(
            versionName = "1.2.4",
            versionCode = versionCode,
            channel = AppUpdateChannel.STABLE,
            releasePageUrl = "https://github.com/AirSodaz/sona/releases/latest",
        )

        private fun releaseWithAsset(versionCode: Int = 12) = release(versionCode).copy(
            assets = mapOf(
                "arm64-v8a" to AppApkAsset(
                    abi = "arm64-v8a",
                    url = "https://github.com/AirSodaz/sona/releases/download/v1.2.4/app-arm64-v8a-debug.apk",
                    sha256 = "a".repeat(64),
                    sizeBytes = 1024L,
                ),
            ),
        )
    }
}
