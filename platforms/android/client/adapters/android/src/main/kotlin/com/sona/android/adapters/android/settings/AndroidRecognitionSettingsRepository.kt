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
import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDeviceCapabilitiesPort
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalAsrModelValidation
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.RecognitionSettings
import com.sona.android.application.recording.RecognitionSettingsPort
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.catch
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.emitAll
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

class AndroidRecognitionSettingsRepository internal constructor(
    private val dataStore: DataStore<Preferences>,
    private val storage: AndroidLocalAsrModelStorage,
    private val deviceCapabilities: LocalAsrDeviceCapabilitiesPort,
    private val legacyBatchProvider: suspend () -> OnlineAsrProvider = {
        OnlineAsrProvider.VOLCENGINE_DOUBAO
    },
) : RecognitionSettingsPort {
    private val modelOperationMutex = Mutex()
    private val migrationMutex = Mutex()

    override val settings: Flow<RecognitionSettings> = flow {
        migrateIfNeeded()
        emitAll(dataStore.data)
    }
        .catch { error ->
            if (error is IOException) emit(emptyPreferences()) else throw error
        }
        .map(::toSettings)
        .distinctUntilChanged()
        .flowOn(Dispatchers.IO)

    override suspend fun load(): RecognitionSettings = settings.first()

    override suspend fun selectModel(slot: AsrSelectionSlot, selection: AsrModelSelection?) {
        selection?.let { validateSelection(slot, it) }
        dataStore.edit { preferences -> writeSelection(preferences, slot, selection) }
    }

    override suspend fun downloadLocalModel(
        model: LocalAsrCatalogModel,
        progress: LocalAsrDownloadProgressListener,
    ): LocalAsrModel = modelOperationMutex.withLock {
        val capabilities = deviceCapabilities.detect()
        require(capabilities.supported) { "This device does not support local recognition." }
        val installed = storage.downloadModel(model, capabilities.recommendedThreads, progress)
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
            storage.delete(modelId)
            dataStore.edit { preferences ->
                preferences[MODEL_REVISION] = (preferences[MODEL_REVISION] ?: 0) + 1
                AsrSelectionSlot.entries.forEach { slot ->
                    val selection = readSelection(preferences, slot)
                    if (selection == AsrModelSelection.Local(modelId)) {
                        writeSelection(preferences, slot, null)
                    }
                }
            }
        }
    }

    private fun validateSelection(slot: AsrSelectionSlot, selection: AsrModelSelection) {
        val mode = slot.mode
        when (selection) {
            is AsrModelSelection.Local -> {
                val model = storage.listInstalledModels()
                    .firstOrNull { it.model.id == selection.modelId }
                    ?.model
                    ?: throw IllegalArgumentException("The selected local model is unavailable.")
                require(model.supports(mode) && localModelIsUsable(model)) {
                    "The selected local model is incompatible or invalid."
                }
            }
            is AsrModelSelection.Online -> require(selection.provider.supports(mode)) {
                "The selected online provider is incompatible."
            }
        }
    }

    private suspend fun migrateIfNeeded() = migrationMutex.withLock {
        val preferences = dataStore.data.first()
        if ((preferences[SCHEMA_VERSION] ?: 0) >= CURRENT_SCHEMA_VERSION) return@withLock

        val hasNewSelection = AsrSelectionSlot.entries.any { slot ->
            preferences[selectionKindKey(slot)] != null
        }
        val hasLegacySelection = preferences[LEGACY_ENGINE] != null ||
            preferences[LEGACY_MODEL_ID] != null
        val installedModels = storage.listInstalledModels().map(InstalledLocalAsrModel::model)

        val migrated = if (hasNewSelection) {
            null
        } else if (!hasLegacySelection) {
            RecognitionSettings()
        } else {
            val localRequested = preferences[LEGACY_ENGINE] == "LOCAL"
            migrateLegacyRecognitionSettings(
                legacyEngine = preferences[LEGACY_ENGINE],
                legacyModelId = preferences[LEGACY_MODEL_ID],
                installedModels = installedModels,
                legacyBatchProvider = if (localRequested) {
                    OnlineAsrProvider.VOLCENGINE_DOUBAO
                } else {
                    legacyBatchProvider()
                },
            )
        }

        dataStore.edit { mutable ->
            if (!hasNewSelection && migrated != null) {
                writeSelection(mutable, AsrSelectionSlot.LIVE, migrated.liveSelection)
                writeSelection(mutable, AsrSelectionSlot.BATCH, migrated.batchSelection)
            }
            mutable[SCHEMA_VERSION] = CURRENT_SCHEMA_VERSION
            LEGACY_STRING_KEYS.forEach(mutable::remove)
            LEGACY_INT_KEYS.forEach(mutable::remove)
        }
    }

    private fun toSettings(preferences: Preferences): RecognitionSettings {
        val installedModels = storage.listInstalledModels().map(InstalledLocalAsrModel::model)
        return RecognitionSettings(
            liveSelection = readSelection(preferences, AsrSelectionSlot.LIVE),
            batchSelection = readSelection(preferences, AsrSelectionSlot.BATCH),
            installedModels = installedModels,
        )
    }

    private suspend fun bumpRevision() {
        dataStore.edit { it[MODEL_REVISION] = (it[MODEL_REVISION] ?: 0) + 1 }
    }

    companion object {
        private const val DATASTORE_NAME = "recognition_settings"
        private const val CURRENT_SCHEMA_VERSION = 2
        private val SCHEMA_VERSION = intPreferencesKey("schema_version")
        private val MODEL_REVISION = intPreferencesKey("local_model_revision")
        private val LEGACY_ENGINE = stringPreferencesKey("engine")
        private val LEGACY_MODEL_ID = stringPreferencesKey("local_model_id")
        private val LEGACY_STRING_KEYS = listOf(
            LEGACY_ENGINE,
            LEGACY_MODEL_ID,
            stringPreferencesKey("local_model_name"),
            stringPreferencesKey("local_model_path"),
            stringPreferencesKey("local_model_type"),
            stringPreferencesKey("local_vad_path"),
            stringPreferencesKey("local_file_encoder"),
            stringPreferencesKey("local_file_decoder"),
            stringPreferencesKey("local_file_model"),
            stringPreferencesKey("local_file_joiner"),
            stringPreferencesKey("local_file_tokens"),
        )
        private val LEGACY_INT_KEYS = listOf(
            intPreferencesKey("local_model_threads"),
        )

        fun create(
            context: Context,
            deviceCapabilities: LocalAsrDeviceCapabilitiesPort,
            legacyBatchProvider: suspend () -> OnlineAsrProvider = {
                OnlineAsrProvider.VOLCENGINE_DOUBAO
            },
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
                legacyBatchProvider = legacyBatchProvider,
            )
        }
    }
}

