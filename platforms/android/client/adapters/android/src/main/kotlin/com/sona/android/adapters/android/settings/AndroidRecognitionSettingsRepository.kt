package com.sona.android.adapters.android.settings

import android.content.Context
import androidx.datastore.core.DataStore
import androidx.datastore.core.handlers.ReplaceFileCorruptionHandler
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.emptyPreferences
import androidx.datastore.preferences.core.intPreferencesKey
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStoreFile
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDeviceCapabilitiesPort
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalAsrModelValidation
import com.sona.android.application.recording.LocalSherpaModelFiles
import com.sona.android.application.recording.RecognitionEngine
import com.sona.android.application.recording.RecognitionSettings
import com.sona.android.application.recording.RecognitionSettingsPort
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

class AndroidRecognitionSettingsRepository internal constructor(
    private val dataStore: DataStore<Preferences>,
    private val storage: AndroidLocalAsrModelStorage,
    private val deviceCapabilities: LocalAsrDeviceCapabilitiesPort,
) : RecognitionSettingsPort {
    private val modelOperationMutex = Mutex()

    override val settings: Flow<RecognitionSettings> = dataStore.data
        .catch { error ->
            if (error is IOException) emit(emptyPreferences()) else throw error
        }
        .map(::toSettings)
        .distinctUntilChanged()
        .flowOn(Dispatchers.IO)

    override suspend fun load(): RecognitionSettings = settings.first()

    override suspend fun selectEngine(engine: RecognitionEngine) {
        if (engine == RecognitionEngine.LOCAL) {
            require(load().localModel != null) { "A local model must be selected first." }
        }
        dataStore.edit { it[ENGINE] = engine.name }
    }

    override suspend fun selectLocalModel(modelId: String) {
        modelOperationMutex.withLock {
            val model = storage.listInstalledModels()
                .firstOrNull { it.model.id == modelId }
                ?.model
                ?: throw IllegalArgumentException("The selected local model is unavailable.")
            require(localModelIsUsable(model)) { "The selected local model is invalid." }
            persist(model)
        }
    }

    override suspend fun downloadLocalModel(
        model: LocalAsrCatalogModel,
        progress: LocalAsrDownloadProgressListener,
    ): LocalAsrModel = modelOperationMutex.withLock {
        val capabilities = deviceCapabilities.detect()
        require(capabilities.supported) { "This device does not support local recognition." }
        val threads = capabilities.recommendedThreads
        val installed = storage.downloadModel(model, threads, progress)
        try {
            persist(installed.model)
        } catch (error: Exception) {
            storage.deleteInstall(installed)
            throw error
        }
        storage.deleteOtherCatalogInstalls(model.id, installed.installRoot)
        bumpRevision()
        installed.model
    }

    override suspend fun validateLocalModel(modelId: String): LocalAsrModelValidation =
        modelOperationMutex.withLock {
            LocalAsrModelValidation(modelId, storage.validate(modelId))
        }

    override suspend fun deleteLocalModel(modelId: String) {
        modelOperationMutex.withLock {
            val activeModelId = load().localModel?.id
            storage.delete(modelId)
            dataStore.edit { preferences ->
                preferences[MODEL_REVISION] = (preferences[MODEL_REVISION] ?: 0) + 1
                if (activeModelId == modelId) {
                    preferences[ENGINE] = RecognitionEngine.ONLINE.name
                    clearActiveModel(preferences)
                }
            }
        }
    }

    private suspend fun persist(model: LocalAsrModel) {
        val config = model.config
        val files = config.fileConfig ?: LocalSherpaModelFiles()
        dataStore.edit { preferences ->
            preferences[ENGINE] = RecognitionEngine.LOCAL.name
            preferences[MODEL_ID] = model.id
            preferences[MODEL_NAME] = model.displayName
            preferences[MODEL_PATH] = config.modelPath
            preferences[MODEL_TYPE] = config.modelType
            preferences[MODEL_THREADS] = config.numThreads
            preferences[MODEL_REVISION] = (preferences[MODEL_REVISION] ?: 0) + 1
            putOptional(preferences, VAD_PATH, config.vadModel)
            putOptional(preferences, FILE_ENCODER, files.encoder)
            putOptional(preferences, FILE_DECODER, files.decoder)
            putOptional(preferences, FILE_MODEL, files.model)
            putOptional(preferences, FILE_JOINER, files.joiner)
            putOptional(preferences, FILE_TOKENS, files.tokens)
        }
    }

    private fun toSettings(preferences: Preferences): RecognitionSettings {
        val requestedEngine = runCatching {
            RecognitionEngine.valueOf(preferences[ENGINE].orEmpty())
        }.getOrDefault(RecognitionEngine.ONLINE)
        val installedModels = storage.listInstalledModels()
            .map { installed ->
                if (installed.model.id == preferences[MODEL_ID]) {
                    installed.model.copy(
                        config = installed.model.config.copy(
                            numThreads = (preferences[MODEL_THREADS] ?: DEFAULT_THREADS)
                                .coerceIn(1, 8),
                        ),
                    )
                } else {
                    installed.model
                }
            }
        val selectedModel = installedModels.firstOrNull { it.id == preferences[MODEL_ID] }
            ?: preferences[MODEL_PATH]?.let { selectedPath ->
                installedModels.firstOrNull {
                    runCatching { File(it.config.modelPath).canonicalPath }.getOrNull() ==
                        runCatching { File(selectedPath).canonicalPath }.getOrNull()
                }
            }
        val engine = if (requestedEngine == RecognitionEngine.LOCAL && selectedModel == null) {
            RecognitionEngine.ONLINE
        } else {
            requestedEngine
        }
        return RecognitionSettings(
            engine = engine,
            localModel = selectedModel,
            installedModels = installedModels,
        )
    }

    private suspend fun bumpRevision() {
        dataStore.edit { it[MODEL_REVISION] = (it[MODEL_REVISION] ?: 0) + 1 }
    }

    private fun clearActiveModel(
        preferences: androidx.datastore.preferences.core.MutablePreferences,
    ) {
        listOf(
            MODEL_ID,
            MODEL_NAME,
            MODEL_PATH,
            MODEL_TYPE,
            VAD_PATH,
            FILE_ENCODER,
            FILE_DECODER,
            FILE_MODEL,
            FILE_JOINER,
            FILE_TOKENS,
        ).forEach(preferences::remove)
        preferences.remove(MODEL_THREADS)
    }

    companion object {
        private const val DATASTORE_NAME = "recognition_settings"
        private const val DEFAULT_THREADS = 2
        private val ENGINE = stringPreferencesKey("engine")
        private val MODEL_ID = stringPreferencesKey("local_model_id")
        private val MODEL_NAME = stringPreferencesKey("local_model_name")
        private val MODEL_PATH = stringPreferencesKey("local_model_path")
        private val MODEL_TYPE = stringPreferencesKey("local_model_type")
        private val MODEL_THREADS = intPreferencesKey("local_model_threads")
        private val MODEL_REVISION = intPreferencesKey("local_model_revision")
        private val VAD_PATH = stringPreferencesKey("local_vad_path")
        private val FILE_ENCODER = stringPreferencesKey("local_file_encoder")
        private val FILE_DECODER = stringPreferencesKey("local_file_decoder")
        private val FILE_MODEL = stringPreferencesKey("local_file_model")
        private val FILE_JOINER = stringPreferencesKey("local_file_joiner")
        private val FILE_TOKENS = stringPreferencesKey("local_file_tokens")

        fun create(
            context: Context,
            deviceCapabilities: LocalAsrDeviceCapabilitiesPort,
        ): AndroidRecognitionSettingsRepository {
            val appContext = context.applicationContext
            return AndroidRecognitionSettingsRepository(
                dataStore = androidx.datastore.preferences.core.PreferenceDataStoreFactory.create(
                    corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    produceFile = { appContext.preferencesDataStoreFile(DATASTORE_NAME) },
                ),
                storage = AndroidLocalAsrModelStorage(appContext),
                deviceCapabilities = deviceCapabilities,
            )
        }
    }
}

private fun putOptional(
    preferences: androidx.datastore.preferences.core.MutablePreferences,
    key: Preferences.Key<String>,
    value: String?,
) {
    if (value == null) preferences.remove(key) else preferences[key] = value
}
