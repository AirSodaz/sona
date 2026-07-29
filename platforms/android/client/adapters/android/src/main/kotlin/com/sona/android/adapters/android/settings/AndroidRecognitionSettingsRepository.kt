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
import com.sona.android.application.recording.LocalAsrModelStoragePort
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
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.flowOn
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

class AndroidRecognitionSettingsRepository internal constructor(
    private val dataStore: DataStore<Preferences>,
    private val storage: LocalAsrModelStoragePort,
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
        require(
            hasLocalModelDownloadStorage(
                capabilities.availableStorageBytes,
                model.estimatedSizeBytes,
            ),
        ) {
            "Not enough storage for this model."
        }
        val installed = storage.downloadModel(model.id, capabilities.recommendedThreads, progress)
        bumpRevision()
        installed
    }

    override suspend fun validateLocalModel(modelId: String): LocalAsrModelValidation =
        modelOperationMutex.withLock {
            LocalAsrModelValidation(modelId, storage.validateModel(modelId))
        }

    override suspend fun deleteLocalModel(modelId: String) {
        modelOperationMutex.withLock {
            storage.deleteModel(modelId)
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
                    .firstOrNull { it.id == selection.modelId }
                    ?: throw IllegalArgumentException("The selected local model is unavailable.")
                require(model.supports(mode)) {
                    "The selected local model is incompatible or invalid."
                }
            }
            is AsrModelSelection.Online -> require(selection.provider.supports(mode)) {
                "The selected online provider is incompatible."
            }
        }
    }

    private fun toSettings(preferences: Preferences): RecognitionSettings {
        val installedModels = storage.listInstalledModels()
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
        private val MODEL_REVISION = intPreferencesKey("local_model_revision")

        fun create(
            context: Context,
            storage: LocalAsrModelStoragePort,
            deviceCapabilities: LocalAsrDeviceCapabilitiesPort,
        ): AndroidRecognitionSettingsRepository {
            val appContext = context.applicationContext
            return AndroidRecognitionSettingsRepository(
                dataStore = androidx.datastore.preferences.core.PreferenceDataStoreFactory.create(
                    corruptionHandler = ReplaceFileCorruptionHandler { emptyPreferences() },
                    scope = CoroutineScope(SupervisorJob() + Dispatchers.IO),
                    produceFile = { appContext.preferencesDataStoreFile(DATASTORE_NAME) },
                ),
                storage = storage,
                deviceCapabilities = deviceCapabilities,
            )
        }
    }
}

internal fun hasLocalModelDownloadStorage(availableBytes: Long, estimatedSizeBytes: Long): Boolean {
    if (estimatedSizeBytes <= 0) return true
    if (estimatedSizeBytes > (Long.MAX_VALUE - LOCAL_MODEL_STORAGE_MARGIN_BYTES) / 2) return false
    val requiredBytes = estimatedSizeBytes * 2 + LOCAL_MODEL_STORAGE_MARGIN_BYTES
    return availableBytes >= requiredBytes
}

private const val LOCAL_MODEL_STORAGE_MARGIN_BYTES = 128L * 1_024 * 1_024

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
