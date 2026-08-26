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

/** How a model handles ASR language selection; mirrors the core `languageMode`. */
enum class LanguageMode {
    SELECTABLE,
    AUTO,
    FIXED,
    NONE,
}

data class LocalAsrCatalogModel(
    val id: String,
    val displayName: String,
    val modelType: String,
    /** All recognizable languages, sorted ascending ISO 639 codes (`yue` = Cantonese). */
    val languages: List<String> = emptyList(),
    val languageMode: LanguageMode = LanguageMode.NONE,
    val sizeLabel: String,
    val estimatedSizeBytes: Long,
    val isRecommended: Boolean,
    val supportedModes: Set<AsrMode> = setOf(AsrMode.STREAMING, AsrMode.BATCH),
    val config: LocalSherpaConfig = LocalSherpaConfig(
        modelPath = "",
        numThreads = 2,
        modelType = modelType,
    ),
) {
    /**
     * Compact localized summary for catalog rows: the first few language names
     * plus a "+N" tail, so 100-language models stay readable.
     */
    fun languageSummary(locale: java.util.Locale = java.util.Locale.getDefault()): String {
        if (languages.isEmpty() || languageMode == LanguageMode.NONE) return ""
        val visible = languages.take(3).map { code -> languageDisplayName(code, locale) }
        val rest = languages.size - visible.size
        val joined = visible.joinToString(", ")
        return if (rest > 0) "$joined +$rest" else joined
    }
}

private fun languageDisplayName(code: String, locale: java.util.Locale): String =
    java.util.Locale.forLanguageTag(code).getDisplayLanguage(locale)
        .ifEmpty { code.uppercase(locale) }

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

interface LocalAsrModelStoragePort {
    fun listInstalledModels(numThreads: Int = 2): List<LocalAsrModel>

    suspend fun downloadModel(
        modelId: String,
        numThreads: Int,
        progress: LocalAsrDownloadProgressListener,
    ): LocalAsrModel

    suspend fun validateModel(modelId: String): Boolean

    suspend fun deleteModel(modelId: String)
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
