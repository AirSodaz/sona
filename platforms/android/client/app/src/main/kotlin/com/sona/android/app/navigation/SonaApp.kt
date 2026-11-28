package com.sona.android.app.navigation

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.animation.AnimatedVisibility
import androidx.compose.animation.fadeIn
import androidx.compose.animation.fadeOut
import androidx.compose.animation.slideInVertically
import androidx.compose.animation.slideOutVertically
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.windowInsetsPadding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.dp
import androidx.navigation.NavBackStackEntry
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.app.feature.library.LibraryDetailScreen
import com.sona.android.app.feature.library.LibraryScreen
import com.sona.android.app.feature.library.LibraryUiState
import com.sona.android.app.feature.recording.RecordMiniBar
import com.sona.android.app.feature.recording.RecordScreen
import com.sona.android.app.feature.recording.RecordingCompletedToast
import com.sona.android.app.feature.settings.AboutSettingsUiState
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.AppearanceSettingsUiState
import com.sona.android.app.feature.settings.CloudTranscriptionSettingsUiState
import com.sona.android.app.feature.settings.DataRecoveryUiState
import com.sona.android.app.feature.settings.LlmSettingsUiState
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.app.feature.settings.SettingsScreen
import com.sona.android.app.feature.settings.SettingsSection
import com.sona.android.app.feature.settings.SyncSettingsUiState
import com.sona.android.app.ui.theme.SonaTheme
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.HistorySortOrder
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySource
import com.sona.android.application.sync.DiscoveredVaultSummary
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncPairingPayload
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.WebDavSyncProvider
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import kotlinx.coroutines.delay

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
internal fun SonaApp(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    libraryState: LibraryUiState,
    appearanceState: AppearanceSettingsUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    syncState: SyncSettingsUiState,
    dataRecoveryState: DataRecoveryUiState,
    llmState: LlmSettingsUiState = LlmSettingsUiState(),
    aboutState: AboutSettingsUiState,
    appLanguage: AppLanguage,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onThemeModeChanged: (ThemeMode) -> Unit = {},
    onPureBlackDarkChanged: (Boolean) -> Unit = {},
    onSelectSystemDynamicColor: () -> Unit = {},
    onSelectPalettePreset: (ColorPalettePreset) -> Unit = {},
    onCustomSeedColorChanged: (Int) -> Unit = {},
    onPaletteStyleChanged: (PaletteStyle) -> Unit = {},
    onContrastLevelChanged: (ContrastLevel) -> Unit = {},
    onAboutShown: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onRetryBootstrap: () -> Unit,
    onStartRecording: () -> Unit,
    onPauseRecording: () -> Unit = {},
    onResumeRecording: () -> Unit = {},
    onStopRecording: () -> Unit,
    onRefreshLibrary: () -> Unit,
    onLoadMoreLibrary: () -> Unit,
    onRetryLibrary: () -> Unit,
    onLoadLibraryTranscript: (String) -> Unit,
    onLibrarySearchChanged: (String) -> Unit,
    onLibraryScopeChanged: (HistoryScope) -> Unit,
    onLibraryFilterChanged: (HistoryFilterType) -> Unit,
    onLibraryDateChanged: (HistoryDateFilter) -> Unit,
    onLibrarySortChanged: (HistorySortOrder) -> Unit,
    onToggleLibrarySelection: (String) -> Unit,
    onClearLibrarySelection: () -> Unit,
    onTrashLibrarySelection: () -> Unit,
    onRestoreLibrarySelection: () -> Unit,
    onPurgeLibrarySelection: () -> Unit,
    onTrashLibraryItem: (String) -> Unit = {},
    onRestoreLibraryItem: (String) -> Unit = {},
    onMoveSelectedToProject: (String?) -> Unit,
    onUpdateHistoryTitle: (String, String) -> Unit,
    onUpdateHistoryProject: (String, String?) -> Unit,
    onCreateHistoryProject: (String) -> Unit,
    onRenameHistoryProject: (String, String) -> Unit = { _, _ -> },
    onDeleteHistoryProject: (String) -> Unit = {},
    onLoadTranscriptSnapshot: (String, String) -> Unit,
    onCloseTranscriptSnapshot: () -> Unit,
    onSummarizeLibrary: () -> Unit = {},
    onTranslateLibrary: (String, String?) -> Unit = { _, _ -> },
    onPolishLibrary: () -> Unit = {},
    onRetryLibraryLlm: () -> Unit = {},
    onClearLibraryLlmConfigurationPrompt: () -> Unit = {},
    onExportTranscript: (String, TranscriptExportFormat, TranscriptExportMode) -> Unit,
    onTogglePlayback: () -> Unit,
    onSeekPlayback: (Long) -> Unit,
    onSkipPlayback: (Long) -> Unit,
    onSetPlaybackSpeed: (Float) -> Unit,
    onPausePlayback: () -> Unit,
    onReleasePlayback: () -> Unit,
    onStartTranscriptEdit: (String, String?) -> Unit,
    onEditTranscriptSegment: (String?) -> Unit,
    onUpdateTranscriptText: (String, String) -> Unit,
    onUpdateTranscriptTranslation: (String, String) -> Unit,
    onDeleteTranscriptSegment: (String) -> Unit,
    onMergeTranscriptSegment: (String) -> Unit,
    onSplitTranscriptSegment: (String, String, String, String?, String?) -> Unit,
    onUndoTranscriptEdit: () -> Unit,
    onRedoTranscriptEdit: () -> Unit,
    onSaveTranscriptEdit: () -> Unit,
    onDiscardTranscriptEdit: () -> Unit,
    onFlushTranscriptEdit: () -> Unit,
    onTranscribeWithCloud: (HistoryItem) -> Unit,
    onImportAudio: (String) -> Unit,
    onCancelAudioImport: () -> Unit,
    onCancelAudioImportJob: (String) -> Unit = { onCancelAudioImport() },
    onDismissFailedAudioImport: (String?) -> Unit = {},
    onTranscribeWithCurrentEngine: (HistoryItem) -> Unit,
    onCloudProviderSelected: (OnlineAsrProvider) -> Unit,
    onCloudApiKeyInputChanged: (String) -> Unit,
    onSaveCloudApiKey: () -> Unit,
    onClearCloudApiKey: () -> Unit,
    onSelectModel: (AsrSelectionSlot, AsrModelSelection?) -> Unit,
    onDownloadLocalModel: (String) -> Unit,
    onValidateLocalModel: (String) -> Unit,
    onDeleteLocalModel: (String) -> Unit,
    onRefreshRecognitionCatalog: () -> Unit,
    onRefreshSync: () -> Unit,
    onTestSyncProvider: (WebDavSyncProvider) -> Unit,
    onCreateSync: (WebDavSyncProvider, SyncPreset, String) -> Unit,
    onPreviewSyncJoin: (WebDavSyncProvider, String, String) -> Unit,
    onJoinSync: (WebDavSyncProvider, String, String) -> Unit,
    onUnlockSync: (String, String) -> Unit,
    onUnlockSyncWithRecovery: (String, String) -> Unit,
    onRunSync: () -> Unit,
    onPauseSync: (Boolean) -> Unit,
    onLockSync: () -> Unit,
    onDisconnectSync: () -> Unit,
    onGenerateSyncRecoveryKey: () -> Unit,
    onExportSyncRecoveryKey: (String) -> Unit,
    onConsumeSyncRecoveryKey: () -> Unit,
    onResolveSyncConflict: (String, SyncConflictResolution) -> Unit,
    onLoadSyncConflict: (String) -> Unit,
    onChangeSyncPreset: (SyncPreset) -> Unit,
    onChangeSyncPassword: (String, String) -> Unit,
    onDiscoverSyncVaults: ((WebDavSyncProvider, ((List<DiscoveredVaultSummary>) -> Unit)?) -> Unit)? = null,
    onCreateSyncWithVaultId: ((WebDavSyncProvider, SyncPreset, String, String?, Boolean) -> Unit)? = null,
    onApplySyncPairingToken: ((String) -> SyncPairingPayload?)? = null,
    onClearSyncPairingNotice: (() -> Unit)? = null,
    onGetSyncPairingToken: ((Boolean, String) -> String?)? = null,
    onExportBackup: (String) -> Unit,
    onInspectBackup: (String) -> Unit,
    onConfirmBackupImport: () -> Unit,
    onCancelBackupImport: () -> Unit,
    onRefreshRecovery: () -> Unit,
    onResumeRecovery: (String) -> Unit,
    onResumeAllRecovery: () -> Unit,
    onDiscardRecovery: (String) -> Unit,
    onClearResolvedRecovery: () -> Unit,
    onLlmProvider: (String) -> Unit = {},
    onLlmModel: (String) -> Unit = {},
    onLlmBaseUrl: (String) -> Unit = {},
    onLlmPath: (String) -> Unit = {},
    onLlmVersion: (String) -> Unit = {},
    onLlmApiKey: (String) -> Unit = {},
    onLlmSave: () -> Unit = {},
    onLlmClear: () -> Unit = {},
    pendingOpenHistoryId: String? = null,
    onPendingOpenHistoryIdConsumed: () -> Unit = {},
) {
    var cloudCredentialFocusRequested by remember { mutableStateOf(false) }
    var requestedSettingsSection by remember { mutableStateOf<SettingsSection?>(null) }
    var settingsSectionRequestToken by remember { mutableIntStateOf(0) }
    var settingsSinglePaneDetailActive by remember { mutableStateOf(false) }
    var settingsSinglePaneListActive by remember { mutableStateOf(false) }
    var detailExitRequestToken by remember { mutableIntStateOf(0) }
    var pendingDetailDestination by remember { mutableStateOf<String?>(null) }
    var recordingSheetExpanded by rememberSaveable { mutableStateOf(false) }
    var autoStartRequestToken by remember { mutableIntStateOf(0) }
    var hasRequestedMicPermission by rememberSaveable { mutableStateOf(false) }
    var hasRequestedNotificationPermission by rememberSaveable { mutableStateOf(false) }
    var lastRecordingSegments by remember { mutableStateOf<List<TranscriptSegment>>(emptyList()) }
    var dismissedCompletedId by rememberSaveable { mutableStateOf<String?>(null) }
    var visibleCompletedToast by remember { mutableStateOf<LiveRecordingState.Completed?>(null) }
    var bottomOverlayHeightPx by remember { mutableIntStateOf(0) }
    var miniBarHeightPx by remember { mutableIntStateOf(0) }
    val density = LocalDensity.current
    val bottomOverlayPadding = with(density) { bottomOverlayHeightPx.toDp() }
    val miniBarOverlayPadding = with(density) { miniBarHeightPx.toDp() }

    val recoveryPendingCount = dataRecoveryState.recovery.items.count {
        it.resolution == RecoveryResolution.PENDING
    }

    LaunchedEffect(recordingState) {
        when (recordingState) {
            is LiveRecordingState.Preparing -> {
                lastRecordingSegments = emptyList()
                visibleCompletedToast = null
                dismissedCompletedId = null
            }
            is LiveRecordingState.Recording -> {
                lastRecordingSegments = recordingState.segments
                visibleCompletedToast = null
            }
            is LiveRecordingState.Completed -> {
                if (recordingState.historyId != dismissedCompletedId) {
                    visibleCompletedToast = recordingState
                    delay(if (recordingState.warning != null) 5_500L else 3_500L)
                    visibleCompletedToast = null
                    dismissedCompletedId = recordingState.historyId
                }
            }
            else -> {
                visibleCompletedToast = null
            }
        }
    }

    SonaTheme(appearanceSettings = appearanceState.toDomainSettings()) {
        val navController = rememberNavController()
        val backStackEntry by navController.currentBackStackEntryAsState()
        val currentRoute = backStackEntry?.destination?.route ?: SonaDestination.LIBRARY.route
        val resolveEffectiveSectionArg: (NavBackStackEntry) -> String? = { entry ->
            SonaNavMotion.resolveEffectiveSettingsSectionArg(
                route = entry.destination.route,
                rawSectionArg = entry.arguments?.getString(SETTINGS_SECTION_ARGUMENT),
                singlePaneListActive = settingsSinglePaneListActive,
            )
        }
        val currentSectionArg = backStackEntry?.let(resolveEffectiveSectionArg)
        val registeredRoute = backStackEntry?.destination?.route
        val isLibraryDetail = registeredRoute == LIBRARY_DETAIL_ROUTE ||
            (currentRoute.startsWith("${SonaDestination.LIBRARY.route}/") && currentRoute != SonaDestination.LIBRARY.route)

        val audioPicker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            uri?.let {
                onImportAudio(it.toString())
                if (currentRoute != SonaDestination.LIBRARY.route) {
                    navController.navigate(SonaDestination.LIBRARY.route) {
                        popUpTo(SonaDestination.LIBRARY.route) { saveState = true }
                        launchSingleTop = true
                        restoreState = true
                    }
                }
            }
        }

        val navigateToSettingsSection: (SettingsSection, Boolean) -> Unit = { section, focusCloud ->
            recordingSheetExpanded = false
            if (focusCloud) {
                onCloudProviderSelected(OnlineAsrProvider.VOLCENGINE_DOUBAO)
            }
            cloudCredentialFocusRequested = focusCloud
            requestedSettingsSection = section
            settingsSinglePaneListActive = false
            settingsSectionRequestToken += 1
            navController.navigate(settingsRoute(section)) {
                popUpTo(SonaDestination.LIBRARY.route) { saveState = true }
                launchSingleTop = true
            }
        }

        val openCompletedHistory: (String) -> Unit = { historyId ->
            visibleCompletedToast = null
            dismissedCompletedId = historyId
            recordingSheetExpanded = false
            val targetRoute = libraryDetailRoute(historyId)
            val currentHistoryId = backStackEntry?.arguments?.getString(LIBRARY_HISTORY_ID_ARGUMENT)
            if (isLibraryDetail && currentHistoryId == historyId) {
                // Already viewing the completed recording
            } else if (isLibraryDetail && libraryState.editor.dirty) {
                pendingDetailDestination = targetRoute
                detailExitRequestToken += 1
            } else {
                navController.navigate(targetRoute) {
                    launchSingleTop = true
                }
            }
        }

        val onConfigureCredential = {
            navigateToSettingsSection(SettingsSection.RECOGNITION, true)
        }
        val onConfigureRecognition = {
            navigateToSettingsSection(SettingsSection.RECOGNITION, false)
        }
        val onConfigureLlm = {
            navigateToSettingsSection(SettingsSection.LLM, false)
        }

        val isRecordingActive = recordingState is LiveRecordingState.Preparing ||
            recordingState is LiveRecordingState.Recording ||
            recordingState is LiveRecordingState.Stopping
        val miniBarVisible = isRecordingActive && !recordingSheetExpanded

        LaunchedEffect(pendingOpenHistoryId) {
            val targetId = pendingOpenHistoryId?.takeIf(String::isNotBlank) ?: return@LaunchedEffect
            recordingSheetExpanded = false
            val targetRoute = libraryDetailRoute(targetId)
            if (isLibraryDetail && libraryState.editor.dirty) {
                pendingDetailDestination = targetRoute
                detailExitRequestToken += 1
            } else {
                navController.navigate(targetRoute) {
                    launchSingleTop = true
                }
            }
            onPendingOpenHistoryIdConsumed()
        }

        val onRecordCapsuleClick = {
            recordingSheetExpanded = true
            val canAutoStart = recordingState is LiveRecordingState.Idle ||
                recordingState is LiveRecordingState.NeedsConfiguration ||
                recordingState is LiveRecordingState.Completed ||
                recordingState is LiveRecordingState.Failed
            if (canAutoStart) {
                autoStartRequestToken += 1
            }
        }

        Box(modifier = Modifier.fillMaxSize()) {
            Surface(
                modifier = Modifier.fillMaxSize(),
                color = MaterialTheme.colorScheme.background,
            ) {
                NavHost(
                    navController = navController,
                    startDestination = SonaDestination.LIBRARY.route,
                    modifier = Modifier.fillMaxSize(),
                    enterTransition = {
                        sonaEnterTransition(sectionArgResolver = resolveEffectiveSectionArg)
                    },
                    exitTransition = {
                        sonaExitTransition(sectionArgResolver = resolveEffectiveSectionArg)
                    },
                    popEnterTransition = {
                        sonaPopEnterTransition(sectionArgResolver = resolveEffectiveSectionArg)
                    },
                    popExitTransition = {
                        sonaPopExitTransition(sectionArgResolver = resolveEffectiveSectionArg)
                    },
                    predictivePopEnterTransition = { swipeEdge ->
                        sonaPredictivePopEnterTransition(
                            swipeEdge = swipeEdge,
                            sectionArgResolver = resolveEffectiveSectionArg,
                        )
                    },
                    predictivePopExitTransition = { swipeEdge ->
                        sonaPredictivePopExitTransition(
                            swipeEdge = swipeEdge,
                            sectionArgResolver = resolveEffectiveSectionArg,
                        )
                    },
                ) {
                    composable(SonaDestination.LIBRARY.route) {
                        LaunchedEffect(Unit) { onRefreshLibrary() }
                        SonaParentPageTransitionContainer(
                            scope = this,
                            enableScrim = SonaNavMotion.isSubPage(currentRoute, currentSectionArg),
                        ) {
                            LibraryScreen(
                                state = libraryState,
                                recognitionSettings = recognitionSettingsState,
                                configuredProviders = cloudTranscriptionState.configuredProviders,
                                onRefresh = onRefreshLibrary,
                                onLoadMore = onLoadMoreLibrary,
                                onRetry = onRetryLibrary,
                                onOpenItem = { historyId ->
                                    navController.navigate(libraryDetailRoute(historyId))
                                },
                                onSearchChanged = onLibrarySearchChanged,
                                onScopeChanged = onLibraryScopeChanged,
                                onFilterChanged = onLibraryFilterChanged,
                                onDateChanged = onLibraryDateChanged,
                                onSortChanged = onLibrarySortChanged,
                                onToggleSelection = onLibrarySelectionToggle(onToggleLibrarySelection),
                                onClearSelection = onClearLibrarySelection,
                                onTrashSelected = onTrashLibrarySelection,
                                onRestoreSelected = onRestoreLibrarySelection,
                                onPurgeSelected = onPurgeLibrarySelection,
                                onTrashItem = onTrashLibraryItem,
                                onRestoreItem = onRestoreLibraryItem,
                                onMoveSelectedToProject = onMoveSelectedToProject,
                                onCreateProject = onCreateHistoryProject,
                                onRenameProject = onRenameHistoryProject,
                                onDeleteProject = onDeleteHistoryProject,
                                onCancelAudioImportJob = onCancelAudioImportJob,
                                onDismissFailedImport = onDismissFailedAudioImport,
                                onConfigureRecognition = onConfigureRecognition,
                                recoveryPendingCount = recoveryPendingCount,
                                onOpenRecovery = {
                                    navigateToSettingsSection(SettingsSection.DATA_RECOVERY, false)
                                },
                                extraBottomPadding = bottomOverlayPadding,
                            )
                        }
                    }
                    composable(
                        route = LIBRARY_DETAIL_ROUTE,
                        arguments = listOf(
                            navArgument(LIBRARY_HISTORY_ID_ARGUMENT) {
                                type = NavType.StringType
                            },
                        ),
                    ) { entry ->
                        val historyId = checkNotNull(
                            entry.arguments?.getString(LIBRARY_HISTORY_ID_ARGUMENT),
                        )
                        LaunchedEffect(historyId) {
                            onLoadLibraryTranscript(historyId)
                        }
                        val isCoveredByAnotherSubPage = currentRoute != LIBRARY_DETAIL_ROUTE &&
                            SonaNavMotion.isSubPage(currentRoute, currentSectionArg)
                        SonaSubPageTransitionContainer(
                            scope = this,
                            enabled = !isCoveredByAnotherSubPage,
                        ) {
                            LibraryDetailScreen(
                                historyId = historyId,
                                item = libraryState.items.firstOrNull { it.historyId == historyId },
                                detail = libraryState.detail,
                                cloudTranscription = libraryState.cloudTranscription,
                                projects = libraryState.projects,
                                snapshots = libraryState.snapshots,
                                snapshotDetail = libraryState.snapshotDetail,
                                operationInProgress = libraryState.operationInProgress,
                                operationError = libraryState.operationError,
                                onRetry = { onLoadLibraryTranscript(historyId) },
                                onTranscribeWithCloud = onTranscribeWithCloud,
                                onTranscribeWithCurrentEngine = onTranscribeWithCurrentEngine,
                                onUpdateTitle = { title -> onUpdateHistoryTitle(historyId, title) },
                                onUpdateProject = { projectId -> onUpdateHistoryProject(historyId, projectId) },
                                onCreateProject = onCreateHistoryProject,
                                onLoadSnapshot = { snapshotId -> onLoadTranscriptSnapshot(historyId, snapshotId) },
                                onCloseSnapshot = onCloseTranscriptSnapshot,
                                onExportTranscript = onExportTranscript,
                                playback = libraryState.playback,
                                editor = libraryState.editor,
                                llm = libraryState.llm,
                                onSummarize = onSummarizeLibrary,
                                onTranslate = onTranslateLibrary,
                                onPolish = onPolishLibrary,
                                onRetryLlm = onRetryLibraryLlm,
                                onConfigureLlm = onConfigureLlm,
                                onClearLlmConfigurationPrompt = onClearLibraryLlmConfigurationPrompt,
                                appLanguage = appLanguage,
                                exitRequestToken = detailExitRequestToken,
                                onExitRequestConsumed = { detailExitRequestToken = 0 },
                                onNavigateBack = {
                                    val destination = pendingDetailDestination
                                    pendingDetailDestination = null
                                    if (destination == null) {
                                        navController.popBackStack()
                                    } else {
                                        navController.navigate(destination) {
                                            popUpTo(SonaDestination.LIBRARY.route) { saveState = true }
                                            launchSingleTop = true
                                            restoreState = true
                                        }
                                    }
                                },
                                onTogglePlayback = onTogglePlayback,
                                onSeekPlayback = onSeekPlayback,
                                onSkipPlayback = onSkipPlayback,
                                onSetPlaybackSpeed = onSetPlaybackSpeed,
                                onPausePlayback = onPausePlayback,
                                onReleasePlayback = onReleasePlayback,
                                onStartEditing = { segmentId -> onStartTranscriptEdit(historyId, segmentId) },
                                onEditSegment = onEditTranscriptSegment,
                                onUpdateText = onUpdateTranscriptText,
                                onUpdateTranslation = onUpdateTranscriptTranslation,
                                onDeleteSegment = onDeleteTranscriptSegment,
                                onMergeSegment = onMergeTranscriptSegment,
                                onSplitSegment = onSplitTranscriptSegment,
                                onUndoEdit = onUndoTranscriptEdit,
                                onRedoEdit = onRedoTranscriptEdit,
                                onSaveEdit = onSaveTranscriptEdit,
                                onDiscardEdit = onDiscardTranscriptEdit,
                                onFlushEdit = onFlushTranscriptEdit,
                                extraBottomPadding = if (miniBarVisible) miniBarOverlayPadding else 0.dp,
                            )
                        }
                    }
                    composable(
                        route = SonaDestination.SETTINGS.routePattern,
                        arguments = listOf(
                            navArgument(SETTINGS_SECTION_ARGUMENT) {
                                type = NavType.StringType
                                nullable = true
                                defaultValue = null
                            },
                        ),
                    ) { entry ->
                        val entrySection = entry.arguments?.getString(SETTINGS_SECTION_ARGUMENT)
                        SonaSubPageTransitionContainer(
                            scope = this,
                            enabled = !entrySection.isNullOrBlank() && !settingsSinglePaneListActive,
                        ) {
                            SettingsScreen(
                                initialSection = SettingsSection.fromRoute(entrySection),
                                requestedSection = requestedSettingsSection,
                                sectionRequestToken = settingsSectionRequestToken,
                                bootstrapState = bootstrapState,
                                appearanceState = appearanceState,
                                cloudTranscriptionState = cloudTranscriptionState,
                                recognitionSettingsState = recognitionSettingsState,
                                syncState = syncState,
                                dataRecoveryState = dataRecoveryState,
                                llmState = llmState,
                                aboutState = aboutState,
                                appLanguage = appLanguage,
                                requestCloudCredentialFocus = cloudCredentialFocusRequested,
                                onAppLanguageChanged = onAppLanguageChanged,
                                onThemeModeChanged = onThemeModeChanged,
                                onPureBlackDarkChanged = onPureBlackDarkChanged,
                                onSelectSystemDynamicColor = onSelectSystemDynamicColor,
                                onSelectPalettePreset = onSelectPalettePreset,
                                onCustomSeedColorChanged = onCustomSeedColorChanged,
                                onPaletteStyleChanged = onPaletteStyleChanged,
                                onContrastLevelChanged = onContrastLevelChanged,
                                onAboutShown = onAboutShown,
                                onCheckForUpdates = onCheckForUpdates,
                                onCloudProviderSelected = onCloudProviderSelected,
                                onCloudApiKeyInputChanged = onCloudApiKeyInputChanged,
                                onSaveCloudApiKey = onSaveCloudApiKey,
                                onClearCloudApiKey = onClearCloudApiKey,
                                onSelectModel = onSelectModel,
                                onDownloadLocalModel = onDownloadLocalModel,
                                onValidateLocalModel = onValidateLocalModel,
                                onDeleteLocalModel = onDeleteLocalModel,
                                onRefreshRecognitionCatalog = onRefreshRecognitionCatalog,
                                onRefreshSync = onRefreshSync,
                                onTestSyncProvider = onTestSyncProvider,
                                onCreateSync = onCreateSync,
                                onPreviewSyncJoin = onPreviewSyncJoin,
                                onJoinSync = onJoinSync,
                                onUnlockSync = onUnlockSync,
                                onUnlockSyncWithRecovery = onUnlockSyncWithRecovery,
                                onRunSync = onRunSync,
                                onPauseSync = onPauseSync,
                                onLockSync = onLockSync,
                                onDisconnectSync = onDisconnectSync,
                                onGenerateSyncRecoveryKey = onGenerateSyncRecoveryKey,
                                onExportSyncRecoveryKey = onExportSyncRecoveryKey,
                                onConsumeSyncRecoveryKey = onConsumeSyncRecoveryKey,
                                onResolveSyncConflict = onResolveSyncConflict,
                                onLoadSyncConflict = onLoadSyncConflict,
                                onChangeSyncPreset = onChangeSyncPreset,
                                onChangeSyncPassword = onChangeSyncPassword,
                                onDiscoverSyncVaults = onDiscoverSyncVaults,
                                onCreateSyncWithVaultId = onCreateSyncWithVaultId,
                                onApplySyncPairingToken = onApplySyncPairingToken,
                                onClearSyncPairingNotice = onClearSyncPairingNotice,
                                onGetSyncPairingToken = onGetSyncPairingToken,
                                onExportBackup = onExportBackup,
                                onInspectBackup = onInspectBackup,
                                onConfirmBackupImport = onConfirmBackupImport,
                                onCancelBackupImport = onCancelBackupImport,
                                onRefreshRecovery = onRefreshRecovery,
                                onResumeRecovery = { itemId ->
                                    val item = dataRecoveryState.recovery.items.firstOrNull { it.id == itemId }
                                    val transcriptHistoryId = item?.historyId
                                    if (item?.source == RecoverySource.TRANSCRIPT_EDIT && transcriptHistoryId != null) {
                                        navController.navigate(libraryDetailRoute(transcriptHistoryId)) {
                                            popUpTo(SonaDestination.LIBRARY.route) { saveState = true }
                                            launchSingleTop = true
                                        }
                                    } else {
                                        onResumeRecovery(itemId)
                                    }
                                },
                                onResumeAllRecovery = onResumeAllRecovery,
                                onDiscardRecovery = onDiscardRecovery,
                                onClearResolvedRecovery = onClearResolvedRecovery,
                                onLlmProvider = onLlmProvider,
                                onLlmModel = onLlmModel,
                                onLlmBaseUrl = onLlmBaseUrl,
                                onLlmPath = onLlmPath,
                                onLlmVersion = onLlmVersion,
                                onLlmApiKey = onLlmApiKey,
                                onLlmSave = onLlmSave,
                                onLlmClear = onLlmClear,
                                onCloudCredentialFocusConsumed = {
                                    cloudCredentialFocusRequested = false
                                },
                                onShowingSinglePaneDetailChanged = { detailActive ->
                                    settingsSinglePaneDetailActive = detailActive
                                },
                                onShowingSinglePaneListChanged = { listActive ->
                                    settingsSinglePaneListActive = listActive
                                },
                                onCloseToLibrary = {
                                    navController.popBackStack()
                                },
                                bottomOverlayPadding = bottomOverlayPadding,
                            )
                        }
                    }
                }
            }

            // Bottom Dock Area (Mini-Bar when recording minimized + Floating Capture/Nav Dock)
            val inSettings = SonaDestination.SETTINGS.matches(currentRoute)
            val showDockForDestination = if (inSettings) {
                !settingsSinglePaneDetailActive
            } else {
                SonaFloatingDockDefaults.shouldShow(currentRoute, currentSectionArg)
            }
            val showFloatingDock = showDockForDestination &&
                !recordingSheetExpanded &&
                libraryState.selectedIds.isEmpty()

            Column(
                modifier = Modifier
                    .align(Alignment.BottomCenter)
                    .fillMaxWidth()
                    .onSizeChanged { size -> bottomOverlayHeightPx = size.height },
                horizontalAlignment = Alignment.CenterHorizontally,
            ) {
                val motionScheme = MaterialTheme.motionScheme
                val toast = visibleCompletedToast
                AnimatedVisibility(
                    visible = toast != null && !recordingSheetExpanded,
                    enter = slideInVertically(
                        initialOffsetY = { it },
                        animationSpec = motionScheme.defaultSpatialSpec(),
                    ) + fadeIn(
                        animationSpec = motionScheme.defaultEffectsSpec(),
                    ),
                    exit = slideOutVertically(
                        targetOffsetY = { it },
                        animationSpec = motionScheme.fastSpatialSpec(),
                    ) + fadeOut(
                        animationSpec = motionScheme.fastEffectsSpec(),
                    ),
                ) {
                    if (toast != null) {
                        RecordingCompletedToast(
                            completed = toast,
                            onView = { openCompletedHistory(toast.historyId) },
                            modifier = if (showFloatingDock) {
                                Modifier.padding(horizontal = 16.dp, vertical = 2.dp)
                            } else {
                                Modifier
                                    .windowInsetsPadding(WindowInsets.navigationBars)
                                    .padding(horizontal = 16.dp, vertical = 12.dp)
                            },
                        )
                    }
                }

                AnimatedVisibility(
                    visible = isRecordingActive && !recordingSheetExpanded,
                    enter = slideInVertically(
                        initialOffsetY = { it },
                        animationSpec = motionScheme.defaultSpatialSpec(),
                    ) + fadeIn(
                        animationSpec = motionScheme.defaultEffectsSpec(),
                    ),
                    exit = slideOutVertically(
                        targetOffsetY = { it },
                        animationSpec = motionScheme.fastSpatialSpec(),
                    ) + fadeOut(
                        animationSpec = motionScheme.fastEffectsSpec(),
                    ),
                ) {
                    RecordMiniBar(
                        recordingState = recordingState,
                        onExpand = { recordingSheetExpanded = true },
                        onPause = onPauseRecording,
                        onResume = onResumeRecording,
                        onStop = onStopRecording,
                        modifier = Modifier
                            .onSizeChanged { size -> miniBarHeightPx = size.height }
                            .then(
                                if (showFloatingDock) {
                                    Modifier.padding(horizontal = 16.dp, vertical = 2.dp)
                                } else {
                                    Modifier
                                        .windowInsetsPadding(WindowInsets.navigationBars)
                                        .padding(horizontal = 16.dp, vertical = 12.dp)
                                },
                            ),
                    )
                }

                SonaFloatingDock(
                    currentRoute = currentRoute,
                    currentSectionArg = currentSectionArg,
                    recordingState = recordingState,
                    visible = showFloatingDock,
                    onDestinationSelected = { destination ->
                        if (isLibraryDetail && libraryState.editor.dirty) {
                            pendingDetailDestination = destination.route
                            detailExitRequestToken += 1
                        } else {
                            if (destination == SonaDestination.SETTINGS) {
                                requestedSettingsSection = null
                            }
                            navController.navigate(destination.route) {
                                popUpTo(SonaDestination.LIBRARY.route) { saveState = true }
                                launchSingleTop = true
                                restoreState = true
                            }
                        }
                    },
                    onRecordClick = onRecordCapsuleClick,
                    onImportAudioClick = { audioPicker.launch(arrayOf("audio/*")) },
                )
            }

            // Full-Screen Live Recording Sheet Overlay
            val overlayMotionScheme = MaterialTheme.motionScheme
            AnimatedVisibility(
                visible = recordingSheetExpanded,
                enter = slideInVertically(
                    initialOffsetY = { it / 2 },
                    animationSpec = overlayMotionScheme.defaultSpatialSpec(),
                ) + fadeIn(
                    animationSpec = overlayMotionScheme.defaultEffectsSpec(),
                ),
                exit = slideOutVertically(
                    targetOffsetY = { it / 2 },
                    animationSpec = overlayMotionScheme.fastSpatialSpec(),
                ) + fadeOut(
                    animationSpec = overlayMotionScheme.fastEffectsSpec(),
                ),
            ) {
                RecordScreen(
                    bootstrapState = bootstrapState,
                    recordingState = recordingState,
                    configuredProviders = cloudTranscriptionState.configuredProviders,
                    recognitionSettings = recognitionSettingsState,
                    isExpanded = recordingSheetExpanded,
                    persistedSegments = lastRecordingSegments,
                    completedToast = visibleCompletedToast,
                    autoStartRequestToken = autoStartRequestToken,
                    hasRequestedMicPermission = hasRequestedMicPermission,
                    hasRequestedNotificationPermission = hasRequestedNotificationPermission,
                    onMicPermissionRequested = { hasRequestedMicPermission = true },
                    onNotificationPermissionRequested = { hasRequestedNotificationPermission = true },
                    onRetryBootstrap = onRetryBootstrap,
                    onStartRecording = onStartRecording,
                    onPauseRecording = onPauseRecording,
                    onResumeRecording = onResumeRecording,
                    onStopRecording = onStopRecording,
                    onSelectLiveModel = { selection ->
                        onSelectModel(AsrSelectionSlot.LIVE, selection)
                    },
                    onConfigureCredential = onConfigureCredential,
                    onConfigureRecognition = onConfigureRecognition,
                    onViewCompleted = openCompletedHistory,
                    onMinimize = { recordingSheetExpanded = false },
                )
            }
        }
    }
}

private inline fun onLibrarySelectionToggle(
    crossinline delegate: (String) -> Unit,
): (String) -> Unit = { id -> delegate(id) }

internal fun settingsRoute(section: SettingsSection): String =
    "${SonaDestination.SETTINGS.route}?$SETTINGS_SECTION_ARGUMENT=${section.route}"

internal fun libraryDetailRoute(historyId: String): String {
    require(historyId.isNotBlank()) { "History ID must not be blank." }
    val encodedHistoryId = URLEncoder.encode(historyId, StandardCharsets.UTF_8.name())
        .replace("+", "%20")
    return "${SonaDestination.LIBRARY.route}/$encodedHistoryId"
}

internal const val LIBRARY_HISTORY_ID_ARGUMENT = "historyId"
internal const val LIBRARY_DETAIL_ROUTE = "library/{$LIBRARY_HISTORY_ID_ARGUMENT}"
