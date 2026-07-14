package com.sona.android.app

import android.Manifest
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.lifecycle.compose.LifecycleResumeEffect
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.core.os.LocaleListCompat
import com.sona.android.app.composition.SonaAppContainer
import com.sona.android.app.feature.bootstrap.SonaBootstrapViewModel
import com.sona.android.app.feature.recording.ForegroundRecordingLifecycleEffect
import com.sona.android.app.feature.recording.MicrophonePermissionDecision
import com.sona.android.app.feature.recording.MicrophonePermissionPolicy
import com.sona.android.app.feature.recording.RecordingViewModel
import com.sona.android.app.feature.settings.AppLanguage
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
            val credentialViewModel: CredentialSettingsViewModel = viewModel(
                factory = CredentialSettingsViewModel.factory(container.credentialSettings),
            )
            val bootstrapState by bootstrapViewModel.bootstrapState.collectAsStateWithLifecycle()
            val recordingState by recordingViewModel.state.collectAsStateWithLifecycle()
            val credentialState by credentialViewModel.state.collectAsStateWithLifecycle()
            var microphonePermissionGranted by remember {
                mutableStateOf(hasMicrophonePermission())
            }
            var hasRequestedMicrophonePermission by rememberSaveable { mutableStateOf(false) }
            var showPermissionRationale by rememberSaveable { mutableStateOf(false) }
            var showAppSettingsPrompt by rememberSaveable { mutableStateOf(false) }
            LifecycleResumeEffect(Unit) {
                microphonePermissionGranted = hasMicrophonePermission()
                onPauseOrDispose {}
            }
            val permissionLauncher = rememberLauncherForActivityResult(
                ActivityResultContracts.RequestPermission(),
            ) { granted ->
                microphonePermissionGranted = granted
                if (granted) {
                    recordingViewModel.onRecordAction()
                }
            }
            ForegroundRecordingLifecycleEffect(
                onAppBackground = recordingViewModel::stopForBackground,
            )
            SonaApp(
                bootstrapState = bootstrapState,
                recordingState = recordingState,
                credentialState = credentialState,
                appLanguage = currentAppLanguage(),
                microphonePermissionGranted = microphonePermissionGranted,
                onRecordAction = {
                    if (!recordingViewModel.actionRequiresMicrophonePermission()) {
                        recordingViewModel.onRecordAction()
                        return@SonaApp
                    }
                    when (
                        MicrophonePermissionPolicy.decide(
                            isGranted = microphonePermissionGranted,
                            hasRequestedBefore = hasRequestedMicrophonePermission,
                            shouldShowRationale = shouldShowRequestPermissionRationale(
                                Manifest.permission.RECORD_AUDIO,
                            ),
                        )
                    ) {
                        MicrophonePermissionDecision.START_RECORDING ->
                            recordingViewModel.onRecordAction()
                        MicrophonePermissionDecision.REQUEST_PERMISSION -> {
                            hasRequestedMicrophonePermission = true
                            permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                        }
                        MicrophonePermissionDecision.SHOW_RATIONALE ->
                            showPermissionRationale = true
                        MicrophonePermissionDecision.OPEN_APP_SETTINGS ->
                            showAppSettingsPrompt = true
                    }
                },
                onSaveCredential = credentialViewModel::save,
                onClearCredential = credentialViewModel::clear,
                onAppLanguageChanged = ::setAppLanguage,
                onRetryBootstrap = bootstrapViewModel::refresh,
            )
            if (showPermissionRationale) {
                AlertDialog(
                    onDismissRequest = { showPermissionRationale = false },
                    title = { Text(getString(R.string.microphone_permission_title)) },
                    text = { Text(getString(R.string.microphone_permission_rationale)) },
                    confirmButton = {
                        TextButton(
                            onClick = {
                                showPermissionRationale = false
                                hasRequestedMicrophonePermission = true
                                permissionLauncher.launch(Manifest.permission.RECORD_AUDIO)
                            },
                        ) {
                            Text(getString(R.string.action_continue))
                        }
                    },
                    dismissButton = {
                        TextButton(onClick = { showPermissionRationale = false }) {
                            Text(getString(R.string.action_cancel))
                        }
                    },
                )
            }
            if (showAppSettingsPrompt) {
                AlertDialog(
                    onDismissRequest = { showAppSettingsPrompt = false },
                    title = { Text(getString(R.string.microphone_permission_title)) },
                    text = { Text(getString(R.string.microphone_permission_settings)) },
                    confirmButton = {
                        TextButton(
                            onClick = {
                                showAppSettingsPrompt = false
                                openAppSettings()
                            },
                        ) {
                            Text(getString(R.string.action_open_settings))
                        }
                    },
                    dismissButton = {
                        TextButton(onClick = { showAppSettingsPrompt = false }) {
                            Text(getString(R.string.action_cancel))
                        }
                    },
                )
            }
        }
    }

    private fun hasMicrophonePermission(): Boolean =
        checkSelfPermission(Manifest.permission.RECORD_AUDIO) == PackageManager.PERMISSION_GRANTED

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

    private fun openAppSettings() {
        startActivity(
            Intent(
                Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                Uri.fromParts("package", packageName, null),
            ),
        )
    }
}
