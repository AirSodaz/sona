package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.recording.LocalAsrCatalogModel
import com.sona.android.application.recording.LocalAsrDeviceCapabilities
import com.sona.android.application.recording.LocalAsrDeviceCapabilitiesPort
import com.sona.android.application.recording.LocalAsrDeviceTier
import com.sona.android.application.recording.LocalAsrDownloadFile
import com.sona.android.application.recording.LocalAsrDownloadProgress
import com.sona.android.application.recording.LocalAsrDownloadProgressListener
import com.sona.android.application.recording.LocalAsrDownloadStage
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalAsrModelCatalogPort
import com.sona.android.application.recording.LocalAsrModelValidation
import com.sona.android.application.recording.LocalSherpaConfig
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.RecognitionSettings
import com.sona.android.application.recording.RecognitionSettingsPort
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test

class RecognitionSettingsViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `catalog and device capabilities load with settings`() =
        runTest(mainDispatcherRule.dispatcher) {
            val viewModel = createViewModel()

            advanceUntilIdle()

            assertEquals("sensevoice", viewModel.uiState.value.catalogModels.single().id)
            assertEquals(LocalAsrDeviceTier.STANDARD, viewModel.uiState.value.deviceCapabilities?.tier)
            assertEquals(false, viewModel.uiState.value.catalogLoading)
        }

    @Test
    fun `download publishes installed model without changing either selection`() =
        runTest(mainDispatcherRule.dispatcher) {
            val viewModel = createViewModel()
            advanceUntilIdle()

            viewModel.downloadLocalModel("sensevoice")
            advanceUntilIdle()

            assertEquals(
                AsrModelSelection.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO),
                viewModel.uiState.value.liveSelection,
            )
            assertEquals(
                AsrModelSelection.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO),
                viewModel.uiState.value.batchSelection,
            )
            assertEquals(1, viewModel.uiState.value.installedModels.size)
            assertEquals(null, viewModel.uiState.value.operationModelId)
        }

    @Test
    fun `validation and deletion update model directory state`() =
        runTest(mainDispatcherRule.dispatcher) {
            val port = FakeRecognitionSettingsPort().apply { installModel() }
            val viewModel = createViewModel(port)
            advanceUntilIdle()

            viewModel.validateLocalModel("sensevoice")
            advanceUntilIdle()
            assertEquals(true, viewModel.uiState.value.validationByModelId["sensevoice"])

            viewModel.deleteLocalModel("sensevoice")
            advanceUntilIdle()
            assertEquals(emptyList<LocalAsrModel>(), viewModel.uiState.value.installedModels)
            assertEquals(
                AsrModelSelection.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO),
                viewModel.uiState.value.liveSelection,
            )
        }

    private fun createViewModel(
        port: FakeRecognitionSettingsPort = FakeRecognitionSettingsPort(),
    ): RecognitionSettingsViewModel = RecognitionSettingsViewModel(
        settingsPort = port,
        catalogPort = LocalAsrModelCatalogPort { listOf(catalogModel()) },
        deviceCapabilitiesPort = LocalAsrDeviceCapabilitiesPort { deviceCapabilities() },
    )
}

private class FakeRecognitionSettingsPort : RecognitionSettingsPort {
    private val mutableSettings = MutableStateFlow(RecognitionSettings())
    override val settings: Flow<RecognitionSettings> = mutableSettings

    override suspend fun load(): RecognitionSettings = mutableSettings.value

    override suspend fun selectModel(slot: AsrSelectionSlot, selection: AsrModelSelection?) {
        mutableSettings.value = when (slot) {
            AsrSelectionSlot.LIVE -> mutableSettings.value.copy(liveSelection = selection)
            AsrSelectionSlot.BATCH -> mutableSettings.value.copy(batchSelection = selection)
        }
    }

    override suspend fun downloadLocalModel(
        model: LocalAsrCatalogModel,
        progress: LocalAsrDownloadProgressListener,
    ): LocalAsrModel {
        progress.onProgress(
            LocalAsrDownloadProgress(model.id, LocalAsrDownloadStage.DOWNLOADING, 5, 10),
        )
        return installModel()
    }

    override suspend fun validateLocalModel(modelId: String): LocalAsrModelValidation =
        LocalAsrModelValidation(modelId, valid = true)

    override suspend fun deleteLocalModel(modelId: String) {
        mutableSettings.value = RecognitionSettings()
    }

    fun installModel(): LocalAsrModel {
        val model = LocalAsrModel(
            id = "sensevoice",
            displayName = "SenseVoice",
            config = LocalSherpaConfig(
                modelPath = "/models/sensevoice",
                numThreads = 2,
                modelType = "sensevoice",
            ),
        )
        mutableSettings.value = mutableSettings.value.copy(installedModels = listOf(model))
        return model
    }
}

private fun catalogModel() = LocalAsrCatalogModel(
    id = "sensevoice",
    displayName = "SenseVoice",
    modelType = "sensevoice",
    language = "zh,en",
    sizeLabel = "155 MB",
    estimatedSizeBytes = 155L * 1_024 * 1_024,
    isRecommended = true,
    download = LocalAsrDownloadFile(
        url = "https://example.com/sensevoice.tar.bz2",
        sha256 = null,
        archive = true,
        fileName = "sensevoice.tar.bz2",
    ),
)

private fun deviceCapabilities() = LocalAsrDeviceCapabilities(
    supported = true,
    tier = LocalAsrDeviceTier.STANDARD,
    cpuCores = 8,
    totalMemoryBytes = 8L * 1_024 * 1_024 * 1_024,
    availableStorageBytes = 4L * 1_024 * 1_024 * 1_024,
    primaryAbi = "arm64-v8a",
    recommendedThreads = 2,
)
