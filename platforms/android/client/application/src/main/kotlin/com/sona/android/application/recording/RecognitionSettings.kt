package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

enum class AsrSelectionSlot {
    LIVE,
    BATCH,
}

enum class AsrMode {
    STREAMING,
    BATCH,
}

sealed interface AsrModelSelection {
    data class Local(val modelId: String) : AsrModelSelection
    data class Online(val provider: OnlineAsrProvider) : AsrModelSelection
}

data class LocalAsrModel(
    val id: String,
    val displayName: String,
    val config: LocalSherpaConfig,
    val supportedModes: Set<AsrMode> = setOf(AsrMode.STREAMING, AsrMode.BATCH),
    val sizeBytes: Long = 0,
    val source: LocalAsrModelSource = LocalAsrModelSource.IMPORTED,
) {
    fun supports(mode: AsrMode): Boolean = mode in supportedModes
}

enum class LocalAsrModelSource {
    CATALOG,
    IMPORTED,
}

data class LocalAsrCatalogModel(
    val id: String,
    val displayName: String,
    val modelType: String,
    val language: String,
    val sizeLabel: String,
    val estimatedSizeBytes: Long,
    val isRecommended: Boolean,
    val supportedModes: Set<AsrMode> = setOf(AsrMode.STREAMING, AsrMode.BATCH),
    val config: LocalSherpaConfig = LocalSherpaConfig(
        modelPath = "",
        numThreads = 2,
        modelType = modelType,
    ),
    val download: LocalAsrDownloadFile,
    val vadDownload: LocalAsrDownloadFile? = null,
    val punctuationDownload: LocalAsrDownloadFile? = null,
)

data class LocalAsrDownloadFile(
    val url: String,
    val sha256: String?,
    val archive: Boolean,
    val fileName: String,
)

enum class LocalAsrDownloadStage {
    DOWNLOADING,
    VERIFYING,
    INSTALLING,
}

data class LocalAsrDownloadProgress(
    val modelId: String,
    val stage: LocalAsrDownloadStage,
    val downloadedBytes: Long = 0,
    val totalBytes: Long = 0,
)

fun interface LocalAsrDownloadProgressListener {
    fun onProgress(progress: LocalAsrDownloadProgress)
}

data class LocalAsrModelValidation(
    val modelId: String,
    val valid: Boolean,
)

enum class LocalAsrDeviceTier {
    LIMITED,
    STANDARD,
    HIGH,
}

data class LocalAsrDeviceCapabilities(
    val supported: Boolean,
    val tier: LocalAsrDeviceTier,
    val cpuCores: Int,
    val totalMemoryBytes: Long,
    val availableStorageBytes: Long,
    val primaryAbi: String,
    val recommendedThreads: Int,
)

data class RecognitionSettings(
    val liveSelection: AsrModelSelection? = AsrModelSelection.Online(
        OnlineAsrProvider.VOLCENGINE_DOUBAO,
    ),
    val batchSelection: AsrModelSelection? = AsrModelSelection.Online(
        OnlineAsrProvider.VOLCENGINE_DOUBAO,
    ),
    val installedModels: List<LocalAsrModel> = emptyList(),
) {
    fun selectionFor(slot: AsrSelectionSlot): AsrModelSelection? = when (slot) {
        AsrSelectionSlot.LIVE -> liveSelection
        AsrSelectionSlot.BATCH -> batchSelection
    }
}

interface RecognitionSettingsPort {
    val settings: Flow<RecognitionSettings>

    suspend fun load(): RecognitionSettings
    suspend fun selectModel(slot: AsrSelectionSlot, selection: AsrModelSelection?)
    suspend fun downloadLocalModel(
        model: LocalAsrCatalogModel,
        progress: LocalAsrDownloadProgressListener,
    ): LocalAsrModel
    suspend fun validateLocalModel(modelId: String): LocalAsrModelValidation
    suspend fun deleteLocalModel(modelId: String)
}

fun interface LocalAsrModelCatalogPort {
    suspend fun loadModels(): List<LocalAsrCatalogModel>
}

fun interface LocalAsrDeviceCapabilitiesPort {
    suspend fun detect(): LocalAsrDeviceCapabilities
}

fun interface RecognitionSettingsResolverPort {
    suspend fun loadForStart(): RecognitionSettings
}

object OnlineRecognitionSettingsResolver : RecognitionSettingsResolverPort {
    override suspend fun loadForStart(): RecognitionSettings = RecognitionSettings()
}
