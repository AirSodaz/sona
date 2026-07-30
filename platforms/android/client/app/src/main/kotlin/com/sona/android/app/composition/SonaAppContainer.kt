package com.sona.android.app.composition

import android.annotation.SuppressLint
import android.content.Context
import com.sona.android.adapters.android.audio.AndroidMicrophoneCapturePort
import com.sona.android.adapters.android.audio.AndroidAudioImportJobAdapter
import com.sona.android.adapters.android.audio.AndroidAudioTranscoder
import com.sona.android.adapters.android.audio.AndroidAudioPlaybackAdapter
import com.sona.android.adapters.android.audio.AudioImportWorkerFactory
import com.sona.android.adapters.android.audio.FrameworkAudioRecordBackend
import com.sona.android.adapters.android.data.AndroidSafFileTransferAdapter
import com.sona.android.adapters.android.recovery.AndroidRecoveryController
import com.sona.android.adapters.android.recovery.AndroidTranscriptEditRecoveryAdapter
import com.sona.android.adapters.android.recovery.RecoveryCoordinator
import com.sona.android.adapters.android.sync.AndroidSyncScheduler
import com.sona.android.adapters.android.sync.SyncWorkerFactory
import com.sona.android.adapters.android.credential.AndroidBatchCredentialRepository
import com.sona.android.adapters.android.settings.AndroidAppearanceSettingsRepository
import com.sona.android.adapters.android.settings.AndroidLocalAsrDeviceCapabilities
import com.sona.android.adapters.android.settings.AndroidRecognitionSettingsRepository
import com.sona.android.adapters.android.sync.AndroidSyncSecretStore
import com.sona.android.adapters.android.system.AndroidMonotonicClock
import com.sona.android.adapters.android.system.UuidRecordingIdPort
import com.sona.android.adapters.uniffi.bootstrap.UniffiSonaBootstrapAdapter
import com.sona.android.adapters.uniffi.data.UniffiBackupAdapter
import com.sona.android.adapters.uniffi.data.UniffiTranscriptExportAdapter
import com.sona.android.adapters.uniffi.library.UniffiTagWorkspaceAdapter
import com.sona.android.adapters.uniffi.library.UniffiTranscriptEditorAdapter
import com.sona.android.adapters.uniffi.recovery.UniffiRecoveryAdapter
import com.sona.android.adapters.uniffi.recording.UniffiOnlineBatchTranscriptionAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalAsrModelCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalAsrModelStorageAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalBatchTranscriptionAdapter
import com.sona.android.adapters.uniffi.recording.UniffiRecordingHistoryAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingProviderCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingTranscriptionAdapter
import com.sona.android.adapters.uniffi.sync.UniffiSyncSecretStoreRegistrar
import com.sona.android.adapters.uniffi.sync.UniffiSyncAdapter
import com.sona.android.app.feature.recording.AndroidRecordingServiceCommandLauncher
import com.sona.android.app.feature.recording.RecordingForegroundGateway
import com.sona.android.application.bootstrap.LoadSonaBootstrap
import com.sona.android.application.library.HistoryWorkspacePort
import com.sona.android.application.library.TagWorkspacePort
import com.sona.android.application.library.HistoryMediaSourcePort
import com.sona.android.application.library.TranscriptEditorPort
import com.sona.android.application.media.AudioPlaybackPort
import com.sona.android.application.data.BackupPort
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.data.TranscriptExportPort
import com.sona.android.application.recovery.RecoveryControllerPort
import com.sona.android.application.recovery.TranscriptEditRecoveryPort
import com.sona.android.application.recovery.RecoveryUnavailableReason
import com.sona.android.application.recording.AudioImportEngine
import com.sona.android.application.sync.SyncPort
import com.sona.android.application.sync.SyncSchedulerPort
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
import java.io.File

