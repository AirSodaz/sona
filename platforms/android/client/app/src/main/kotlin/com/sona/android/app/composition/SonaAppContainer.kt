package com.sona.android.app.composition

import android.annotation.SuppressLint
import android.content.Context
import com.sona.android.adapters.android.audio.AndroidMicrophoneCapturePort
import com.sona.android.adapters.android.audio.AndroidAudioImportJobAdapter
import com.sona.android.adapters.android.audio.AndroidAudioTranscoder
import com.sona.android.adapters.android.audio.AudioImportWorkerFactory
import com.sona.android.adapters.android.audio.FrameworkAudioRecordBackend
import com.sona.android.adapters.android.credential.AndroidBatchCredentialRepository
import com.sona.android.adapters.android.settings.AndroidAppearanceSettingsRepository
import com.sona.android.adapters.android.settings.AndroidLocalAsrDeviceCapabilities
import com.sona.android.adapters.android.settings.AndroidRecognitionSettingsRepository
import com.sona.android.adapters.android.sync.AndroidSyncSecretStore
import com.sona.android.adapters.android.system.AndroidMonotonicClock
import com.sona.android.adapters.android.system.UuidRecordingIdPort
import com.sona.android.adapters.uniffi.bootstrap.UniffiSonaBootstrapAdapter
import com.sona.android.adapters.uniffi.recording.UniffiOnlineBatchTranscriptionAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalAsrModelCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalBatchTranscriptionAdapter
import com.sona.android.adapters.uniffi.recording.UniffiRecordingHistoryAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingProviderCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingTranscriptionAdapter
import com.sona.android.adapters.uniffi.sync.UniffiSyncSecretStoreRegistrar
import com.sona.android.app.feature.recording.AndroidRecordingServiceCommandLauncher
import com.sona.android.app.feature.recording.RecordingForegroundGateway
import com.sona.android.application.bootstrap.LoadSonaBootstrap
import com.sona.android.application.library.RecordingLibraryPort
import com.sona.android.application.recording.BatchCredentialSettingsPort
import com.sona.android.application.recording.LiveRecordingController
import com.sona.android.application.recording.LiveRecordingCoordinator
import com.sona.android.application.recording.RunAudioImport
import com.sona.android.application.recording.ScheduleAudioImport
import com.sona.android.application.recording.ScheduleAudioRetranscription
import com.sona.android.application.recording.TranscribeRecordingWithCloud
import com.sona.android.application.settings.AppearanceSettingsPort
import com.sona.android.application.sync.SyncSecretStorePort
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first

class SonaAppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val processScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val appDataDir = appContext.filesDir.absolutePath
    private val bootstrapPort = UniffiSonaBootstrapAdapter()
    private val appearanceSettingsRepository = AndroidAppearanceSettingsRepository.create(appContext)
    private val localAsrDeviceCapabilities = AndroidLocalAsrDeviceCapabilities.create(appContext)
    private val localAsrModelCatalog = UniffiLocalAsrModelCatalogAdapter()
    private val batchCredentialRepository = AndroidBatchCredentialRepository.create(appContext)
    private val recognitionSettingsRepository = AndroidRecognitionSettingsRepository.create(
        appContext,
        localAsrDeviceCapabilities,
        legacyBatchProvider = {
            batchCredentialRepository.configuration.first().selectedProvider
        },
    )
    private val syncSecretStore = AndroidSyncSecretStore.create(appContext)
    private val syncSecretStoreRegistration = UniffiSyncSecretStoreRegistrar().apply {
        register(appDataDir, syncSecretStore)
    }
    private val providerCatalog = UniffiStreamingProviderCatalogAdapter()
    private val microphoneCapture = AndroidMicrophoneCapturePort(
        backendFactory = ::createAudioBackend,
        readerDispatcher = Dispatchers.IO,
    )
    private val streamingTranscription = UniffiStreamingTranscriptionAdapter()
    private val batchTranscription = UniffiOnlineBatchTranscriptionAdapter()
    private val localBatchTranscription = UniffiLocalBatchTranscriptionAdapter()
    private val history = UniffiRecordingHistoryAdapter(appDataDir)
    private val audioImportJobs = AndroidAudioImportJobAdapter.create(appContext)
    private val audioTranscoder = AndroidAudioTranscoder.create(appContext)
    private val monotonicClock = AndroidMonotonicClock()
    private val recordingIds = UuidRecordingIdPort()

    val loadSonaBootstrap = LoadSonaBootstrap(bootstrapPort)
    val appearanceSettings: AppearanceSettingsPort = appearanceSettingsRepository
    val recognitionSettings = recognitionSettingsRepository
    val recognitionModelCatalog = localAsrModelCatalog
    val recognitionDeviceCapabilities = localAsrDeviceCapabilities
    val batchCredentialSettings: BatchCredentialSettingsPort = batchCredentialRepository
    val syncSecrets: SyncSecretStorePort = syncSecretStore
    val recordingLibrary: RecordingLibraryPort = history
    val transcribeRecordingWithCloud = TranscribeRecordingWithCloud(
        credentials = batchCredentialRepository,
        transcription = batchTranscription,
        history = history,
    )
    private val runAudioImport = RunAudioImport(
        transcoder = audioTranscoder,
        recognitionSettings = recognitionSettingsRepository,
        batchCredentials = batchCredentialRepository,
        localTranscription = localBatchTranscription,
        onlineTranscription = batchTranscription,
        history = history,
    )
    val scheduleAudioImport = ScheduleAudioImport(
        recognitionSettings = recognitionSettingsRepository,
        batchCredentials = batchCredentialRepository,
        recordingIds = recordingIds,
        jobs = audioImportJobs,
    )
    val scheduleAudioRetranscription = ScheduleAudioRetranscription(
        recognitionSettings = recognitionSettingsRepository,
        batchCredentials = batchCredentialRepository,
        recordingIds = recordingIds,
        jobs = audioImportJobs,
    )
    val audioImportJobState = audioImportJobs.state
    val audioImportJobsController = audioImportJobs
    val audioImportWorkerFactory = AudioImportWorkerFactory(runAudioImport)
    internal val recordingGateway = RecordingForegroundGateway(
        launcher = AndroidRecordingServiceCommandLauncher(appContext),
        scope = processScope,
    )

    fun createLiveRecording(scope: CoroutineScope): LiveRecordingController =
        LiveRecordingCoordinator(
            credentialResolver = batchCredentialRepository,
            providerCatalog = providerCatalog,
            microphoneCapture = microphoneCapture,
            streamingTranscription = streamingTranscription,
            history = history,
            monotonicClock = monotonicClock,
            recordingIds = recordingIds,
            scope = scope,
            recognitionSettings = { recognitionSettingsRepository.load() },
        )

    @SuppressLint("MissingPermission")
    private fun createAudioBackend() = FrameworkAudioRecordBackend.create(appContext)
}
