package com.sona.android.application.settings

const val MAXIMUM_ANDROID_VERSION_CODE = 2_100_000_000

enum class AppUpdateChannel(val wireValue: String) {
    STABLE("stable"),
    NIGHTLY("nightly"),
    ;

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

data class AppReleaseInfo(
    val versionName: String,
    val versionCode: Int,
    val channel: AppUpdateChannel,
    val releasePageUrl: String,
) {
    init {
        require(versionName.isNotBlank())
        require(versionCode in 1..MAXIMUM_ANDROID_VERSION_CODE)
        require(releasePageUrl.startsWith("https://"))
    }
}

fun interface AppUpdatePort {
    suspend fun latestRelease(channel: AppUpdateChannel): AppReleaseInfo
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
    suspend operator fun invoke(current: AppBuildInfo): AppUpdateCheckResult {
        val latest = updates.latestRelease(current.channel)
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