class SonaAppContainer(context: Context) {
    private val appContext = context.applicationContext
    private val processScope = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
    private val appDataDir = appContext.filesDir.absolutePath
    private val bootstrapPort = UniffiSonaBootstrapAdapter()
    private val appearanceSettingsRepository = AndroidAppearanceSettingsRepository.create(appContext)
    private val localAsrDeviceCapabilities = AndroidLocalAsrDeviceCapabilities.create(appContext)
    private val localAsrModelCatalog = UniffiLocalAsrModelCatalogAdapter()
    private val localAsrModelStorage = UniffiLocalAsrModelStorageAdapter(
        File(appContext.filesDir, "models").absolutePath,
    )
    private val batchCredentialRepository = AndroidBatchCredentialRepository.create(appContext)
    private val recognitionSettingsRepository = AndroidRecognitionSettingsRepository.create(
        appContext,
        localAsrModelStorage,
        localAsrDeviceCapabilities,
    )
    private val syncSecretStore = AndroidSyncSecretStore.create(appContext)
    private val syncSecretStoreRegistration = UniffiSyncSecretStoreRegistrar().apply {
        register(appDataDir, syncSecretStore)
    }
    private val sync = UniffiSyncAdapter(appDataDir)
    private val syncScheduler = AndroidSyncScheduler.create(appContext)
    private val recovery = UniffiRecoveryAdapter(appDataDir)
    private val recoveryCoordinator = RecoveryCoordinator(recovery)
    private val backup = UniffiBackupAdapter(appDataDir)
    private val transcriptExporter = UniffiTranscriptExportAdapter()
    private val fileTransfer = AndroidSafFileTransferAdapter.create(appContext)
    private val tags = UniffiTagWorkspaceAdapter(appDataDir, syncScheduler::scheduleAfterLocalChange)
    private val providerCatalog = UniffiStreamingProviderCatalogAdapter()
    private val microphoneCapture = AndroidMicrophoneCapturePort(
        backendFactory = ::createAudioBackend,
        readerDispatcher = Dispatchers.IO,
    )
    private val streamingTranscription = UniffiStreamingTranscriptionAdapter()
    private val batchTranscription = UniffiOnlineBatchTranscriptionAdapter()
    private val localBatchTranscription = UniffiLocalBatchTranscriptionAdapter()
    private val history = UniffiRecordingHistoryAdapter(appDataDir, syncScheduler::scheduleAfterLocalChange)
    private val transcriptEditorAdapter = UniffiTranscriptEditorAdapter(
        appDataDir,
        syncScheduler::scheduleAfterLocalChange,
    )
    private val transcriptEditRecoveryAdapter = AndroidTranscriptEditRecoveryAdapter(recoveryCoordinator)
    private val audioPlaybackAdapter = AndroidAudioPlaybackAdapter.create(appContext)
    private val audioImportJobs = AndroidAudioImportJobAdapter.create(appContext, recoveryCoordinator)
    private val recoveryController = AndroidRecoveryController(
        appContext,
        recoveryCoordinator,
        audioImportJobs,
        unavailableReason = { job ->
        when (val engine = job.engine) {
            is AudioImportEngine.Local -> if (
                localAsrModelStorage.listInstalledModels().none { it.id == engine.modelId }
            ) RecoveryUnavailableReason.MODEL_MISSING else null
            is AudioImportEngine.Online -> if (
                batchCredentialRepository.load(engine.provider) == null
            ) RecoveryUnavailableReason.CREDENTIAL_MISSING else null
        }
        },
        transcriptHistory = history,
    )
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
    val syncOperations: SyncPort = sync
    val syncWork: SyncSchedulerPort = syncScheduler
    val backups: BackupPort = backup
    val transcriptExports: TranscriptExportPort = transcriptExporter
    val fileTransfers: FileTransferPort = fileTransfer
    val recoveryJobs: RecoveryControllerPort = recoveryController
    val tagWorkspace: TagWorkspacePort = tags
    val recordingLibrary: HistoryWorkspacePort = history
    val transcriptEditor: TranscriptEditorPort = transcriptEditorAdapter
    val historyMediaSources: HistoryMediaSourcePort = transcriptEditorAdapter
    val audioPlayback: AudioPlaybackPort = audioPlaybackAdapter
    val transcriptEditRecovery: TranscriptEditRecoveryPort = transcriptEditRecoveryAdapter
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
    internal val workerFactory = SonaWorkerFactory(
        AudioImportWorkerFactory(runAudioImport, recoveryCoordinator),
        SyncWorkerFactory(sync),
    )
    internal val recordingGateway = RecordingForegroundGateway(
        launcher = AndroidRecordingServiceCommandLauncher(appContext),
        scope = processScope,
    )

    init {
        syncScheduler.schedulePeriodic()
    }

    fun rebindAfterBackupRestore() {
        syncSecretStoreRegistration.register(appDataDir, syncSecretStore)
        syncScheduler.schedulePeriodic()
    }

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
