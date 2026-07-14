package com.sona.android.app.composition

import android.annotation.SuppressLint
import android.content.Context
import com.sona.android.adapters.android.audio.AndroidMicrophoneCapturePort
import com.sona.android.adapters.android.audio.FrameworkAudioRecordBackend
import com.sona.android.adapters.android.credential.AndroidStreamingCredentialRepository
import com.sona.android.adapters.android.settings.AndroidAppearanceSettingsRepository
import com.sona.android.adapters.android.system.AndroidMonotonicClock
import com.sona.android.adapters.android.system.UuidRecordingIdPort
import com.sona.android.adapters.uniffi.bootstrap.UniffiSonaBootstrapAdapter
import com.sona.android.adapters.uniffi.recording.UniffiRecordingHistoryAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingProviderCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingTranscriptionAdapter
import com.sona.android.application.bootstrap.LoadSonaBootstrap
import com.sona.android.application.recording.LiveRecordingController
import com.sona.android.application.recording.LiveRecordingCoordinator
import com.sona.android.application.recording.StreamingCredentialSettingsPort
import com.sona.android.application.settings.AppearanceSettingsPort
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers

class SonaAppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val appDataDir = appContext.filesDir.absolutePath
    private val bootstrapPort = UniffiSonaBootstrapAdapter()
    private val credentialRepository = AndroidStreamingCredentialRepository.create(appContext)
    private val appearanceSettingsRepository = AndroidAppearanceSettingsRepository.create(appContext)
    private val providerCatalog = UniffiStreamingProviderCatalogAdapter()
    private val microphoneCapture = AndroidMicrophoneCapturePort(
        backendFactory = ::createAudioBackend,
        readerDispatcher = Dispatchers.IO,
    )
    private val streamingTranscription = UniffiStreamingTranscriptionAdapter()
    private val history = UniffiRecordingHistoryAdapter(appDataDir)
    private val monotonicClock = AndroidMonotonicClock()
    private val recordingIds = UuidRecordingIdPort()

    val loadSonaBootstrap = LoadSonaBootstrap(bootstrapPort)
    val appearanceSettings: AppearanceSettingsPort = appearanceSettingsRepository
    val credentialSettings: StreamingCredentialSettingsPort = credentialRepository

    fun createLiveRecording(scope: CoroutineScope): LiveRecordingController =
        LiveRecordingCoordinator(
            credentialResolver = credentialRepository,
            providerCatalog = providerCatalog,
            microphoneCapture = microphoneCapture,
            streamingTranscription = streamingTranscription,
            history = history,
            monotonicClock = monotonicClock,
            recordingIds = recordingIds,
            scope = scope,
        )

    @SuppressLint("MissingPermission")
    private fun createAudioBackend() = FrameworkAudioRecordBackend.create(appContext)
}
