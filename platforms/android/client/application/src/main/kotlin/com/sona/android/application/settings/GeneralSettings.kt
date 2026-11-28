package com.sona.android.application.settings

import kotlinx.coroutines.flow.Flow

enum class DownloadMirror(val wireValue: String) {
    AUTO("auto"),
    DIRECT("direct"),
    GH_PROXY("ghproxy"),
    GH_NET("ghnet"),
    HF_MIRROR("hf-mirror"),
    ;

    companion object {
        fun fromWireValue(value: String?): DownloadMirror =
            entries.firstOrNull { it.wireValue == value?.trim()?.lowercase() } ?: AUTO
    }
}

enum class AppLogLevel(val wireValue: String, val priority: Int) {
    TRACE("trace", 0),
    DEBUG("debug", 1),
    INFO("info", 2),
    WARN("warn", 3),
    ERROR("error", 4),
    ;

    fun allows(level: AppLogLevel): Boolean = level.priority >= this.priority

    companion object {
        fun fromWireValue(value: String?, fallback: AppLogLevel = INFO): AppLogLevel =
            entries.firstOrNull { it.wireValue == value?.trim()?.lowercase() } ?: fallback
    }
}

data class GeneralSettings(
    val autoCheckUpdates: Boolean = true,
    val wifiOnlyUpdates: Boolean = false,
    val ignoredUpdateVersionCode: Int? = null,
    val downloadMirror: DownloadMirror = DownloadMirror.AUTO,
    val logLevel: AppLogLevel = AppLogLevel.INFO,
)

interface GeneralSettingsPort {
    val settings: Flow<GeneralSettings>

    suspend fun load(): GeneralSettings
    suspend fun setAutoCheckUpdates(enabled: Boolean)
    suspend fun setWifiOnlyUpdates(enabled: Boolean)
    suspend fun setIgnoredUpdateVersionCode(versionCode: Int?)
    suspend fun setDownloadMirror(mirror: DownloadMirror)
    suspend fun setLogLevel(level: AppLogLevel)
}

object DownloadUrlResolver {
    private const val GHPROXY_PREFIX = "https://mirror.ghproxy.com/"
    private const val GHNET_PREFIX = "https://ghproxy.net/"

    fun candidatesForGitHubUrl(originalUrl: String, mirror: DownloadMirror): List<String> {
        require(originalUrl.startsWith("https://")) { "Download URL must use HTTPS." }
        if (!isGitHubHost(originalUrl)) {
            return listOf(originalUrl)
        }
        val ghNet = "$GHNET_PREFIX$originalUrl"
        val ghProxy = "$GHPROXY_PREFIX$originalUrl"
        return when (mirror) {
            DownloadMirror.DIRECT, DownloadMirror.HF_MIRROR -> listOf(originalUrl)
            DownloadMirror.AUTO -> listOf(originalUrl, ghNet, ghProxy)
            DownloadMirror.GH_PROXY -> listOf(ghProxy, originalUrl, ghNet)
            DownloadMirror.GH_NET -> listOf(ghNet, originalUrl, ghProxy)
        }.distinct()
    }

    private fun isGitHubHost(url: String): Boolean {
        val host = url.removePrefix("https://")
            .substringBefore('/')
            .substringBefore('?')
            .substringBefore('#')
            .lowercase()
        return host == "github.com" ||
            host.endsWith(".github.com") ||
            host == "githubusercontent.com" ||
            host.endsWith(".githubusercontent.com")
    }
}
