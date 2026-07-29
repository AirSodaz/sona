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
import com.sona.android.app.feature.settings.RecognitionSettingsViewModel
import com.sona.android.app.feature.settings.SyncSettingsViewModel
import com.sona.android.app.feature.settings.DataRecoveryViewModel
import com.sona.android.app.navigation.SonaApp
import com.sona.android.application.data.DataTransferBlocker
import com.sona.android.application.recording.AudioImportJobState
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.sync.SyncLifecycleState

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
                factory = RecordingViewModel.factory(container.recordingGateway),
            )
            val libraryViewModel: LibraryViewModel = viewModel(
                factory = LibraryViewModel.factory(
                    library = container.recordingLibrary,
                    transcribeRecordingWithCloud = container.transcribeRecordingWithCloud,
                    scheduleAudioImport = container.scheduleAudioImport,
                    scheduleAudioRetranscription = container.scheduleAudioRetranscription,
                    audioImportJobs = container.audioImportJobsController,
                    tags = container.tagWorkspace,
                    exporter = container.transcriptExports,
                    files = container.fileTransfers,
                ),
            )
            val appearanceSettingsViewModel: AppearanceSettingsViewModel = viewModel(
                factory = AppearanceSettingsViewModel.factory(container.appearanceSettings),
            )
            val cloudTranscriptionViewModel: CloudTranscriptionSettingsViewModel = viewModel(
                factory = CloudTranscriptionSettingsViewModel.factory(
                    container.batchCredentialSettings,
                ),
            )
            val recognitionSettingsViewModel: RecognitionSettingsViewModel = viewModel(
                factory = RecognitionSettingsViewModel.factory(
                    container.recognitionSettings,
                    container.recognitionModelCatalog,
                    container.recognitionDeviceCapabilities,
                ),
            )
            val syncSettingsViewModel: SyncSettingsViewModel = viewModel(
                factory = SyncSettingsViewModel.factory(
                    container.syncOperations,
                    container.syncWork,
                    container.fileTransfers,
                ),
            )
            val dataRecoveryViewModel: DataRecoveryViewModel = viewModel(
                factory = DataRecoveryViewModel.factory(
                    container.backups,
                    container.fileTransfers,
                    container.recoveryJobs,
                    container.syncWork,
                    BuildConfig.VERSION_NAME,
                    container::rebindAfterBackupRestore,
                ),
            )
            val bootstrapState by bootstrapViewModel.bootstrapState.collectAsStateWithLifecycle()
            val recordingState by recordingViewModel.state.collectAsStateWithLifecycle()
            val libraryState by libraryViewModel.state.collectAsStateWithLifecycle()
            val appearanceState by appearanceSettingsViewModel.state.collectAsStateWithLifecycle()
            val cloudTranscriptionState by cloudTranscriptionViewModel.uiState
                .collectAsStateWithLifecycle()
            val recognitionSettingsState by recognitionSettingsViewModel.uiState
                .collectAsStateWithLifecycle()
            val syncState by syncSettingsViewModel.state.collectAsStateWithLifecycle()
            val dataRecoveryState by dataRecoveryViewModel.state.collectAsStateWithLifecycle()
            LaunchedEffect(Unit) {
                syncSettingsViewModel.refresh()
                dataRecoveryViewModel.refreshRecovery()
            }
            LaunchedEffect(recordingState, libraryState.audioImport, syncState.status, dataRecoveryState.recovery) {
                val blockers = buildSet {
                    if (recordingState is LiveRecordingState.Preparing ||
                        recordingState is LiveRecordingState.Recording ||
                        recordingState is LiveRecordingState.Stopping
                    ) add(DataTransferBlocker.LIVE_RECORDING)
                    if (libraryState.audioImport is AudioImportJobState.Running) add(DataTransferBlocker.AUDIO_IMPORT)
                    if (syncState.status.state == SyncLifecycleState.SYNCING) add(DataTransferBlocker.SYNC)
                    if (dataRecoveryState.recovery.items.any { it.resolution == RecoveryResolution.PENDING }) {
                        add(DataTransferBlocker.RECOVERY)
                    }
                }
                dataRecoveryViewModel.setBlockers(blockers)
            }
            LaunchedEffect(dataRecoveryState.restoreGeneration) {
                if (dataRecoveryState.restoreGeneration > 0) {
                    bootstrapViewModel.refresh()
                    libraryViewModel.resetAfterRestore()
                    syncSettingsViewModel.refresh()
                    recognitionSettingsViewModel.refreshCatalog()
                }
            }
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
                cloudTranscriptionState = cloudTranscriptionState,
                recognitionSettingsState = recognitionSettingsState,
                syncState = syncState,
                dataRecoveryState = dataRecoveryState,
                appLanguage = currentAppLanguage(),
                onAppLanguageChanged = ::setAppLanguage,
                onDynamicColorChanged = appearanceSettingsViewModel::setDynamicColorEnabled,
                onRetryBootstrap = bootstrapViewModel::refresh,
                onStartRecording = recordingViewModel::startRecording,
                onStopRecording = recordingViewModel::stopRecording,
                onRefreshLibrary = libraryViewModel::refresh,
                onLoadMoreLibrary = libraryViewModel::loadNextPage,
                onRetryLibrary = libraryViewModel::retryList,
                onLoadLibraryTranscript = libraryViewModel::loadTranscript,
                onLibrarySearchChanged = libraryViewModel::setSearchQuery,
                onLibraryScopeChanged = libraryViewModel::setScope,
                onLibraryFilterChanged = libraryViewModel::setFilter,
                onLibraryDateChanged = libraryViewModel::setDateFilter,
                onLibrarySortChanged = libraryViewModel::setSortOrder,
                onToggleLibrarySelection = libraryViewModel::toggleSelection,
                onClearLibrarySelection = libraryViewModel::clearSelection,
                onTrashLibrarySelection = libraryViewModel::trashSelected,
                onRestoreLibrarySelection = libraryViewModel::restoreSelected,
                onPurgeLibrarySelection = libraryViewModel::purgeSelected,
                onAddTagToLibrarySelection = libraryViewModel::addTagToSelected,
                onRemoveTagFromLibrarySelection = libraryViewModel::removeTagFromSelected,
                onUpdateHistoryTitle = libraryViewModel::updateTitle,
                onUpdateHistoryTags = libraryViewModel::updateTags,
                onCreateHistoryTag = libraryViewModel::createTag,
                onLoadTranscriptSnapshot = libraryViewModel::loadSnapshot,
                onCloseTranscriptSnapshot = libraryViewModel::closeSnapshot,
                onExportTranscript = libraryViewModel::exportTranscript,
                onTranscribeWithCloud = libraryViewModel::transcribeWithCloud,
                onImportAudio = libraryViewModel::importAudio,
                onCancelAudioImport = libraryViewModel::cancelAudioImport,
                onTranscribeWithCurrentEngine = libraryViewModel::transcribeWithCurrentEngine,
                onCloudProviderSelected = cloudTranscriptionViewModel::selectProvider,
                onCloudApiKeyInputChanged = cloudTranscriptionViewModel::onApiKeyInputChanged,
                onSaveCloudApiKey = cloudTranscriptionViewModel::saveApiKey,
                onClearCloudApiKey = cloudTranscriptionViewModel::clearApiKey,
                onSelectModel = recognitionSettingsViewModel::selectModel,
                onDownloadLocalModel = recognitionSettingsViewModel::downloadLocalModel,
                onValidateLocalModel = recognitionSettingsViewModel::validateLocalModel,
                onDeleteLocalModel = recognitionSettingsViewModel::deleteLocalModel,
                onRefreshRecognitionCatalog = recognitionSettingsViewModel::refreshCatalog,
                onRefreshSync = syncSettingsViewModel::refresh,
                onTestSyncProvider = syncSettingsViewModel::testProvider,
                onCreateSync = syncSettingsViewModel::create,
                onPreviewSyncJoin = syncSettingsViewModel::previewJoin,
                onJoinSync = syncSettingsViewModel::join,
                onUnlockSync = syncSettingsViewModel::unlock,
                onUnlockSyncWithRecovery = syncSettingsViewModel::unlockWithRecovery,
                onRunSync = syncSettingsViewModel::runNow,
                onPauseSync = syncSettingsViewModel::setPaused,
                onLockSync = syncSettingsViewModel::lock,
                onDisconnectSync = syncSettingsViewModel::disconnect,
                onGenerateSyncRecoveryKey = syncSettingsViewModel::generateRecoveryKey,
                onExportSyncRecoveryKey = syncSettingsViewModel::exportRecoveryKey,
                onConsumeSyncRecoveryKey = syncSettingsViewModel::consumeRecoveryKey,
                onResolveSyncConflict = syncSettingsViewModel::resolveConflict,
                onLoadSyncConflict = syncSettingsViewModel::loadConflict,
                onChangeSyncPreset = syncSettingsViewModel::changePreset,
                onChangeSyncPassword = syncSettingsViewModel::changePassword,
                onExportBackup = dataRecoveryViewModel::exportBackup,
                onInspectBackup = dataRecoveryViewModel::inspectBackup,
                onConfirmBackupImport = dataRecoveryViewModel::confirmImport,
                onCancelBackupImport = dataRecoveryViewModel::cancelPreparedBackup,
                onRefreshRecovery = dataRecoveryViewModel::refreshRecovery,
                onResumeRecovery = dataRecoveryViewModel::resume,
                onResumeAllRecovery = dataRecoveryViewModel::resumeAll,
                onDiscardRecovery = dataRecoveryViewModel::discard,
                onClearResolvedRecovery = dataRecoveryViewModel::clearResolved,
            )
        }
    }

    override fun onResume() {
        super.onResume()
        container.syncWork.scheduleImmediate()
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
