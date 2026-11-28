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
import com.sona.android.adapters.android.settings.AndroidApkInstaller
import com.sona.android.adapters.android.settings.AndroidAppUpdateAdapter
import com.sona.android.adapters.android.settings.AndroidFileLogger
import com.sona.android.adapters.android.settings.AndroidGeneralSettingsRepository
import com.sona.android.adapters.android.settings.AndroidLocalAsrDeviceCapabilities
import com.sona.android.adapters.android.settings.AndroidNetworkStatus
import com.sona.android.adapters.android.settings.AndroidRecognitionSettingsRepository
import com.sona.android.adapters.android.llm.AndroidLlmConfigurationRepository
import com.sona.android.adapters.android.sync.AndroidSyncSecretStore
import com.sona.android.adapters.android.system.AndroidMonotonicClock
import com.sona.android.adapters.android.system.UuidRecordingIdPort
import com.sona.android.adapters.uniffi.bootstrap.UniffiSonaBootstrapAdapter
import com.sona.android.adapters.uniffi.data.UniffiBackupAdapter
import com.sona.android.adapters.uniffi.data.UniffiTranscriptExportAdapter
import com.sona.android.adapters.uniffi.library.UniffiProjectWorkspaceAdapter
import com.sona.android.adapters.uniffi.library.UniffiTranscriptEditorAdapter
import com.sona.android.adapters.uniffi.logging.UniffiMobileLoggerAdapter
import com.sona.android.adapters.uniffi.recovery.UniffiRecoveryAdapter
import com.sona.android.adapters.uniffi.recording.UniffiOnlineBatchTranscriptionAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalAsrModelCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalAsrModelStorageAdapter
import com.sona.android.adapters.uniffi.recording.UniffiLocalBatchTranscriptionAdapter
import com.sona.android.adapters.uniffi.recording.UniffiRecordingHistoryAdapter
import com.sona.android.adapters.uniffi.llm.UniffiLlmAdapter
import com.sona.android.adapters.uniffi.llm.loadLlmProviders
import com.sona.android.adapters.uniffi.recording.UniffiStreamingProviderCatalogAdapter
import com.sona.android.adapters.uniffi.recording.UniffiStreamingTranscriptionAdapter
import com.sona.android.adapters.uniffi.settings.UniffiDiagnosticsAdapter
import com.sona.android.adapters.uniffi.settings.UniffiStorageUsageAdapter
import com.sona.android.adapters.uniffi.sync.UniffiSyncSecretStoreRegistrar
import com.sona.android.adapters.uniffi.sync.UniffiSyncAdapter
import com.sona.android.app.BuildConfig
import com.sona.android.app.feature.recording.AndroidRecordingServiceCommandLauncher
import com.sona.android.app.feature.recording.RecordingForegroundGateway
import com.sona.android.application.bootstrap.LoadSonaBootstrap
import com.sona.android.application.library.HistoryWorkspacePort
import com.sona.android.application.library.ProjectWorkspacePort
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
import com.sona.android.application.settings.ApkInstallerPort
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.AppLogPort
import com.sona.android.application.settings.AppUpdateDownloadPort
import com.sona.android.application.settings.AppearanceSettingsPort
import com.sona.android.application.settings.CheckForAppUpdate
import com.sona.android.application.settings.DiagnosticsPort
import com.sona.android.application.settings.GeneralSettingsPort
import com.sona.android.application.settings.NetworkStatusPort
import com.sona.android.application.settings.StorageUsagePort
import com.sona.android.application.sync.SyncSecretStorePort
import com.sona.android.application.llm.LlmConfigurationPort
import com.sona.android.application.llm.LlmTaskPort
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
    private val generalSettingsRepository = AndroidGeneralSettingsRepository.create(appContext, BuildConfig.DEBUG)
    private val fileLogger = AndroidFileLogger(
        context = appContext,
        initialLevel = if (BuildConfig.DEBUG) AppLogLevel.DEBUG else AppLogLevel.INFO,
        onLevelChanged = UniffiMobileLoggerAdapter::applyLevel,
    ).also { logger ->
        UniffiMobileLoggerAdapter.setSink(logger::append)
    }
    private val appUpdates = AndroidAppUpdateAdapter(appContext)
    private val apkInstallerAdapter = AndroidApkInstaller(appContext)
    private val networkStatusAdapter = AndroidNetworkStatus(appContext)
    private val localAsrDeviceCapabilities = AndroidLocalAsrDeviceCapabilities.create(appContext)
    private val localAsrModelCatalog = UniffiLocalAsrModelCatalogAdapter()
    private val localAsrModelStorage = UniffiLocalAsrModelStorageAdapter(
        File(appContext.filesDir, "models").absolutePath,
    )
    private val batchCredentialRepository = AndroidBatchCredentialRepository.create(appContext)
    private val llmConfigurationRepository = AndroidLlmConfigurationRepository.create(appContext, ::loadLlmProviders)
    private val recognitionSettingsRepository = AndroidRecognitionSettingsRepository.create(
        appContext,
        localAsrModelStorage,
        localAsrDeviceCapabilities,
        generalSettingsRepository,
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
    private val projects = UniffiProjectWorkspaceAdapter(appDataDir, syncScheduler::scheduleAfterLocalChange)
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
    val generalSettings: GeneralSettingsPort = generalSettingsRepository
    val appLogs: AppLogPort = fileLogger
    val checkForAppUpdate = CheckForAppUpdate(appUpdates)
    val appUpdateDownloader: AppUpdateDownloadPort = appUpdates
    val apkInstaller: ApkInstallerPort = apkInstallerAdapter
    val networkStatus: NetworkStatusPort = networkStatusAdapter
    val storageUsage: StorageUsagePort = UniffiStorageUsageAdapter(appDataDir, appContext.cacheDir)
    val diagnostics: DiagnosticsPort = UniffiDiagnosticsAdapter(
        appDataDir = appDataDir,
        logDirPath = fileLogger.logDirectoryPath,
        recognitionSettings = recognitionSettingsRepository,
        batchCredentials = batchCredentialRepository,
        deviceCapabilities = localAsrDeviceCapabilities,
        appLogs = fileLogger,
    )
    val recognitionSettings = recognitionSettingsRepository
    val recognitionModelCatalog = localAsrModelCatalog
    val recognitionDeviceCapabilities = localAsrDeviceCapabilities
    val batchCredentialSettings: BatchCredentialSettingsPort = batchCredentialRepository
    val llmConfiguration: LlmConfigurationPort = llmConfigurationRepository
    val llmTasks: LlmTaskPort = object : LlmTaskPort {
        private suspend fun adapter(): UniffiLlmAdapter {
            val config = llmConfigurationRepository.configuration.first()
            check(config.configured) { "LLM configuration is missing." }
            val key = llmConfigurationRepository.loadApiKey().orEmpty()
            if (config.requiresApiKey()) {
                check(key.isNotBlank()) { "LLM API key is missing." }
            }
            return UniffiLlmAdapter(config, key)
        }
        override suspend fun summarize(historyId: String, segments: List<com.sona.android.application.recording.TranscriptSegment>, template: com.sona.android.application.llm.LlmSummaryTemplate, observer: com.sona.android.application.llm.LlmTaskObserver) = adapter().summarize(historyId, segments, template, observer)
        override suspend fun translate(historyId: String, segments: List<com.sona.android.application.recording.TranscriptSegment>, targetLanguage: String, targetLanguageName: String?, observer: com.sona.android.application.llm.LlmTaskObserver) = adapter().translate(historyId, segments, targetLanguage, targetLanguageName, observer)
        override suspend fun polish(historyId: String, segments: List<com.sona.android.application.recording.TranscriptSegment>, observer: com.sona.android.application.llm.LlmTaskObserver) = adapter().polish(historyId, segments, observer)
    }
    val syncSecrets: SyncSecretStorePort = syncSecretStore
    val syncOperations: SyncPort = sync
    val syncWork: SyncSchedulerPort = syncScheduler
    val backups: BackupPort = backup
    val transcriptExports: TranscriptExportPort = transcriptExporter
    val fileTransfers: FileTransferPort = fileTransfer
    val recoveryJobs: RecoveryControllerPort = recoveryController
    val projectWorkspace: ProjectWorkspacePort = projects
    val recordingLibrary: HistoryWorkspacePort = history
    val llmHistory: com.sona.android.application.llm.LlmHistorySummaryPort = history
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
