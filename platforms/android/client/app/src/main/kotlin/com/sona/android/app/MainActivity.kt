package com.sona.android.app

import android.os.Bundle
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.runtime.getValue
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.core.os.LocaleListCompat
import com.sona.android.app.composition.SonaAppContainer
import com.sona.android.app.feature.bootstrap.SonaBootstrapViewModel
import com.sona.android.app.feature.recording.RecordingViewModel
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.AppearanceSettingsViewModel
import com.sona.android.app.feature.settings.CredentialSettingsViewModel
import com.sona.android.app.navigation.SonaApp

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
            val appearanceSettingsViewModel: AppearanceSettingsViewModel = viewModel(
                factory = AppearanceSettingsViewModel.factory(container.appearanceSettings),
            )
            val credentialViewModel: CredentialSettingsViewModel = viewModel(
                factory = CredentialSettingsViewModel.factory(container.credentialSettings),
            )
            val bootstrapState by bootstrapViewModel.bootstrapState.collectAsStateWithLifecycle()
            val recordingState by recordingViewModel.state.collectAsStateWithLifecycle()
            val appearanceState by appearanceSettingsViewModel.state.collectAsStateWithLifecycle()
            val credentialState by credentialViewModel.uiState.collectAsStateWithLifecycle()
            SonaApp(
                bootstrapState = bootstrapState,
                recordingState = recordingState,
                appearanceState = appearanceState,
                credentialState = credentialState,
                appLanguage = currentAppLanguage(),
                onAppLanguageChanged = ::setAppLanguage,
                onDynamicColorChanged = appearanceSettingsViewModel::setDynamicColorEnabled,
                onRetryBootstrap = bootstrapViewModel::refresh,
                onStartRecording = recordingViewModel::startRecording,
                onStopRecording = recordingViewModel::stopRecording,
                onAppBackground = recordingViewModel::stopForBackground,
                onCredentialInputChanged = credentialViewModel::onCredentialInputChanged,
                onSaveCredential = credentialViewModel::saveCredential,
                onClearCredential = credentialViewModel::clearCredential,
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
