package com.sona.android.adapters.uniffi.settings

import android.os.StatFs
import com.sona.android.application.settings.StorageUsagePort
import com.sona.android.application.settings.StorageUsageSummary
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.sona_uniffi_bind.loadStorageUsageSnapshotV1

class UniffiStorageUsageAdapter(
    private val appDataDir: String,
    private val cacheDir: File,
) : StorageUsagePort {
    override suspend fun loadSummary(): StorageUsageSummary = withContext(Dispatchers.IO) {
        val snapshot = loadStorageUsageSnapshotV1(appDataDir)
        val updateCacheBytes = directorySizeBytes(File(cacheDir, "updates"))
        val exportsBytes = directorySizeBytes(File(cacheDir, "exports"))
        val backupImportsBytes = directorySizeBytes(File(cacheDir, "backup-imports"))
        val tempCacheBytes = exportsBytes +
            backupImportsBytes +
            snapshot.categories.temporary.bytes.toLongSaturated()
        val availableDeviceBytes = runCatching { StatFs(appDataDir).availableBytes }.getOrDefault(0L)
        val audioBytes = snapshot.categories.audio.bytes.toLongSaturated()
        val databaseBytes = snapshot.categories.database.bytes.toLongSaturated()
        val modelsBytes = snapshot.categories.models.bytes.toLongSaturated()
        val totalAppBytes = snapshot.totalBytes.toLongSaturated() + updateCacheBytes +
            exportsBytes + backupImportsBytes

        StorageUsageSummary(
            totalAppBytes = totalAppBytes,
            audioBytes = audioBytes,
            databaseBytes = databaseBytes,
            localModelsBytes = modelsBytes,
            updateCacheBytes = updateCacheBytes,
            tempCacheBytes = tempCacheBytes,
            availableDeviceBytes = availableDeviceBytes,
        )
    }

    override suspend fun clearTempCaches(): Long = withContext(Dispatchers.IO) {
        var freed = 0L
        val dirs = listOf(
            File(cacheDir, "updates"),
            File(cacheDir, "exports"),
            File(cacheDir, "backup-imports"),
            File(appDataDir, "api_temp"),
        )
        for (dir in dirs) {
            if (dir.isDirectory) {
                dir.walkBottomUp().forEach { file ->
                    if (file.isFile) {
                        val size = file.length()
                        if (file.delete()) {
                            freed += size
                        }
                    } else if (file != dir && file.isDirectory && file.list()?.isEmpty() == true) {
                        file.delete()
                    }
                }
            }
        }
        freed
    }

    private fun directorySizeBytes(dir: File): Long {
        if (!dir.exists()) return 0L
        if (dir.isFile) return dir.length()
        var total = 0L
        dir.walkTopDown().forEach { file ->
            if (file.isFile) {
                total += file.length()
            }
        }
        return total
    }

    private fun ULong.toLongSaturated(): Long =
        coerceAtMost(Long.MAX_VALUE.toULong()).toLong()
}
