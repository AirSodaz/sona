package com.sona.android.app.feature.settings

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.settings.ApkInstallerPort
import com.sona.android.application.settings.AppApkAsset
import com.sona.android.application.settings.AppBuildInfo
import com.sona.android.application.settings.AppReleaseInfo
import com.sona.android.application.settings.AppUpdateCheckResult
import com.sona.android.application.settings.AppUpdateDownloadPort
import com.sona.android.application.settings.AppUpdateDownloadProgress
import com.sona.android.application.settings.AppUpdateDownloadStage
import com.sona.android.application.settings.AppUpdateErrorCategory
import com.sona.android.application.settings.AppUpdateException
import com.sona.android.application.settings.CheckForAppUpdate
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.GeneralSettingsPort
import com.sona.android.application.settings.NetworkStatusPort
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

interface AppUpdateNotificationPort {
    fun updateProgress(versionName: String, progress: AppUpdateDownloadProgress)
    fun showReadyToInstall(versionName: String)
    fun showFailed(versionName: String)
    fun cancel()
}

sealed interface AboutUpdateStatus {
    data object Idle : AboutUpdateStatus
    data object Checking : AboutUpdateStatus
    data class UpToDate(val latest: AppReleaseInfo) : AboutUpdateStatus
    data class UpdateAvailable(
        val release: AppReleaseInfo,
        val matchingAsset: AppApkAsset? = null,
        val isIgnored: Boolean = false,
    ) : AboutUpdateStatus
    data class Downloading(
        val release: AppReleaseInfo,
        val asset: AppApkAsset,
        val downloadedBytes: Long = 0L,
        val totalBytes: Long? = asset.sizeBytes,
    ) : AboutUpdateStatus
    data class Verifying(val release: AppReleaseInfo) : AboutUpdateStatus
    data class ReadyToInstall(
        val release: AppReleaseInfo,
        val apkPath: String,
    ) : AboutUpdateStatus
    data class Error(
        val category: AppUpdateErrorCategory = AppUpdateErrorCategory.NETWORK,
        val release: AppReleaseInfo? = null,
    ) : AboutUpdateStatus
}

data class AboutSettingsUiState(
    val build: AppBuildInfo,
    val updateStatus: AboutUpdateStatus = AboutUpdateStatus.Idle,
    val downloadMirror: DownloadMirror = DownloadMirror.AUTO,
    val wifiOnlyUpdates: Boolean = false,
    val ignoredVersionCode: Int? = null,
    val showCellularConfirmDialog: Boolean = false,
    val showInstallPermissionDialog: Boolean = false,
    val pendingInstallApkPath: String? = null,
    val bannerDismissedForSession: Boolean = false,
) {
    val hasActionableUpdateBadge: Boolean
        get() = when (updateStatus) {
            is AboutUpdateStatus.UpdateAvailable -> !updateStatus.isIgnored
            is AboutUpdateStatus.ReadyToInstall -> true
            else -> false
        }

    val showStartupBanner: Boolean
        get() = !bannerDismissedForSession && when (updateStatus) {
            is AboutUpdateStatus.UpdateAvailable -> !updateStatus.isIgnored
            is AboutUpdateStatus.ReadyToInstall -> true
            else -> false
        }
}

