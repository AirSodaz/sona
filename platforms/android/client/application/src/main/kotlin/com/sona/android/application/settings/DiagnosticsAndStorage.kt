package com.sona.android.application.settings

data class StorageUsageSummary(
    val totalAppBytes: Long,
    val audioBytes: Long,
    val databaseBytes: Long,
    val localModelsBytes: Long,
    val updateCacheBytes: Long,
    val tempCacheBytes: Long,
    val availableDeviceBytes: Long,
)

interface StorageUsagePort {
    suspend fun loadSummary(): StorageUsageSummary
    suspend fun clearTempCaches(): Long = 0L
}

data class DiagnosticsModelCheck(
    val slotName: String,
    val label: String,
    val ready: Boolean,
    val detail: String,
)

data class DiagnosticsSummary(
    val overallStatus: String,
    val deviceAbi: String,
    val deviceTier: String,
    val cpuCores: Int,
    val totalMemoryBytes: Long,
    val availableStorageBytes: Long,
    val microphonePermissionGranted: Boolean,
    val notificationPermissionGranted: Boolean,
    val liveModelCheck: DiagnosticsModelCheck,
    val batchModelCheck: DiagnosticsModelCheck,
    val logFileSizeBytes: Long,
    val scannedAt: String,
)

fun interface DiagnosticsPort {
    suspend fun loadSummary(
        microphonePermissionGranted: Boolean,
        notificationPermissionGranted: Boolean,
    ): DiagnosticsSummary
}

interface AppLogPort {
    fun currentLogLevel(): AppLogLevel
    fun applyLogLevel(level: AppLogLevel)
    suspend fun logSizeBytes(): Long
    suspend fun readRecentLogs(maxBytes: Int = 256 * 1024): String
    suspend fun clearLogs()
}

fun interface NetworkStatusPort {
    fun isActiveNetworkMetered(): Boolean
}

interface ApkInstallerPort {
    fun supportedAbis(): List<String>
    fun canRequestPackageInstalls(): Boolean
    fun installApk(apkPath: String)
    fun openInstallPermissionSettings()
}
