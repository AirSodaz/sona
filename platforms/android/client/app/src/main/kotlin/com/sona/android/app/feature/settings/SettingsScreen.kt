package com.sona.android.app.feature.settings

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.ui.input.nestedscroll.nestedScroll
import com.sona.android.app.ui.component.InsetGroupedCard
import com.sona.android.app.ui.component.SonaCardDefaults
import com.sona.android.app.ui.component.SonaCollapsibleTopAppBar
import com.sona.android.app.ui.component.TwoLineItemDivider
import com.sona.android.app.ui.component.TwoLineItemRow
import com.sona.android.app.ui.component.springOverscroll
import androidx.compose.material3.adaptive.ExperimentalMaterial3AdaptiveApi
import androidx.compose.material3.adaptive.layout.AnimatedPane
import androidx.compose.material3.adaptive.layout.ListDetailPaneScaffoldRole
import androidx.compose.material3.adaptive.layout.PaneAdaptedValue
import androidx.compose.material3.adaptive.layout.ThreePaneScaffoldDestinationItem
import androidx.compose.material3.adaptive.navigation.NavigableListDetailPaneScaffold
import androidx.compose.material3.adaptive.navigation.rememberListDetailPaneScaffoldNavigator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.ui.component.SonaBackButton
import com.sona.android.app.ui.component.SonaTopAppBar
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
import com.sona.android.application.sync.DiscoveredVaultSummary
import com.sona.android.application.sync.SyncPairingPayload
import com.sona.android.application.sync.SyncConflictResolution
import com.sona.android.application.sync.SyncPreset
import com.sona.android.application.sync.WebDavSyncProvider
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
internal fun SettingsScreen(
    initialSection: SettingsSection?,
    bootstrapState: SonaBootstrapUiState,
    appearanceState: AppearanceSettingsUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    syncState: SyncSettingsUiState,
    dataRecoveryState: DataRecoveryUiState,
    aboutState: AboutSettingsUiState,
    llmState: LlmSettingsUiState = LlmSettingsUiState(),
    appLanguage: AppLanguage,
    requestCloudCredentialFocus: Boolean,
    onCloudCredentialFocusConsumed: () -> Unit,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onAboutShown: () -> Unit,
    onCheckForUpdates: () -> Unit,
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
    onLlmProvider: (String) -> Unit = {}, onLlmModel: (String) -> Unit = {}, onLlmBaseUrl: (String) -> Unit = {}, onLlmPath: (String) -> Unit = {}, onLlmVersion: (String) -> Unit = {}, onLlmApiKey: (String) -> Unit = {}, onLlmSave: () -> Unit = {}, onLlmClear: () -> Unit = {},
) {
    val initialDestinationHistory = remember(initialSection) {
        settingsDestinationHistory(initialSection)
    }
    val navigator = rememberListDetailPaneScaffoldNavigator<SettingsSection>(
        initialDestinationHistory = initialDestinationHistory,
    )
    val scope = rememberCoroutineScope()
    var selectedSectionRoute by rememberSaveable {
        mutableStateOf(initialSection?.route ?: SettingsSection.APPEARANCE.route)
    }
    var cloudCredentialFocusSessionActive by remember {
        mutableStateOf(requestCloudCredentialFocus)
    }
    val currentOnCloudCredentialFocusConsumed by rememberUpdatedState(
        onCloudCredentialFocusConsumed,
    )
    val selectedSection = SettingsSection.fromRoute(selectedSectionRoute)
        ?: SettingsSection.APPEARANCE
    val listPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.List] ==
        PaneAdaptedValue.Expanded
    val detailPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.Detail] ==
        PaneAdaptedValue.Expanded
    val isTwoPane = listPaneVisible && detailPaneVisible
    val canNavigateBack = !listPaneVisible && navigator.canNavigateBack()
    val consumeCloudCredentialFocusSession = {
        if (cloudCredentialFocusSessionActive) {
            cloudCredentialFocusSessionActive = false
            onCloudCredentialFocusConsumed()
        }
    }
    val navigateBack: () -> Unit = {
        consumeCloudCredentialFocusSession()
        scope.launch { navigator.navigateBack() }
    }

    LaunchedEffect(requestCloudCredentialFocus) {
        if (requestCloudCredentialFocus) {
            cloudCredentialFocusSessionActive = true
        }
    }
    DisposableEffect(Unit) {
        onDispose { currentOnCloudCredentialFocusConsumed() }
    }
    LaunchedEffect(initialSection) {
        initialSection?.let { section ->
            selectedSectionRoute = section.route
            val currentDestination = navigator.currentDestination
            if (
                currentDestination?.pane != ListDetailPaneScaffoldRole.Detail ||
                currentDestination.contentKey != section
            ) {
                navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, section)
            }
        }
    }
    BackHandler(enabled = canNavigateBack) {
        navigateBack()
    }

    NavigableListDetailPaneScaffold(
        modifier = Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background),
        navigator = navigator,
        listPane = {
            AnimatedPane {
                SettingsSectionList(
                    selectedSection = selectedSection,
                    showSelection = isTwoPane,
                    onSectionSelected = { section ->
                        if (section != SettingsSection.RECOGNITION) {
                            consumeCloudCredentialFocusSession()
                        }
                        selectedSectionRoute = section.route
                        scope.launch {
                            navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, section)
                        }
                    },
                )
            }
        },
        detailPane = {
            AnimatedPane {
                SettingsDetailPane(
                    section = selectedSection,
                    showBack = canNavigateBack,
                    bootstrapState = bootstrapState,
                    appearanceState = appearanceState,
                    cloudTranscriptionState = cloudTranscriptionState,
                    recognitionSettingsState = recognitionSettingsState,
                    syncState = syncState,
                    dataRecoveryState = dataRecoveryState,
                    aboutState = aboutState,
                    llmState = llmState,
                    appLanguage = appLanguage,
                    requestCloudCredentialFocus = cloudCredentialFocusSessionActive,
                    onBack = navigateBack,
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
                    onResumeRecovery = onResumeRecovery,
                    onResumeAllRecovery = onResumeAllRecovery,
                    onDiscardRecovery = onDiscardRecovery,
                    onClearResolvedRecovery = onClearResolvedRecovery,
                    onLlmProvider = onLlmProvider, onLlmModel = onLlmModel, onLlmBaseUrl = onLlmBaseUrl, onLlmPath = onLlmPath, onLlmVersion = onLlmVersion, onLlmApiKey = onLlmApiKey, onLlmSave = onLlmSave, onLlmClear = onLlmClear,
                )
            }
        },
    )
}

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
internal fun settingsDestinationHistory(
    initialSection: SettingsSection?,
): List<ThreePaneScaffoldDestinationItem<SettingsSection>> {
    val listDestination = ThreePaneScaffoldDestinationItem<SettingsSection>(
        pane = ListDetailPaneScaffoldRole.List,
        contentKey = null,
    )
    return if (initialSection == null) {
        listOf(listDestination)
    } else {
        listOf(
            listDestination,
            ThreePaneScaffoldDestinationItem(
                pane = ListDetailPaneScaffoldRole.Detail,
                contentKey = initialSection,
            ),
        )
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsSectionList(
    selectedSection: SettingsSection,
    showSelection: Boolean,
    onSectionSelected: (SettingsSection) -> Unit,
) {
    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()

    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background)
            .nestedScroll(scrollBehavior.nestedScrollConnection),
    ) {
        SonaCollapsibleTopAppBar(
            title = stringResource(R.string.destination_settings),
            scrollBehavior = scrollBehavior,
        )
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .springOverscroll(scrollBehavior = scrollBehavior),
            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp),
        ) {
            item {
                InsetGroupedCard(
                    shape = SonaCardDefaults.CardShape,
                ) {
                    SettingsSection.entries.forEachIndexed { index, section ->
                        val selected = showSelection && section == selectedSection
                        TwoLineItemRow(
                            headline = stringResource(section.labelRes),
                            supportingText = stringResource(section.summaryRes),
                            leadingContent = {
                                Surface(
                                    shape = RoundedCornerShape(12.dp),
                                    color = if (selected) {
                                        MaterialTheme.colorScheme.primary
                                    } else {
                                        MaterialTheme.colorScheme.primaryContainer
                                    },
                                    contentColor = if (selected) {
                                        MaterialTheme.colorScheme.onPrimary
                                    } else {
                                        MaterialTheme.colorScheme.onPrimaryContainer
                                    },
                                    modifier = Modifier.size(40.dp),
                                ) {
                                    Box(contentAlignment = Alignment.Center) {
                                        Icon(
                                            imageVector = section.icon,
                                            contentDescription = null,
                                            modifier = Modifier.size(20.dp),
                                        )
                                    }
                                }
                            },
                            showDefaultTrailingChevron = !showSelection,
                            onClick = { onSectionSelected(section) },
                        )
                        if (index < SettingsSection.entries.lastIndex) {
                            TwoLineItemDivider(startIndent = 72.dp)
                        }
                    }
                }
                Spacer(modifier = Modifier.size(100.dp))
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsDetailPane(
    section: SettingsSection,
    showBack: Boolean,
    bootstrapState: SonaBootstrapUiState,
    appearanceState: AppearanceSettingsUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    syncState: SyncSettingsUiState,
    dataRecoveryState: DataRecoveryUiState,
    aboutState: AboutSettingsUiState,
    llmState: LlmSettingsUiState,
    appLanguage: AppLanguage,
    requestCloudCredentialFocus: Boolean,
    onBack: () -> Unit,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onAboutShown: () -> Unit,
    onCheckForUpdates: () -> Unit,
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
    onLlmProvider: (String) -> Unit, onLlmModel: (String) -> Unit, onLlmBaseUrl: (String) -> Unit, onLlmPath: (String) -> Unit, onLlmVersion: (String) -> Unit, onLlmApiKey: (String) -> Unit, onLlmSave: () -> Unit, onLlmClear: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background)) {
        SonaTopAppBar(
            title = stringResource(section.labelRes),
            navigationIcon = {
                if (showBack) {
                    SonaBackButton(onClick = onBack)
                }
            },
        )
        when (section) {
            SettingsSection.APPEARANCE -> AppearanceSettingsPane(
                state = appearanceState,
                appLanguage = appLanguage,
                onAppLanguageChanged = onAppLanguageChanged,
                onDynamicColorChanged = onDynamicColorChanged,
                modifier = Modifier.weight(1f),
            )
            SettingsSection.RECOGNITION -> RecognitionSettingsPane(
                bootstrapState = bootstrapState,
                cloudTranscriptionState = cloudTranscriptionState,
                recognitionSettingsState = recognitionSettingsState,
                requestCloudCredentialFocus = requestCloudCredentialFocus,
                onCloudProviderSelected = onCloudProviderSelected,
                onCloudApiKeyInputChanged = onCloudApiKeyInputChanged,
                onSaveCloudApiKey = onSaveCloudApiKey,
                onClearCloudApiKey = onClearCloudApiKey,
                onSelectModel = onSelectModel,
                onDownloadLocalModel = onDownloadLocalModel,
                onValidateLocalModel = onValidateLocalModel,
                onDeleteLocalModel = onDeleteLocalModel,
                onRefreshRecognitionCatalog = onRefreshRecognitionCatalog,
                modifier = Modifier.weight(1f),
            )
            SettingsSection.LLM -> LlmSettingsPane(llmState, onLlmProvider, onLlmModel, onLlmBaseUrl, onLlmPath, onLlmVersion, onLlmApiKey, onLlmSave, onLlmClear, Modifier.weight(1f))
            SettingsSection.SYNC -> SyncSettingsPane(
                state = syncState,
                onRefresh = onRefreshSync,
                onTestProvider = onTestSyncProvider,
                onCreate = onCreateSync,
                onPreviewJoin = onPreviewSyncJoin,
                onJoin = onJoinSync,
                onUnlock = onUnlockSync,
                onUnlockWithRecovery = onUnlockSyncWithRecovery,
                onRunNow = onRunSync,
                onSetPaused = onPauseSync,
                onLock = onLockSync,
                onDisconnect = onDisconnectSync,
                onGenerateRecoveryKey = onGenerateSyncRecoveryKey,
                onExportRecoveryKey = onExportSyncRecoveryKey,
                onConsumeRecoveryKey = onConsumeSyncRecoveryKey,
                onResolveConflict = onResolveSyncConflict,
                onLoadConflict = onLoadSyncConflict,
                onChangePreset = onChangeSyncPreset,
                onChangePassword = onChangeSyncPassword,
                onDiscoverVaults = onDiscoverSyncVaults,
                onCreateWithVaultId = onCreateSyncWithVaultId,
                onApplyPairingToken = onApplySyncPairingToken,
                onClearPairingNotice = onClearSyncPairingNotice,
                onGetPairingToken = onGetSyncPairingToken,
                modifier = Modifier.weight(1f),
            )
            SettingsSection.DATA_RECOVERY -> DataRecoveryPane(
                state = dataRecoveryState,
                onExportBackup = onExportBackup,
                onInspectBackup = onInspectBackup,
                onConfirmImport = onConfirmBackupImport,
                onCancelImport = onCancelBackupImport,
                onRefresh = onRefreshRecovery,
                onResume = onResumeRecovery,
                onResumeAll = onResumeAllRecovery,
                onDiscard = onDiscardRecovery,
                onClearResolved = onClearResolvedRecovery,
                modifier = Modifier.weight(1f),
            )
            SettingsSection.ABOUT -> AboutSettingsPane(
                state = aboutState,
                onShown = onAboutShown,
                onCheckForUpdates = onCheckForUpdates,
                modifier = Modifier.weight(1f),
            )
        }
    }
}
