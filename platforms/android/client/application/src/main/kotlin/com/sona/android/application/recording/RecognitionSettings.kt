package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow

enum class RecognitionEngine {
    ONLINE,
    LOCAL,
}

data class LocalAsrModel(
    val displayName: String,
    val config: LocalSherpaStreamingConfig,
)

data class RecognitionSettings(
    val engine: RecognitionEngine = RecognitionEngine.ONLINE,
    val localModel: LocalAsrModel? = null,
)

interface RecognitionSettingsPort {
    val settings: Flow<RecognitionSettings>

    suspend fun load(): RecognitionSettings
    suspend fun selectEngine(engine: RecognitionEngine)
    suspend fun importLocalModel(sourceLocation: String): LocalAsrModel
}

fun interface RecognitionSettingsResolverPort {
    suspend fun loadForStart(): RecognitionSettings
}

object OnlineRecognitionSettingsResolver : RecognitionSettingsResolverPort {
    override suspend fun loadForStart(): RecognitionSettings = RecognitionSettings()
}