class AboutSettingsViewModel(
    build: AppBuildInfo,
    private val checkForAppUpdate: CheckForAppUpdate,
    private val updateDownloader: AppUpdateDownloadPort? = null,
    private val apkInstaller: ApkInstallerPort? = null,
    private val generalSettings: GeneralSettingsPort? = null,
    private val networkStatus: NetworkStatusPort? = null,
    private val notificationPort: AppUpdateNotificationPort? = null,
) : ViewModel() {
    private val mutableState = MutableStateFlow(AboutSettingsUiState(build = build))
    val state: StateFlow<AboutSettingsUiState> = mutableState.asStateFlow()
    private var automaticCheckStarted = false
    private var checkJob: Job? = null
    private var activeCheckSilent = true
    private var downloadJob: Job? = null

    init {
        if (generalSettings != null) {
            viewModelScope.launch {
                generalSettings.settings.collect { settings ->
                    mutableState.update { current ->
                        val updatedStatus = when (val status = current.updateStatus) {
                            is AboutUpdateStatus.UpdateAvailable -> status.copy(
                                isIgnored = settings.ignoredUpdateVersionCode == status.release.versionCode,
                            )
                            else -> status
                        }
                        current.copy(
                            updateStatus = updatedStatus,
                            downloadMirror = settings.downloadMirror,
                            wifiOnlyUpdates = settings.wifiOnlyUpdates,
                            ignoredVersionCode = settings.ignoredUpdateVersionCode,
                        )
                    }
                }
            }
        }
    }

    fun checkOnStartupIfEnabled() {
        if (!mutableState.value.build.channel.isUpdateSupported) return
        if (automaticCheckStarted) return
        automaticCheckStarted = true
        viewModelScope.launch {
            runCatching { updateDownloader?.cleanupStaleUpdates(mutableState.value.build.versionCode) }
            val settings = runCatching { generalSettings?.load() }.getOrNull()
            if (settings?.autoCheckUpdates != false) {
                performUpdateCheck(silent = true)
            }
        }
    }

    fun checkIfNeeded() {
        if (!mutableState.value.build.channel.isUpdateSupported) return
        if (automaticCheckStarted) return
        automaticCheckStarted = true
        performUpdateCheck(silent = false)
    }

    fun checkForUpdates() {
        if (!mutableState.value.build.channel.isUpdateSupported) return
        val status = mutableState.value.updateStatus
        if (status is AboutUpdateStatus.Checking ||
            status is AboutUpdateStatus.Downloading ||
            status is AboutUpdateStatus.Verifying
        ) {
            return
        }
        automaticCheckStarted = true
        performUpdateCheck(silent = false)
    }

    private fun performUpdateCheck(silent: Boolean) {
        val currentStatus = mutableState.value.updateStatus
        if (currentStatus is AboutUpdateStatus.Downloading ||
            currentStatus is AboutUpdateStatus.Verifying
        ) {
            return
        }
        if (checkJob?.isActive == true) {
            if (!silent && activeCheckSilent) {
                activeCheckSilent = false
                mutableState.update { it.copy(updateStatus = AboutUpdateStatus.Checking) }
            }
            return
        }
        activeCheckSilent = silent
        if (!silent) {
            mutableState.update { it.copy(updateStatus = AboutUpdateStatus.Checking) }
        }
        checkJob = viewModelScope.launch {
            try {
                val mirror = runCatching { generalSettings?.load()?.downloadMirror }.getOrNull()
                    ?: mutableState.value.downloadMirror
                val ignoredCode = runCatching { generalSettings?.load()?.ignoredUpdateVersionCode }.getOrNull()
                    ?: mutableState.value.ignoredVersionCode
                val result = checkForAppUpdate(mutableState.value.build, mirror)
                val supportedAbis = apkInstaller?.supportedAbis() ?: DEFAULT_SUPPORTED_ABIS
                mutableState.update { current ->
                    val nextStatus = when (result) {
                        is AppUpdateCheckResult.UpToDate -> AboutUpdateStatus.UpToDate(result.latest)
                        is AppUpdateCheckResult.UpdateAvailable -> {
                            val cached = updateDownloader?.findCachedApk(result.latest, supportedAbis)
                            if (cached != null) {
                                AboutUpdateStatus.ReadyToInstall(result.latest, cached.second)
                            } else {
                                AboutUpdateStatus.UpdateAvailable(
                                    release = result.latest,
                                    matchingAsset = result.latest.selectAssetForAbis(supportedAbis),
                                    isIgnored = ignoredCode == result.latest.versionCode,
                                )
                            }
                        }
                    }
                    current.copy(
                        updateStatus = nextStatus,
                        ignoredVersionCode = ignoredCode,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                if (!activeCheckSilent) {
                    mutableState.update {
                        it.copy(updateStatus = AboutUpdateStatus.Error(AppUpdateErrorCategory.NETWORK))
                    }
                }
            } finally {
                checkJob = null
            }
        }
    }

    fun startDownload(allowMetered: Boolean = false) {
        val release = currentReleaseFromState() ?: return
        val downloader = updateDownloader ?: return
        val supportedAbis = apkInstaller?.supportedAbis() ?: DEFAULT_SUPPORTED_ABIS
        val asset = release.selectAssetForAbis(supportedAbis)
        if (asset == null) {
            mutableState.update {
                it.copy(
                    updateStatus = AboutUpdateStatus.Error(
                        category = AppUpdateErrorCategory.UNSUPPORTED_ABI,
                        release = release,
                    ),
                )
            }
            return
        }

        val cached = downloader.findCachedApk(release, supportedAbis)
        if (cached != null) {
            mutableState.update {
                it.copy(updateStatus = AboutUpdateStatus.ReadyToInstall(release, cached.second))
            }
            installDownloadedApk()
            return
        }

        val metered = networkStatus?.isActiveNetworkMetered() == true
        if (metered) {
            if (mutableState.value.wifiOnlyUpdates) {
                mutableState.update {
                    it.copy(
                        updateStatus = AboutUpdateStatus.Error(
                            category = AppUpdateErrorCategory.WIFI_REQUIRED,
                            release = release,
                        ),
                    )
                }
                return
            }
            if (!allowMetered) {
                mutableState.update { it.copy(showCellularConfirmDialog = true) }
                return
            }
        }

        downloadJob?.cancel()
        mutableState.update {
            it.copy(
                showCellularConfirmDialog = false,
                updateStatus = AboutUpdateStatus.Downloading(
                    release = release,
                    asset = asset,
                    downloadedBytes = 0L,
                    totalBytes = asset.sizeBytes,
                ),
            )
        }
        downloadJob = viewModelScope.launch {
            try {
                val mirror = mutableState.value.downloadMirror
                val apkPath = downloader.downloadApk(asset, release.versionCode, mirror) { progress ->
                    if (progress.stage != AppUpdateDownloadStage.READY) {
                        notificationPort?.updateProgress(release.versionName, progress)
                    }
                    mutableState.update { current ->
                        val nextStatus = when (progress.stage) {
                            AppUpdateDownloadStage.DOWNLOADING -> AboutUpdateStatus.Downloading(
                                release = release,
                                asset = asset,
                                downloadedBytes = progress.downloadedBytes,
                                totalBytes = progress.totalBytes ?: asset.sizeBytes,
                            )
                            AppUpdateDownloadStage.VERIFYING -> AboutUpdateStatus.Verifying(release)
                            AppUpdateDownloadStage.READY -> current.updateStatus
                        }
                        current.copy(updateStatus = nextStatus)
                    }
                }
                notificationPort?.showReadyToInstall(release.versionName)
                mutableState.update {
                    it.copy(updateStatus = AboutUpdateStatus.ReadyToInstall(release, apkPath))
                }
                installDownloadedApk()
            } catch (error: CancellationException) {
                notificationPort?.cancel()
                mutableState.update {
                    it.copy(
                        updateStatus = AboutUpdateStatus.UpdateAvailable(
                            release = release,
                            matchingAsset = asset,
                            isIgnored = mutableState.value.ignoredVersionCode == release.versionCode,
                        ),
                    )
                }
                throw error
            } catch (error: AppUpdateException) {
                notificationPort?.showFailed(release.versionName)
                mutableState.update {
                    it.copy(
                        updateStatus = AboutUpdateStatus.Error(
                            category = error.category,
                            release = release,
                        ),
                    )
                }
            } catch (_: Exception) {
                notificationPort?.showFailed(release.versionName)
                mutableState.update {
                    it.copy(
                        updateStatus = AboutUpdateStatus.Error(
                            category = AppUpdateErrorCategory.NETWORK,
                            release = release,
                        ),
                    )
                }
            }
        }
    }

    fun confirmCellularDownload() {
        mutableState.update { it.copy(showCellularConfirmDialog = false) }
        startDownload(allowMetered = true)
    }

    fun dismissCellularConfirmDialog() {
        mutableState.update { it.copy(showCellularConfirmDialog = false) }
    }

    fun cancelDownload() {
        downloadJob?.cancel()
        downloadJob = null
    }

    fun retryDownloadWithMirror(mirror: DownloadMirror) {
        viewModelScope.launch {
            generalSettings?.setDownloadMirror(mirror)
            mutableState.update { it.copy(downloadMirror = mirror) }
            startDownload(allowMetered = true)
        }
    }

    fun ignoreAvailableUpdate() {
        val release = currentReleaseFromState() ?: return
        viewModelScope.launch {
            generalSettings?.setIgnoredUpdateVersionCode(release.versionCode)
            mutableState.update { current ->
                val status = current.updateStatus
                current.copy(
                    ignoredVersionCode = release.versionCode,
                    bannerDismissedForSession = true,
                    updateStatus = if (status is AboutUpdateStatus.UpdateAvailable) {
                        status.copy(isIgnored = true)
                    } else {
                        status
                    },
                )
            }
        }
    }

    fun clearIgnoredUpdate() {
        viewModelScope.launch {
            generalSettings?.setIgnoredUpdateVersionCode(null)
            mutableState.update { current ->
                val status = current.updateStatus
                current.copy(
                    ignoredVersionCode = null,
                    bannerDismissedForSession = false,
                    updateStatus = if (status is AboutUpdateStatus.UpdateAvailable) {
                        status.copy(isIgnored = false)
                    } else {
                        status
                    },
                )
            }
        }
    }

    fun dismissBannerForSession() {
        mutableState.update { it.copy(bannerDismissedForSession = true) }
    }

    fun installDownloadedApk() {
        val ready = mutableState.value.updateStatus as? AboutUpdateStatus.ReadyToInstall ?: return
        val installer = apkInstaller ?: return
        if (!installer.canRequestPackageInstalls()) {
            mutableState.update {
                it.copy(
                    showInstallPermissionDialog = true,
                    pendingInstallApkPath = ready.apkPath,
                )
            }
            return
        }
        try {
            installer.installApk(ready.apkPath)
        } catch (error: AppUpdateException) {
            mutableState.update {
                it.copy(
                    updateStatus = AboutUpdateStatus.Error(
                        category = error.category,
                        release = ready.release,
                    ),
                )
            }
        }
    }

    fun openInstallPermissionSettings() {
        mutableState.update { it.copy(showInstallPermissionDialog = false) }
        try {
            apkInstaller?.openInstallPermissionSettings()
        } catch (error: AppUpdateException) {
            val release = currentReleaseFromState()
            mutableState.update {
                it.copy(
                    pendingInstallApkPath = null,
                    updateStatus = AboutUpdateStatus.Error(
                        category = error.category,
                        release = release,
                    ),
                )
            }
        }
    }

    fun dismissInstallPermissionDialog() {
        mutableState.update {
            it.copy(
                showInstallPermissionDialog = false,
                pendingInstallApkPath = null,
            )
        }
    }

    fun onHostResumed() {
        val pendingPath = mutableState.value.pendingInstallApkPath ?: return
        val installer = apkInstaller ?: return
        mutableState.update {
            it.copy(
                showInstallPermissionDialog = false,
                pendingInstallApkPath = null,
            )
        }
        if (!installer.canRequestPackageInstalls()) return
        try {
            installer.installApk(pendingPath)
        } catch (error: AppUpdateException) {
            val release = currentReleaseFromState()
            mutableState.update {
                it.copy(
                    updateStatus = AboutUpdateStatus.Error(
                        category = error.category,
                        release = release,
                    ),
                )
            }
        }
    }

    fun onUpdateCacheCleared() {
        val ready = mutableState.value.updateStatus as? AboutUpdateStatus.ReadyToInstall ?: return
        val supportedAbis = apkInstaller?.supportedAbis() ?: DEFAULT_SUPPORTED_ABIS
        if (updateDownloader?.findCachedApk(ready.release, supportedAbis) != null) return
        notificationPort?.cancel()
        mutableState.update { current ->
            current.copy(
                showInstallPermissionDialog = false,
                pendingInstallApkPath = null,
                updateStatus = AboutUpdateStatus.UpdateAvailable(
                    release = ready.release,
                    matchingAsset = ready.release.selectAssetForAbis(supportedAbis),
                    isIgnored = current.ignoredVersionCode == ready.release.versionCode,
                ),
            )
        }
    }

    private fun currentReleaseFromState(): AppReleaseInfo? = when (val status = mutableState.value.updateStatus) {
        is AboutUpdateStatus.UpdateAvailable -> status.release
        is AboutUpdateStatus.Downloading -> status.release
        is AboutUpdateStatus.Verifying -> status.release
        is AboutUpdateStatus.ReadyToInstall -> status.release
        is AboutUpdateStatus.Error -> status.release
        else -> null
    }

    companion object {
        private val DEFAULT_SUPPORTED_ABIS = listOf("arm64-v8a", "x86_64")

        fun factory(
            build: AppBuildInfo,
            checkForAppUpdate: CheckForAppUpdate,
            updateDownloader: AppUpdateDownloadPort? = null,
            apkInstaller: ApkInstallerPort? = null,
            generalSettings: GeneralSettingsPort? = null,
            networkStatus: NetworkStatusPort? = null,
            notificationPort: AppUpdateNotificationPort? = null,
        ): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                require(modelClass.isAssignableFrom(AboutSettingsViewModel::class.java))
                return AboutSettingsViewModel(
                    build = build,
                    checkForAppUpdate = checkForAppUpdate,
                    updateDownloader = updateDownloader,
                    apkInstaller = apkInstaller,
                    generalSettings = generalSettings,
                    networkStatus = networkStatus,
                    notificationPort = notificationPort,
                ) as T
            }
        }
    }
}
