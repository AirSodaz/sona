package com.sona.android.adapters.android.settings

import android.app.ActivityManager
import android.content.Context
import android.os.Build
import android.os.StatFs
import com.sona.android.application.recording.LocalAsrDeviceCapabilities
import com.sona.android.application.recording.LocalAsrDeviceCapabilitiesPort
import com.sona.android.application.recording.LocalAsrDeviceTier
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

class AndroidLocalAsrDeviceCapabilities private constructor(
    private val appContext: Context,
) : LocalAsrDeviceCapabilitiesPort {
    override suspend fun detect(): LocalAsrDeviceCapabilities = withContext(Dispatchers.IO) {
        val memoryInfo = ActivityManager.MemoryInfo()
        appContext.getSystemService(ActivityManager::class.java)?.getMemoryInfo(memoryInfo)
        val totalMemory = memoryInfo.totalMem.coerceAtLeast(0)
        val availableStorage = runCatching {
            StatFs(appContext.filesDir.absolutePath).availableBytes
        }.getOrDefault(0)
        val cpuCores = Runtime.getRuntime().availableProcessors().coerceAtLeast(1)
        val primaryAbi = Build.SUPPORTED_ABIS.firstOrNull().orEmpty()
        val supported = primaryAbi in SUPPORTED_ABIS &&
            totalMemory >= MINIMUM_MEMORY_BYTES &&
            availableStorage >= MINIMUM_STORAGE_BYTES
        val tier = classifyLocalAsrDevice(totalMemory, availableStorage, cpuCores)

        LocalAsrDeviceCapabilities(
            supported = supported,
            tier = tier,
            cpuCores = cpuCores,
            totalMemoryBytes = totalMemory,
            availableStorageBytes = availableStorage,
            primaryAbi = primaryAbi,
            recommendedThreads = recommendedThreads(tier, cpuCores),
        )
    }

    companion object {
        private val SUPPORTED_ABIS = setOf("arm64-v8a", "x86_64")
        private const val MINIMUM_MEMORY_BYTES = 2L * 1_024 * 1_024 * 1_024
        private const val MINIMUM_STORAGE_BYTES = 512L * 1_024 * 1_024

        fun create(context: Context): AndroidLocalAsrDeviceCapabilities =
            AndroidLocalAsrDeviceCapabilities(context.applicationContext)
    }
}

internal fun classifyLocalAsrDevice(
    totalMemoryBytes: Long,
    availableStorageBytes: Long,
    cpuCores: Int,
): LocalAsrDeviceTier = when {
    totalMemoryBytes >= 8L * 1_024 * 1_024 * 1_024 &&
        availableStorageBytes >= 2L * 1_024 * 1_024 * 1_024 &&
        cpuCores >= 8 -> LocalAsrDeviceTier.HIGH
    totalMemoryBytes >= 4L * 1_024 * 1_024 * 1_024 &&
        availableStorageBytes >= 1L * 1_024 * 1_024 * 1_024 &&
        cpuCores >= 4 -> LocalAsrDeviceTier.STANDARD
    else -> LocalAsrDeviceTier.LIMITED
}

internal fun recommendedThreads(tier: LocalAsrDeviceTier, cpuCores: Int): Int {
    val tierLimit = when (tier) {
        LocalAsrDeviceTier.LIMITED -> 1
        LocalAsrDeviceTier.STANDARD -> 2
        LocalAsrDeviceTier.HIGH -> 4
    }
    return minOf(tierLimit, cpuCores.coerceAtLeast(1))
}
