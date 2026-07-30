package com.sona.android.app.navigation

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.MediumTopAppBar
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.navigation.NavType
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import androidx.navigation.navArgument
import com.sona.android.app.BuildConfig
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.app.feature.library.LibraryDetailScreen
import com.sona.android.app.feature.library.LibraryScreen
import com.sona.android.app.feature.library.LibraryUiState
import com.sona.android.app.feature.recording.RecordScreen
import com.sona.android.app.feature.home.HomeScreen
import com.sona.android.app.feature.home.FileTranscriptionScreen
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.AboutSettingsUiState
import com.sona.android.app.feature.settings.AppearanceSettingsUiState
import com.sona.android.app.feature.settings.CloudTranscriptionSettingsUiState
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.app.feature.settings.SettingsScreen
import com.sona.android.app.feature.settings.SettingsSection
import com.sona.android.app.feature.settings.SyncSettingsUiState
import com.sona.android.app.feature.settings.DataRecoveryUiState
import com.sona.android.app.ui.theme.SonaTheme
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.HistorySortOrder
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.WebDavSyncProvider
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySource
import java.net.URLEncoder
import java.nio.charset.StandardCharsets

@OptIn(ExperimentalMaterial3Api::class)
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
    aboutState: AboutSettingsUiState,
    appLanguage: AppLanguage,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onAboutShown: () -> Unit,
    onCheckForUpdates: () -> Unit,
    onRetryBootstrap: () -> Unit,
    onStartRecording: () -> Unit,
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
    onAddTagToLibrarySelection: (String) -> Unit,
    onRemoveTagFromLibrarySelection: (String) -> Unit,
    onUpdateHistoryTitle: (String, String) -> Unit,
    onUpdateHistoryTags: (String, Set<String>) -> Unit,
    onCreateHistoryTag: (String) -> Unit,
    onLoadTranscriptSnapshot: (String, String) -> Unit,
    onCloseTranscriptSnapshot: () -> Unit,
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
    onExportBackup: (String) -> Unit,
    onInspectBackup: (String) -> Unit,
    onConfirmBackupImport: () -> Unit,
    onCancelBackupImport: () -> Unit,
    onRefreshRecovery: () -> Unit,
    onResumeRecovery: (String) -> Unit,
    onResumeAllRecovery: () -> Unit,
    onDiscardRecovery: (String) -> Unit,
    onClearResolvedRecovery: () -> Unit,
) {
    var cloudCredentialFocusRequested by remember { mutableStateOf(false) }
    var detailExitRequestToken by remember { mutableStateOf(0) }
    var pendingDetailDestination by remember { mutableStateOf<String?>(null) }
    val recoveryPendingCount = dataRecoveryState.recovery.items.count {
        it.resolution == RecoveryResolution.PENDING
    }

    SonaTheme(dynamicColorEnabled = appearanceState.dynamicColorEnabled) {
        val navController = rememberNavController()
        val backStackEntry by navController.currentBackStackEntryAsState()
        val currentRoute = backStackEntry?.destination?.route ?: SonaDestination.HOME.route
        val currentDestination = SonaDestination.entries.firstOrNull { it.matches(currentRoute) }
            ?: SonaDestination.HOME
        val isLibraryDetail = currentRoute == LIBRARY_DETAIL_ROUTE
        val isHomeWorkspace = currentRoute == HOME_LIVE_ROUTE || currentRoute == HOME_FILE_ROUTE
        val onConfigureCredential = {
            onCloudProviderSelected(OnlineAsrProvider.VOLCENGINE_DOUBAO)
            cloudCredentialFocusRequested = true
            navController.navigate(settingsRoute(SettingsSection.RECOGNITION)) {
                popUpTo(SonaDestination.HOME.route) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        }
        val onConfigureRecognition = {
            cloudCredentialFocusRequested = false
            navController.navigate(settingsRoute(SettingsSection.RECOGNITION)) {
                popUpTo(SonaDestination.HOME.route) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        }

        val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()

        NavigationSuiteScaffold(
            navigationSuiteItems = {
                SonaDestination.entries.forEach { destination ->
                    item(
                        selected = destination.matches(currentRoute),
                        onClick = {
                            if (isLibraryDetail && libraryState.editor.dirty) {
                                pendingDetailDestination = destination.route
                                detailExitRequestToken += 1
                            } else {
                                navController.navigate(destination.route) {
                                    popUpTo(SonaDestination.HOME.route) { saveState = true }
                                    launchSingleTop = true
                                    restoreState = true
                                }
                            }
                        },
                        icon = {
                            Icon(
                                imageVector = destination.icon,
                                contentDescription = stringResource(destination.labelRes),
                            )
                        },
                        label = { Text(stringResource(destination.labelRes)) },
                    )
                }
            },
        ) {
            Scaffold(
                modifier = Modifier.nestedScroll(scrollBehavior.nestedScrollConnection),
                topBar = {
                    if (currentDestination != SonaDestination.SETTINGS) {
                        MediumTopAppBar(
                            scrollBehavior = scrollBehavior,
                            navigationIcon = {
                                if (isLibraryDetail || isHomeWorkspace) {
                                    IconButton(onClick = {
                                        if (isLibraryDetail) {
                                            pendingDetailDestination = null
                                            detailExitRequestToken += 1
                                        }
                                        else navController.popBackStack()
                                    }) {
                                        Icon(
                                            imageVector = Icons.AutoMirrored.Rounded.ArrowBack,
                                            contentDescription = stringResource(R.string.action_back),
                                        )
                                    }
                                }
                            },
                            title = {
                                Column {
                                    Text(
                                        text = BuildConfig.APP_NAME,
                                        style = MaterialTheme.typography.titleLarge,
                                        fontWeight = FontWeight.Bold,
                                        color = MaterialTheme.colorScheme.primary
                                    )
                                    Text(
                                        text = stringResource(currentDestination.labelRes),
                                        style = MaterialTheme.typography.labelMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            },
                            colors = TopAppBarDefaults.mediumTopAppBarColors(
                                containerColor = MaterialTheme.colorScheme.background,
                                scrolledContainerColor = MaterialTheme.colorScheme.surfaceContainer
                            )
                        )
                    }
                },
            ) { contentPadding ->
                NavHost(
                    navController = navController,
                    startDestination = SonaDestination.HOME.route,
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(contentPadding),
                ) {
                    composable(SonaDestination.HOME.route) {
                        LaunchedEffect(Unit) { onRefreshLibrary() }
                        HomeScreen(
                            recordingState = recordingState,
                            libraryState = libraryState,
                            recognitionSettings = recognitionSettingsState,
                            configuredProviders = cloudTranscriptionState.configuredProviders,
                            onOpenLive = { navController.navigate(HOME_LIVE_ROUTE) },
                            onOpenFile = { navController.navigate(HOME_FILE_ROUTE) },
                            onOpenLibrary = { navController.navigate(SonaDestination.LIBRARY.route) },
                            onOpenItem = { navController.navigate(libraryDetailRoute(it)) },
                            recoveryPendingCount = recoveryPendingCount,
                            onOpenRecovery = {
                                navController.navigate(settingsRoute(SettingsSection.DATA_RECOVERY))
                            },
                        )
                    }
                    composable(HOME_LIVE_ROUTE) {
                        RecordScreen(
                            bootstrapState = bootstrapState,
                            recordingState = recordingState,
                            onlineCredentialConfigured =
                                OnlineAsrProvider.VOLCENGINE_DOUBAO in
                                    cloudTranscriptionState.configuredProviders,
                            recognitionSettings = recognitionSettingsState,
                            onRetryBootstrap = onRetryBootstrap,
                            onStartRecording = onStartRecording,
                            onStopRecording = onStopRecording,
                            onConfigureCredential = onConfigureCredential,
                            onConfigureRecognition = onConfigureRecognition,
                        )
                    }
                    composable(HOME_FILE_ROUTE) {
                        FileTranscriptionScreen(
                            importState = libraryState.audioImport,
                            recognitionSettings = recognitionSettingsState,
                            configuredProviders = cloudTranscriptionState.configuredProviders,
                            onStart = onImportAudio,
                            onCancel = onCancelAudioImport,
                            onConfigure = onConfigureRecognition,
                            onViewResult = { navController.navigate(libraryDetailRoute(it)) },
                        )
                    }
                    composable(SonaDestination.LIBRARY.route) {
                        LaunchedEffect(Unit) { onRefreshLibrary() }
                        LibraryScreen(
                            state = libraryState,
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
                            onToggleSelection = onToggleLibrarySelection,
                            onClearSelection = onClearLibrarySelection,
                            onTrashSelected = onTrashLibrarySelection,
                            onRestoreSelected = onRestoreLibrarySelection,
                            onPurgeSelected = onPurgeLibrarySelection,
                            onAddTagToSelected = onAddTagToLibrarySelection,
                            onRemoveTagFromSelected = onRemoveTagFromLibrarySelection,
                            recoveryPendingCount = recoveryPendingCount,
                            onOpenRecovery = {
                                navController.navigate(settingsRoute(SettingsSection.DATA_RECOVERY))
                            },
                        )
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
                        LibraryDetailScreen(
                            historyId = historyId,
                            item = libraryState.items.firstOrNull { it.historyId == historyId },
                            detail = libraryState.detail,
                            cloudTranscription = libraryState.cloudTranscription,
                            tags = libraryState.tags,
                            snapshots = libraryState.snapshots,
                            snapshotDetail = libraryState.snapshotDetail,
                            operationInProgress = libraryState.operationInProgress,
                            operationError = libraryState.operationError,
                            onRetry = { onLoadLibraryTranscript(historyId) },
                            onTranscribeWithCloud = onTranscribeWithCloud,
                            onTranscribeWithCurrentEngine = onTranscribeWithCurrentEngine,
                            onUpdateTitle = { title -> onUpdateHistoryTitle(historyId, title) },
                            onUpdateTags = { selected -> onUpdateHistoryTags(historyId, selected) },
                            onCreateTag = onCreateHistoryTag,
                            onLoadSnapshot = { snapshotId -> onLoadTranscriptSnapshot(historyId, snapshotId) },
                            onCloseSnapshot = onCloseTranscriptSnapshot,
                            onExportTranscript = onExportTranscript,
                            playback = libraryState.playback,
                            editor = libraryState.editor,
                            exitRequestToken = detailExitRequestToken,
                            onNavigateBack = {
                                val destination = pendingDetailDestination
                                pendingDetailDestination = null
                                if (destination == null) {
                                    navController.popBackStack()
                                } else {
                                    navController.navigate(destination) {
                                        popUpTo(SonaDestination.HOME.route) { saveState = true }
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
                        )
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
                        SettingsScreen(
                            initialSection = SettingsSection.fromRoute(
                                entry.arguments?.getString(SETTINGS_SECTION_ARGUMENT),
                            ),
                            bootstrapState = bootstrapState,
                            appearanceState = appearanceState,
                            cloudTranscriptionState = cloudTranscriptionState,
                            recognitionSettingsState = recognitionSettingsState,
                            syncState = syncState,
                            dataRecoveryState = dataRecoveryState,
                            aboutState = aboutState,
                            appLanguage = appLanguage,
                            requestCloudCredentialFocus = cloudCredentialFocusRequested,
                            onAppLanguageChanged = onAppLanguageChanged,
                            onDynamicColorChanged = onDynamicColorChanged,
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
                            onExportBackup = onExportBackup,
                            onInspectBackup = onInspectBackup,
                            onConfirmBackupImport = onConfirmBackupImport,
                            onCancelBackupImport = onCancelBackupImport,
                            onRefreshRecovery = onRefreshRecovery,
                            onResumeRecovery = { itemId ->
                                val item = dataRecoveryState.recovery.items.firstOrNull { it.id == itemId }
                                val transcriptHistoryId = item?.historyId
                                if (item?.source == RecoverySource.TRANSCRIPT_EDIT && transcriptHistoryId != null) {
                                    navController.navigate(libraryDetailRoute(transcriptHistoryId))
                                } else {
                                    onResumeRecovery(itemId)
                                }
                            },
                            onResumeAllRecovery = onResumeAllRecovery,
                            onDiscardRecovery = onDiscardRecovery,
                            onClearResolvedRecovery = onClearResolvedRecovery,
                            onCloudCredentialFocusConsumed = {
                                cloudCredentialFocusRequested = false
                            },
                        )
                    }
                }
            }
        }
    }
}

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
internal const val HOME_LIVE_ROUTE = "home/live"
internal const val HOME_FILE_ROUTE = "home/file"
