package com.sona.android.app.feature.settings

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.recording.LocalAsrModel
import com.sona.android.application.recording.LocalSherpaStreamingConfig
import com.sona.android.application.recording.RecognitionEngine
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
    fun `selecting an engine is persisted through the settings port`() =
        runTest(mainDispatcherRule.dispatcher) {
            val port = FakeRecognitionSettingsPort()
            val viewModel = RecognitionSettingsViewModel(port)
            advanceUntilIdle()

            viewModel.selectEngine(RecognitionEngine.LOCAL)
            advanceUntilIdle()

            assertEquals(RecognitionEngine.LOCAL, viewModel.uiState.value.engine)
        }

    @Test
    fun `successful model import publishes the model and selects local recognition`() =
        runTest(mainDispatcherRule.dispatcher) {
            val port = FakeRecognitionSettingsPort()
            val viewModel = RecognitionSettingsViewModel(port)
            advanceUntilIdle()

            viewModel.importLocalModel("content://models/tree")
            advanceUntilIdle()

            assertEquals("SenseVoice", viewModel.uiState.value.localModel?.displayName)
            assertEquals(RecognitionEngine.LOCAL, viewModel.uiState.value.engine)
            assertEquals(false, viewModel.uiState.value.importInProgress)
            assertEquals(false, viewModel.uiState.value.importError)
        }
}

private class FakeRecognitionSettingsPort : RecognitionSettingsPort {
    private val mutableSettings = MutableStateFlow(RecognitionSettings())
    override val settings: Flow<RecognitionSettings> = mutableSettings

    override suspend fun load(): RecognitionSettings = mutableSettings.value

    override suspend fun selectEngine(engine: RecognitionEngine) {
        mutableSettings.value = mutableSettings.value.copy(engine = engine)
    }

    override suspend fun importLocalModel(sourceLocation: String): LocalAsrModel {
        val model = LocalAsrModel(
            displayName = "SenseVoice",
            config = LocalSherpaStreamingConfig(
                modelPath = "/models/sensevoice",
                numThreads = 2,
                modelType = "sensevoice",
            ),
        )
        mutableSettings.value = RecognitionSettings(RecognitionEngine.LOCAL, model)
        return model
    }
}
