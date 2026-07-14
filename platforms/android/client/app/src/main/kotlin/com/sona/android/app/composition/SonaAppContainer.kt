package com.sona.android.app.composition

import android.annotation.SuppressLint
import android.content.Context
import com.sona.android.adapters.android.audio.AndroidMicrophoneCapturePort
import com.sona.android.adapters.android.audio.FrameworkAudioRecordBackend
import com.sona.android.adapters.android.credential.AndroidStreamingCredentialRepository
import com.sona.android.adapters.android.system.AndroidMonotonicClock
import com.sona.android.adapters.android.system.UuidRecordingIdPort
import com.sona.android.adapters.uniffi.bootstrap.UniffiSonaBootstrapAdapter
import com.sona.android.adapters.uniffi.recording.UniffiRecordingHistoryAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingProviderCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingTranscriptionAdapter
import com.sona.android.application.bootstrap.LoadSonaBootstrap
import com.sona.android.application.recording.LiveRecordingCoordinator
import com.sona.android.application.recording.LiveRecordingUseCase
import com.sona.android.application.recording.StreamingCredentialSettingsPort
import java.io.File
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers

class SonaAppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val appDataDir = createAppDataDir(appContext)
    private val bootstrapPort = UniffiSonaBootstrapAdapter()
    private val credentialRepository = AndroidStreamingCredentialRepository.create(appContext)
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
    val credentialSettings: StreamingCredentialSettingsPort = credentialRepository

    fun createLiveRecording(scope: CoroutineScope): LiveRecordingUseCase =
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

    private companion object {
        fun createAppDataDir(context: Context): String {
            val directory = File(context.filesDir, "sona")
            check(directory.isDirectory || directory.mkdirs()) {
                "Unable to create the Sona app data directory."
            }
            return directory.absolutePath
        }
    }
}
