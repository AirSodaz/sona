package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDownloadFile
import com.sona.android.application.recording.LocalAsrModelCatalogPort
import uniffi.sona_uniffi_bind.FfiPresetModel
import uniffi.sona_uniffi_bind.presetModels

class UniffiLocalAsrModelCatalogAdapter : LocalAsrModelCatalogPort {
    override suspend fun loadStreamingModels(): List<LocalAsrCatalogModel> {
        val presets = presetModels()
        val vad = presets.firstOrNull { it.id == SILERO_VAD_ID }
        return presets
            .asSequence()
            .filter { "streaming" in it.modes }
            .filter { it.engine == "sherpa-onnx" }
            .filter { it.modelType in SUPPORTED_STREAMING_TYPES }
            .map { it.toApplication(vad) }
            .toList()
    }

    private fun FfiPresetModel.toApplication(vad: FfiPresetModel?): LocalAsrCatalogModel =
        LocalAsrCatalogModel(
            id = id,
            displayName = listOfNotNull(name, versionLabel).distinct().joinToString(" "),
            modelType = modelType,
            language = language,
            sizeLabel = size,
            estimatedSizeBytes = parseSizeBytes(size),
            isRecommended = isRecommended,
            download = toDownloadFile(),
            vadDownload = vad?.takeIf { rules.requiresVad }?.toDownloadFile(),
        )

    private fun FfiPresetModel.toDownloadFile(): LocalAsrDownloadFile = LocalAsrDownloadFile(
        url = url,
        sha256 = sha256,
        archive = isArchive,
        fileName = filename ?: if (isArchive) "$id.tar.bz2" else id,
    )

    companion object {
        private const val SILERO_VAD_ID = "silero-vad"
        private val SUPPORTED_STREAMING_TYPES = setOf(
            "sensevoice",
            "dolphin",
            "paraformer",
            "zipformer",
        )
    }
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
