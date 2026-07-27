package com.sona.android.app

import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.core.os.LocaleListCompat
import com.sona.android.app.composition.SonaAppContainer
import com.sona.android.app.feature.bootstrap.SonaBootstrapViewModel
import com.sona.android.app.feature.library.LibraryViewModel
import com.sona.android.app.feature.recording.RecordingViewModel
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.AppearanceSettingsViewModel
import com.sona.android.app.feature.settings.CloudTranscriptionSettingsViewModel
import com.sona.android.app.feature.settings.CredentialSettingsViewModel
import com.sona.android.app.feature.settings.RecognitionSettingsViewModel
import com.sona.android.app.navigation.SonaApp
import com.sona.android.application.recording.LiveRecordingState

class MainActivity : AppCompatActivity() {
    private val container: SonaAppContainer by lazy {
        (application as SonaApplication).container
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent {
            val bootstrapViewModel: SonaBootstrapViewModel = viewModel(
                factory = SonaBootstrapViewModel.factory(container.loadSonaBootstrap),
            )
            val recordingViewModel: RecordingViewModel = viewModel(
                factory = RecordingViewModel.factory(container::createLiveRecording),
            )
            val libraryViewModel: LibraryViewModel = viewModel(
                factory = LibraryViewModel.factory(
                    library = container.recordingLibrary,
                    transcribeRecordingWithCloud = container.transcribeRecordingWithCloud,
                ),
            )
            val appearanceSettingsViewModel: AppearanceSettingsViewModel = viewModel(
                factory = AppearanceSettingsViewModel.factory(container.appearanceSettings),
            )
            val credentialViewModel: CredentialSettingsViewModel = viewModel(
                factory = CredentialSettingsViewModel.factory(container.credentialSettings),
            )
            val cloudTranscriptionViewModel: CloudTranscriptionSettingsViewModel = viewModel(
                factory = CloudTranscriptionSettingsViewModel.factory(
                    container.batchCredentialSettings,
                ),
            )
            val recognitionSettingsViewModel: RecognitionSettingsViewModel = viewModel(
                factory = RecognitionSettingsViewModel.factory(container.recognitionSettings),
            )
            val bootstrapState by bootstrapViewModel.bootstrapState.collectAsStateWithLifecycle()
            val recordingState by recordingViewModel.state.collectAsStateWithLifecycle()
            val libraryState by libraryViewModel.state.collectAsStateWithLifecycle()
            val appearanceState by appearanceSettingsViewModel.state.collectAsStateWithLifecycle()
            val credentialState by credentialViewModel.uiState.collectAsStateWithLifecycle()
            val cloudTranscriptionState by cloudTranscriptionViewModel.uiState
                .collectAsStateWithLifecycle()
            val recognitionSettingsState by recognitionSettingsViewModel.uiState
                .collectAsStateWithLifecycle()
            LaunchedEffect(recordingState) {
                if (recordingState is LiveRecordingState.Completed) {
                    libraryViewModel.refresh()
                }
            }
            SonaApp(
                bootstrapState = bootstrapState,
                recordingState = recordingState,
                libraryState = libraryState,
                appearanceState = appearanceState,
                credentialState = credentialState,
                cloudTranscriptionState = cloudTranscriptionState,
                recognitionSettingsState = recognitionSettingsState,
                appLanguage = currentAppLanguage(),
                onAppLanguageChanged = ::setAppLanguage,
                onDynamicColorChanged = appearanceSettingsViewModel::setDynamicColorEnabled,
                onRetryBootstrap = bootstrapViewModel::refresh,
                onStartRecording = recordingViewModel::startRecording,
                onStopRecording = recordingViewModel::stopRecording,
                onAppBackground = recordingViewModel::stopForBackground,
                onRefreshLibrary = libraryViewModel::refresh,
                onLoadMoreLibrary = libraryViewModel::loadNextPage,
                onRetryLibrary = libraryViewModel::retryList,
                onLoadLibraryTranscript = libraryViewModel::loadTranscript,
                onTranscribeWithCloud = libraryViewModel::transcribeWithCloud,
                onCredentialInputChanged = credentialViewModel::onCredentialInputChanged,
                onSaveCredential = credentialViewModel::saveCredential,
                onClearCredential = credentialViewModel::clearCredential,
                onCloudProviderSelected = cloudTranscriptionViewModel::selectProvider,
                onCloudApiKeyInputChanged = cloudTranscriptionViewModel::onApiKeyInputChanged,
                onSaveCloudApiKey = cloudTranscriptionViewModel::saveApiKey,
                onClearCloudApiKey = cloudTranscriptionViewModel::clearApiKey,
                onRecognitionEngineSelected = recognitionSettingsViewModel::selectEngine,
                onImportLocalModel = recognitionSettingsViewModel::importLocalModel,
            )
        }
    }

    private fun currentAppLanguage(): AppLanguage = AppLanguage.fromLanguageTags(
        AppCompatDelegate.getApplicationLocales().toLanguageTags(),
    )

    private fun setAppLanguage(language: AppLanguage) {
        val locales = if (language == AppLanguage.SYSTEM) {
            LocaleListCompat.getEmptyLocaleList()
        } else {
            LocaleListCompat.forLanguageTags(language.languageTag)
        }
        AppCompatDelegate.setApplicationLocales(locales)
    }
}
