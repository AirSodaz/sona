package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.LanguageMode
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrModelCatalogPort
import com.sona.android.application.recording.LocalSherpaConfig
import com.sona.android.application.recording.AsrMode
import uniffi.sona_uniffi_bind.FfiLanguageMode
import uniffi.sona_uniffi_bind.FfiPresetModel
import uniffi.sona_uniffi_bind.presetModels

class UniffiLocalAsrModelCatalogAdapter(
    private val modelSources: AndroidModelSourceRepository = AndroidModelSourceRepository(),
) : LocalAsrModelCatalogPort {
    override suspend fun loadModels(): List<LocalAsrCatalogModel> {
        val presets = presetModels().associateBy { it.id }
        val sources = modelSources.loadSources()
        return sources.mapNotNull { source ->
            val preset = presets[source.id]
            if (preset != null) {
                preset.toApplication(source)
            } else {
                source.toApplication()
            }
        }.filter { it.supportedModes.isNotEmpty() }
    }

    private fun FfiPresetModel.toApplication(source: AndroidModelSource? = null): LocalAsrCatalogModel =
        LocalAsrCatalogModel(
            id = id,
            displayName = source?.name?.ifBlank { null }
                ?: listOfNotNull(name, versionLabel).distinct().joinToString(" "),
            modelType = modelType,
            languages = if (source != null && source.languages.isNotEmpty()) source.languages else languages,
            languageMode = languageMode.toApplication(),
            sizeLabel = if (source != null && source.size.isNotBlank()) source.size else size,
            estimatedSizeBytes = parseSizeBytes(if (source != null && source.size.isNotBlank()) source.size else size),
            isRecommended = source?.isRecommended ?: isRecommended,
            supportedModes = modes.mapNotNullTo(mutableSetOf()) {
                when (it) {
                    "streaming" -> AsrMode.STREAMING
                    "batch" -> AsrMode.BATCH
                    else -> null
                }
            },
            config = LocalSherpaConfig(
                modelPath = "",
                numThreads = 2,
                modelType = modelType,
            ),
        )

    private fun AndroidModelSource.toApplication(): LocalAsrCatalogModel =
        LocalAsrCatalogModel(
            id = id,
            displayName = name,
            modelType = modelType,
            languages = languages,
            languageMode = when (languageMode.lowercase()) {
                "selectable" -> LanguageMode.SELECTABLE
                "fixed" -> LanguageMode.FIXED
                "none" -> LanguageMode.NONE
                else -> LanguageMode.AUTO
            },
            sizeLabel = size,
            estimatedSizeBytes = parseSizeBytes(size),
            isRecommended = isRecommended,
            supportedModes = modes.mapNotNullTo(mutableSetOf()) {
                when (it) {
                    "streaming" -> AsrMode.STREAMING
                    "batch" -> AsrMode.BATCH
                    else -> null
                }
            },
            config = LocalSherpaConfig(
                modelPath = "",
                numThreads = 2,
                modelType = modelType,
            ),
        )
}

internal fun parseSizeBytes(value: String): Long {
    val match = Regex("""~?\s*([0-9]+(?:\.[0-9]+)?)\s*(KB|MB|GB)""", RegexOption.IGNORE_CASE)
        .find(value) ?: return 0
    val amount = match.groupValues[1].toDoubleOrNull() ?: return 0
    val multiplier = when (match.groupValues[2].uppercase()) {
        "KB" -> 1_024.0
        "MB" -> 1_024.0 * 1_024.0
        "GB" -> 1_024.0 * 1_024.0 * 1_024.0
        else -> return 0
    }
    return (amount * multiplier).toLong()
}

private fun FfiLanguageMode.toApplication(): LanguageMode = when (this) {
    FfiLanguageMode.SELECTABLE -> LanguageMode.SELECTABLE
    FfiLanguageMode.AUTO -> LanguageMode.AUTO
    FfiLanguageMode.FIXED -> LanguageMode.FIXED
    FfiLanguageMode.NONE -> LanguageMode.NONE
}