private val AsrSelectionSlot.mode: AsrMode
    get() = if (this == AsrSelectionSlot.LIVE) AsrMode.STREAMING else AsrMode.BATCH

private fun selectionKindKey(slot: AsrSelectionSlot) =
    if (slot == AsrSelectionSlot.LIVE) LIVE_SELECTION_KIND else BATCH_SELECTION_KIND

private fun selectionValueKey(slot: AsrSelectionSlot) =
    if (slot == AsrSelectionSlot.LIVE) LIVE_SELECTION_VALUE else BATCH_SELECTION_VALUE

private fun readSelection(preferences: Preferences, slot: AsrSelectionSlot): AsrModelSelection? =
    when (preferences[selectionKindKey(slot)]) {
        "LOCAL" -> preferences[selectionValueKey(slot)]?.let(AsrModelSelection::Local)
        "ONLINE" -> preferences[selectionValueKey(slot)]?.let { value ->
            runCatching { OnlineAsrProvider.valueOf(value) }.getOrNull()
                ?.let(AsrModelSelection::Online)
        }
        else -> null
    }

private fun writeSelection(
    preferences: androidx.datastore.preferences.core.MutablePreferences,
    slot: AsrSelectionSlot,
    selection: AsrModelSelection?,
) {
    val kindKey = selectionKindKey(slot)
    val valueKey = selectionValueKey(slot)
    when (selection) {
        null -> {
            preferences.remove(kindKey)
            preferences.remove(valueKey)
        }
        is AsrModelSelection.Local -> {
            preferences[kindKey] = "LOCAL"
            preferences[valueKey] = selection.modelId
        }
        is AsrModelSelection.Online -> {
            preferences[kindKey] = "ONLINE"
            preferences[valueKey] = selection.provider.name
        }
    }
}

private val LIVE_SELECTION_KIND = stringPreferencesKey("live_selection_kind")
private val LIVE_SELECTION_VALUE = stringPreferencesKey("live_selection_value")
private val BATCH_SELECTION_KIND = stringPreferencesKey("batch_selection_kind")
private val BATCH_SELECTION_VALUE = stringPreferencesKey("batch_selection_value")

internal fun migrateLegacyRecognitionSettings(
    legacyEngine: String?,
    legacyModelId: String?,
    installedModels: List<LocalAsrModel>,
    legacyBatchProvider: OnlineAsrProvider,
): RecognitionSettings {
    val model = installedModels.firstOrNull { it.id == legacyModelId }
    if (legacyEngine == "LOCAL" && model != null) {
        return RecognitionSettings(
            liveSelection = model.takeIf { it.supports(AsrMode.STREAMING) }
                ?.let { AsrModelSelection.Local(it.id) },
            batchSelection = model.takeIf { it.supports(AsrMode.BATCH) }
                ?.let { AsrModelSelection.Local(it.id) },
            installedModels = installedModels,
        )
    }
    return RecognitionSettings(
        liveSelection = AsrModelSelection.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO),
        batchSelection = AsrModelSelection.Online(legacyBatchProvider),
        installedModels = installedModels,
    )
}
