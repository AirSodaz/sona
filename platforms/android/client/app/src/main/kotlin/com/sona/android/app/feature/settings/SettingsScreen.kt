package com.sona.android.app.feature.settings

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.EnterTransition
import androidx.compose.animation.ExitTransition
import androidx.compose.animation.togetherWith
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.Badge
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
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.navigation.SonaNavMotion
import com.sona.android.app.navigation.SonaParentPageTransitionContainer
import com.sona.android.app.navigation.SonaSubPageTransitionContainer
import com.sona.android.app.navigation.sonaSubPageEnterFromLeft
import com.sona.android.app.navigation.sonaSubPageEnterFromRight
import com.sona.android.app.navigation.sonaSubPageExitToLeft
import com.sona.android.app.navigation.sonaSubPageExitToRight
import com.sona.android.app.navigation.sonaTwoPaneSectionTransition
import com.sona.android.app.ui.component.SonaBackButton
import com.sona.android.app.ui.component.SonaTopAppBar
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.settings.AppLogLevel
import com.sona.android.application.settings.ColorPalettePreset
import com.sona.android.application.settings.ContrastLevel
import com.sona.android.application.settings.DownloadMirror
import com.sona.android.application.settings.PaletteStyle
import com.sona.android.application.settings.ThemeMode
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
    requestedSection: SettingsSection? = initialSection,
    sectionRequestToken: Int = 0,
    bootstrapState: SonaBootstrapUiState,
    appearanceState: AppearanceSettingsUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    syncState: SyncSettingsUiState,
    dataRecoveryState: DataRecoveryUiState,
    generalDiagnosticsState: GeneralDiagnosticsUiState = GeneralDiagnosticsUiState(),
    aboutState: AboutSettingsUiState,
    llmState: LlmSettingsUiState = LlmSettingsUiState(),
    appLanguage: AppLanguage,
    requestCloudCredentialFocus: Boolean,
    onCloudCredentialFocusConsumed: () -> Unit,
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
    onDownloadUpdate: () -> Unit = {},
    onCancelUpdateDownload: () -> Unit = {},
    onInstallUpdate: () -> Unit = {},
    onIgnoreUpdate: () -> Unit = {},
    onClearIgnoredUpdate: () -> Unit = {},
    onConfirmCellularUpdateDownload: () -> Unit = {},
    onDismissCellularUpdateDownload: () -> Unit = {},
    onOpenInstallPermissionSettings: () -> Unit = {},
    onDismissInstallPermissionDialog: () -> Unit = {},
    onRetryUpdateWithMirror: (DownloadMirror) -> Unit = {},
    onGeneralDiagnosticsShown: () -> Unit = {},
    onAutoCheckUpdatesChanged: (Boolean) -> Unit = {},
    onWifiOnlyUpdatesChanged: (Boolean) -> Unit = {},
    onDownloadMirrorChanged: (DownloadMirror) -> Unit = {},
    onLogLevelChanged: (AppLogLevel) -> Unit = {},
    onRefreshDiagnostics: () -> Unit = {},
    onClearTempCaches: () -> Unit = {},
    onExportLogs: (String) -> Unit = {},
    onClearLogs: () -> Unit = {},
    onBuildDiagnosticReport: () -> String = { "" },
    onDismissGeneralNotice: () -> Unit = {},
    onOpenRecognitionSettings: () -> Unit = {},
    onOpenGeneralSettings: () -> Unit = {},
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
    onReturnedToSettingsList: () -> Unit = {},
    onShowingSinglePaneDetailChanged: (Boolean) -> Unit = {},
    onShowingSinglePaneListChanged: (Boolean) -> Unit = {},
    onCloseToLibrary: (() -> Unit)? = null,
    bottomOverlayPadding: Dp = 0.dp,
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
    var handledSectionRequestToken by rememberSaveable {
        mutableIntStateOf(sectionRequestToken)
    }
    var cloudCredentialFocusSessionActive by remember {
        mutableStateOf(requestCloudCredentialFocus)
    }
    val currentOnCloudCredentialFocusConsumed by rememberUpdatedState(
        onCloudCredentialFocusConsumed,
    )
    val currentOnReturnedToSettingsList by rememberUpdatedState(onReturnedToSettingsList)
    val currentOnShowingSinglePaneDetailChanged by rememberUpdatedState(
        onShowingSinglePaneDetailChanged,
    )
    val currentOnShowingSinglePaneListChanged by rememberUpdatedState(
        onShowingSinglePaneListChanged,
    )
    val selectedSection = SettingsSection.fromRoute(selectedSectionRoute)
        ?: SettingsSection.APPEARANCE
    val currentDetailSection = navigator.currentDestination
        ?.takeIf { it.pane == ListDetailPaneScaffoldRole.Detail }
        ?.contentKey
    val activeSection = currentDetailSection ?: selectedSection
    val listPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.List] ==
        PaneAdaptedValue.Expanded
    val detailPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.Detail] ==
        PaneAdaptedValue.Expanded
    val isTwoPane = listPaneVisible && detailPaneVisible
    val showingSinglePaneList = !isTwoPane &&
        navigator.currentDestination?.pane == ListDetailPaneScaffoldRole.List
    val showingSinglePaneDetail = !isTwoPane &&
        navigator.currentDestination?.pane == ListDetailPaneScaffoldRole.Detail
    val canNavigateBack = !listPaneVisible && navigator.canNavigateBack()
    val showDetailBack = canNavigateBack || (isTwoPane && initialSection != null && onCloseToLibrary != null)
    val consumeCloudCredentialFocusSession = {
        if (cloudCredentialFocusSessionActive) {
            cloudCredentialFocusSessionActive = false
            onCloudCredentialFocusConsumed()
        }
    }
    val navigateBack: () -> Unit = {
        consumeCloudCredentialFocusSession()
        if (canNavigateBack) {
            scope.launch { navigator.navigateBack() }
        } else {
            onCloseToLibrary?.invoke()
        }
    }

    LaunchedEffect(showingSinglePaneDetail) {
        currentOnShowingSinglePaneDetailChanged(showingSinglePaneDetail)
    }

    LaunchedEffect(showingSinglePaneList) {
        currentOnShowingSinglePaneListChanged(showingSinglePaneList)
        if (showingSinglePaneList) {
            consumeCloudCredentialFocusSession()
            currentOnReturnedToSettingsList()
        }
    }

    LaunchedEffect(requestCloudCredentialFocus) {
        if (requestCloudCredentialFocus) {
            cloudCredentialFocusSessionActive = true
        }
    }
    DisposableEffect(Unit) {
        onDispose {
            currentOnCloudCredentialFocusConsumed()
            currentOnShowingSinglePaneDetailChanged(false)
            currentOnShowingSinglePaneListChanged(false)
        }
    }
    LaunchedEffect(currentDetailSection) {
        if (currentDetailSection != null && currentDetailSection.route != selectedSectionRoute) {
            selectedSectionRoute = currentDetailSection.route
        }
    }
    LaunchedEffect(initialSection, requestedSection, sectionRequestToken) {
        if (sectionRequestToken > 0 && sectionRequestToken != handledSectionRequestToken) {
            handledSectionRequestToken = sectionRequestToken
            (requestedSection ?: initialSection)?.let { section ->
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
    }

    NavigableListDetailPaneScaffold(
        modifier = Modifier.fillMaxSize().background(MaterialTheme.colorScheme.background),
        navigator = navigator,
        listPane = {
            AnimatedPane(
                enterTransition = sonaSubPageEnterFromLeft(),
                exitTransition = sonaSubPageExitToLeft(),
                boundsAnimationSpec = SonaNavMotion.boundsSpringSpec(),
            ) {
                SonaParentPageTransitionContainer(
                    scope = this,
                    enableScrim = !isTwoPane,
                ) {
                    SettingsSectionList(
                        selectedSection = activeSection,
                        showSelection = isTwoPane,
                        hasUpdateBadge = aboutState.hasActionableUpdateBadge,
                        extraBottomPadding = bottomOverlayPadding,
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
            }
        },
        detailPane = {
            AnimatedPane(
                enterTransition = sonaSubPageEnterFromRight(),
                exitTransition = sonaSubPageExitToRight(),
                boundsAnimationSpec = SonaNavMotion.boundsSpringSpec(),
            ) {
                SonaSubPageTransitionContainer(
                    scope = this,
                    enabled = !isTwoPane,
                ) {
                    SettingsDetailPane(
                        section = activeSection,
                        isTwoPane = isTwoPane,
                        showBack = showDetailBack,
                        bootstrapState = bootstrapState,
                        appearanceState = appearanceState,
                        cloudTranscriptionState = cloudTranscriptionState,
                        recognitionSettingsState = recognitionSettingsState,
                        syncState = syncState,
                        dataRecoveryState = dataRecoveryState,
                        generalDiagnosticsState = generalDiagnosticsState,
                        aboutState = aboutState,
                        llmState = llmState,
                        appLanguage = appLanguage,
                        requestCloudCredentialFocus = cloudCredentialFocusSessionActive,
                        onBack = navigateBack,
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
                        onDownloadUpdate = onDownloadUpdate,
                        onCancelUpdateDownload = onCancelUpdateDownload,
                        onInstallUpdate = onInstallUpdate,
                        onIgnoreUpdate = onIgnoreUpdate,
                        onClearIgnoredUpdate = onClearIgnoredUpdate,
                        onConfirmCellularUpdateDownload = onConfirmCellularUpdateDownload,
                        onDismissCellularUpdateDownload = onDismissCellularUpdateDownload,
                        onOpenInstallPermissionSettings = onOpenInstallPermissionSettings,
                        onDismissInstallPermissionDialog = onDismissInstallPermissionDialog,
                        onRetryUpdateWithMirror = onRetryUpdateWithMirror,
                        onGeneralDiagnosticsShown = onGeneralDiagnosticsShown,
                        onAutoCheckUpdatesChanged = onAutoCheckUpdatesChanged,
                        onWifiOnlyUpdatesChanged = onWifiOnlyUpdatesChanged,
                        onDownloadMirrorChanged = onDownloadMirrorChanged,
                        onLogLevelChanged = onLogLevelChanged,
                        onRefreshDiagnostics = onRefreshDiagnostics,
                        onClearTempCaches = onClearTempCaches,
                        onExportLogs = onExportLogs,
                        onClearLogs = onClearLogs,
                        onBuildDiagnosticReport = onBuildDiagnosticReport,
                        onDismissGeneralNotice = onDismissGeneralNotice,
                        onOpenRecognitionSettings = {
                            selectedSectionRoute = SettingsSection.RECOGNITION.route
                            val currentDestination = navigator.currentDestination
                            if (
                                currentDestination?.pane != ListDetailPaneScaffoldRole.Detail ||
                                currentDestination.contentKey != SettingsSection.RECOGNITION
                            ) {
                                scope.launch {
                                    navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, SettingsSection.RECOGNITION)
                                }
                            }
                        },
                        onOpenGeneralSettings = {
                            selectedSectionRoute = SettingsSection.GENERAL_DIAGNOSTICS.route
                            val currentDestination = navigator.currentDestination
                            if (
                                currentDestination?.pane != ListDetailPaneScaffoldRole.Detail ||
                                currentDestination.contentKey != SettingsSection.GENERAL_DIAGNOSTICS
                            ) {
                                scope.launch {
                                    navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, SettingsSection.GENERAL_DIAGNOSTICS)
                                }
                            }
                        },
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
                        onLlmProvider = onLlmProvider,
                        onLlmModel = onLlmModel,
                        onLlmBaseUrl = onLlmBaseUrl,
                        onLlmPath = onLlmPath,
                        onLlmVersion = onLlmVersion,
                        onLlmApiKey = onLlmApiKey,
                        onLlmSave = onLlmSave,
                        onLlmClear = onLlmClear,
                        extraBottomPadding = bottomOverlayPadding,
                    )
                }
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
    hasUpdateBadge: Boolean = false,
    onSectionSelected: (SettingsSection) -> Unit,
    extraBottomPadding: Dp = 0.dp,
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
        val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
        LazyColumn(
            modifier = Modifier
                .fillMaxSize()
                .springOverscroll(scrollBehavior = scrollBehavior),
            contentPadding = PaddingValues(
                start = 20.dp,
                end = 20.dp,
                top = 12.dp,
                bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
            ),
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
                                    shape = RoundedCornerShape(14.dp),
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
                                    modifier = Modifier.size(42.dp),
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
                            trailingContent = if (hasUpdateBadge && section == SettingsSection.ABOUT) {
                                {
                                    Row(
                                        verticalAlignment = Alignment.CenterVertically,
                                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                                    ) {
                                        Badge(
                                            containerColor = MaterialTheme.colorScheme.error,
                                            modifier = Modifier.size(8.dp),
                                        )
                                        if (!showSelection) {
                                            Icon(
                                                imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                                                contentDescription = null,
                                                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                                            )
                                        }
                                    }
                                }
                            } else {
                                null
                            },
                            showDefaultTrailingChevron = !showSelection,
                            onClick = { onSectionSelected(section) },
                        )
                        if (index < SettingsSection.entries.lastIndex) {
                            TwoLineItemDivider(startIndent = 74.dp)
                        }
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsDetailPane(
    section: SettingsSection,
    isTwoPane: Boolean,
    showBack: Boolean,
    bootstrapState: SonaBootstrapUiState,
    appearanceState: AppearanceSettingsUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    syncState: SyncSettingsUiState,
    dataRecoveryState: DataRecoveryUiState,
    generalDiagnosticsState: GeneralDiagnosticsUiState,
    aboutState: AboutSettingsUiState,
    llmState: LlmSettingsUiState,
    appLanguage: AppLanguage,
    requestCloudCredentialFocus: Boolean,
    onBack: () -> Unit,
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
    onDownloadUpdate: () -> Unit = {},
    onCancelUpdateDownload: () -> Unit = {},
    onInstallUpdate: () -> Unit = {},
    onIgnoreUpdate: () -> Unit = {},
    onClearIgnoredUpdate: () -> Unit = {},
    onConfirmCellularUpdateDownload: () -> Unit = {},
    onDismissCellularUpdateDownload: () -> Unit = {},
    onOpenInstallPermissionSettings: () -> Unit = {},
    onDismissInstallPermissionDialog: () -> Unit = {},
    onRetryUpdateWithMirror: (DownloadMirror) -> Unit = {},
    onGeneralDiagnosticsShown: () -> Unit = {},
    onAutoCheckUpdatesChanged: (Boolean) -> Unit = {},
    onWifiOnlyUpdatesChanged: (Boolean) -> Unit = {},
    onDownloadMirrorChanged: (DownloadMirror) -> Unit = {},
    onLogLevelChanged: (AppLogLevel) -> Unit = {},
    onRefreshDiagnostics: () -> Unit = {},
    onClearTempCaches: () -> Unit = {},
    onExportLogs: (String) -> Unit = {},
    onClearLogs: () -> Unit = {},
    onBuildDiagnosticReport: () -> String = { "" },
    onDismissGeneralNotice: () -> Unit = {},
    onOpenRecognitionSettings: () -> Unit = {},
    onOpenGeneralSettings: () -> Unit = {},
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
    onLlmProvider: (String) -> Unit,
    onLlmModel: (String) -> Unit,
    onLlmBaseUrl: (String) -> Unit,
    onLlmPath: (String) -> Unit,
    onLlmVersion: (String) -> Unit,
    onLlmApiKey: (String) -> Unit,
    onLlmSave: () -> Unit,
    onLlmClear: () -> Unit,
    extraBottomPadding: Dp = 0.dp,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background),
    ) {
        SonaTopAppBar(
            title = stringResource(section.labelRes),
            navigationIcon = {
                if (showBack) {
                    SonaBackButton(onClick = onBack)
                }
            },
        )
        val renderSectionContent: @Composable (SettingsSection, Modifier) -> Unit = { activeSection, contentModifier ->
            when (activeSection) {
                SettingsSection.APPEARANCE -> AppearanceSettingsPane(
                    state = appearanceState,
                    appLanguage = appLanguage,
                    onAppLanguageChanged = onAppLanguageChanged,
                    onThemeModeChanged = onThemeModeChanged,
                    onPureBlackDarkChanged = onPureBlackDarkChanged,
                    onSelectSystemDynamicColor = onSelectSystemDynamicColor,
                    onSelectPalettePreset = onSelectPalettePreset,
                    onCustomSeedColorChanged = onCustomSeedColorChanged,
                    onPaletteStyleChanged = onPaletteStyleChanged,
                    onContrastLevelChanged = onContrastLevelChanged,
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
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
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
                )
                SettingsSection.LLM -> LlmSettingsPane(
                    state = llmState,
                    onProvider = onLlmProvider,
                    onModel = onLlmModel,
                    onBaseUrl = onLlmBaseUrl,
                    onPath = onLlmPath,
                    onVersion = onLlmVersion,
                    onApiKey = onLlmApiKey,
                    onSave = onLlmSave,
                    onClear = onLlmClear,
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
                )
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
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
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
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
                )
                SettingsSection.GENERAL_DIAGNOSTICS -> GeneralDiagnosticsPane(
                    state = generalDiagnosticsState,
                    onShown = onGeneralDiagnosticsShown,
                    onAutoCheckUpdatesChanged = onAutoCheckUpdatesChanged,
                    onWifiOnlyUpdatesChanged = onWifiOnlyUpdatesChanged,
                    onDownloadMirrorChanged = onDownloadMirrorChanged,
                    onLogLevelChanged = onLogLevelChanged,
                    onClearIgnoredUpdate = onClearIgnoredUpdate,
                    onRefreshDiagnostics = onRefreshDiagnostics,
                    onClearTempCaches = onClearTempCaches,
                    onExportLogs = onExportLogs,
                    onClearLogs = onClearLogs,
                    onOpenRecognitionSettings = onOpenRecognitionSettings,
                    onBuildDiagnosticReport = onBuildDiagnosticReport,
                    onDismissNotice = onDismissGeneralNotice,
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
                )
                SettingsSection.ABOUT -> AboutSettingsPane(
                    state = aboutState,
                    onShown = onAboutShown,
                    onCheckForUpdates = onCheckForUpdates,
                    onDownloadUpdate = onDownloadUpdate,
                    onCancelDownload = onCancelUpdateDownload,
                    onInstallUpdate = onInstallUpdate,
                    onIgnoreUpdate = onIgnoreUpdate,
                    onClearIgnoredUpdate = onClearIgnoredUpdate,
                    onConfirmCellularDownload = onConfirmCellularUpdateDownload,
                    onDismissCellularDownload = onDismissCellularUpdateDownload,
                    onOpenInstallPermissionSettings = onOpenInstallPermissionSettings,
                    onDismissInstallPermissionDialog = onDismissInstallPermissionDialog,
                    onRetryWithMirror = onRetryUpdateWithMirror,
                    onOpenGeneralSettings = onOpenGeneralSettings,
                    extraBottomPadding = extraBottomPadding,
                    modifier = contentModifier,
                )
            }
        }
        AnimatedContent(
            targetState = section,
            transitionSpec = {
                if (isTwoPane) {
                    sonaTwoPaneSectionTransition()
                } else {
                    EnterTransition.None togetherWith ExitTransition.None
                }
            },
            label = "settingsDetailSection",
            modifier = Modifier.weight(1f),
        ) { activeSection ->
            renderSectionContent(activeSection, Modifier.fillMaxSize())
        }
    }
}
