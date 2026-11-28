package com.sona.android.application.settings

const val MAXIMUM_ANDROID_VERSION_CODE = 2_100_000_000

enum class AppUpdateChannel(val wireValue: String) {
    STABLE("stable"),
    NIGHTLY("nightly"),
    CI("ci"),
    DEV("dev"),
    ;

    val isUpdateSupported: Boolean
        get() = this == STABLE || this == NIGHTLY

    companion object {
        fun fromWireValue(value: String): AppUpdateChannel =
            entries.firstOrNull { it.wireValue == value }
                ?: throw IllegalArgumentException("Unsupported app update channel")
    }
}

data class AppBuildInfo(
    val appName: String,
    val versionName: String,
    val versionCode: Int,
    val channel: AppUpdateChannel,
) {
    init {
        require(appName.isNotBlank())
        require(versionName.isNotBlank())
        require(versionCode in 1..MAXIMUM_ANDROID_VERSION_CODE)
    }
}

data class AppApkAsset(
    val abi: String,
    val url: String,
    val sha256: String,
    val sizeBytes: Long,
) {
    init {
        require(abi.isNotBlank())
        require(url.startsWith("https://"))
        require(sha256.matches(Regex("[0-9a-f]{64}")))
        require(sizeBytes > 0L)
    }
}

data class AppReleaseInfo(
    val versionName: String,
    val versionCode: Int,
    val channel: AppUpdateChannel,
    val releasePageUrl: String,
    val pubDate: String? = null,
    val notes: String? = null,
    val signing: String? = null,
    val assets: Map<String, AppApkAsset> = emptyMap(),
) {
    init {
        require(versionName.isNotBlank())
        require(versionCode in 1..MAXIMUM_ANDROID_VERSION_CODE)
        require(releasePageUrl.startsWith("https://"))
    }

    fun selectAssetForAbis(supportedAbis: List<String>): AppApkAsset? {
        for (abi in supportedAbis) {
            assets[abi]?.let { return it }
        }
        return null
    }
}

enum class AppUpdateDownloadStage {
    DOWNLOADING,
    VERIFYING,
    READY,
}

data class AppUpdateDownloadProgress(
    val downloadedBytes: Long,
    val totalBytes: Long?,
    val stage: AppUpdateDownloadStage,
)

enum class AppUpdateErrorCategory {
    NETWORK,
    VERIFICATION_FAILED,
    UNSUPPORTED_ABI,
    WIFI_REQUIRED,
    INSTALL_PERMISSION_REQUIRED,
    INSTALL_FAILED,
}

class AppUpdateException(
    val category: AppUpdateErrorCategory,
    message: String,
    cause: Throwable? = null,
) : RuntimeException(message, cause)

fun interface AppUpdatePort {
    suspend fun latestRelease(channel: AppUpdateChannel, mirror: DownloadMirror): AppReleaseInfo
}

interface AppUpdateDownloadPort {
    suspend fun downloadApk(
        asset: AppApkAsset,
        versionCode: Int,
        mirror: DownloadMirror,
        onProgress: (AppUpdateDownloadProgress) -> Unit,
    ): String

    fun findCachedApk(release: AppReleaseInfo, supportedAbis: List<String>): Pair<AppApkAsset, String>?

    suspend fun cleanupStaleUpdates(currentVersionCode: Int)

    suspend fun clearCachedUpdates(): Long
}

sealed interface AppUpdateCheckResult {
    val latest: AppReleaseInfo

    data class UpToDate(
        override val latest: AppReleaseInfo,
    ) : AppUpdateCheckResult

    data class UpdateAvailable(
        override val latest: AppReleaseInfo,
    ) : AppUpdateCheckResult
}

class CheckForAppUpdate(
    private val updates: AppUpdatePort,
) {
    suspend operator fun invoke(
        current: AppBuildInfo,
        mirror: DownloadMirror = DownloadMirror.AUTO,
    ): AppUpdateCheckResult {
        val latest = updates.latestRelease(current.channel, mirror)
        require(latest.channel == current.channel) {
            "The update channel does not match the current build"
        }
        return if (latest.versionCode > current.versionCode) {
            AppUpdateCheckResult.UpdateAvailable(latest)
        } else {
            AppUpdateCheckResult.UpToDate(latest)
        }
    }
}
