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
import com.sona.android.app.feature.settings.AppearanceSettingsUiState
import com.sona.android.app.feature.settings.CloudTranscriptionSettingsUiState
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.app.feature.settings.SettingsScreen
import com.sona.android.app.feature.settings.SettingsSection
import com.sona.android.app.ui.theme.SonaTheme
import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
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
    appLanguage: AppLanguage,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onRetryBootstrap: () -> Unit,
    onStartRecording: () -> Unit,
    onStopRecording: () -> Unit,
    onRefreshLibrary: () -> Unit,
    onLoadMoreLibrary: () -> Unit,
    onRetryLibrary: () -> Unit,
    onLoadLibraryTranscript: (String) -> Unit,
    onTranscribeWithCloud: (RecordingLibraryItem) -> Unit,
    onImportAudio: (String) -> Unit,
    onCancelAudioImport: () -> Unit,
    onTranscribeWithCurrentEngine: (RecordingLibraryItem) -> Unit,
    onCloudProviderSelected: (OnlineAsrProvider) -> Unit,
    onCloudApiKeyInputChanged: (String) -> Unit,
    onSaveCloudApiKey: () -> Unit,
    onClearCloudApiKey: () -> Unit,
    onSelectModel: (AsrSelectionSlot, AsrModelSelection?) -> Unit,
    onDownloadLocalModel: (String) -> Unit,
    onValidateLocalModel: (String) -> Unit,
    onDeleteLocalModel: (String) -> Unit,
    onRefreshRecognitionCatalog: () -> Unit,
) {
    var cloudCredentialFocusRequested by remember { mutableStateOf(false) }

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
                            navController.navigate(destination.route) {
                                popUpTo(SonaDestination.HOME.route) { saveState = true }
                                launchSingleTop = true
                                restoreState = true
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
                                    IconButton(onClick = { navController.popBackStack() }) {
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
                            onRetry = { onLoadLibraryTranscript(historyId) },
                            onTranscribeWithCloud = onTranscribeWithCloud,
                            onTranscribeWithCurrentEngine = onTranscribeWithCurrentEngine,
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
                            appLanguage = appLanguage,
                            requestCloudCredentialFocus = cloudCredentialFocusRequested,
                            onAppLanguageChanged = onAppLanguageChanged,
                            onDynamicColorChanged = onDynamicColorChanged,
                            onCloudProviderSelected = onCloudProviderSelected,
                            onCloudApiKeyInputChanged = onCloudApiKeyInputChanged,
                            onSaveCloudApiKey = onSaveCloudApiKey,
                            onClearCloudApiKey = onClearCloudApiKey,
                            onSelectModel = onSelectModel,
                            onDownloadLocalModel = onDownloadLocalModel,
                            onValidateLocalModel = onValidateLocalModel,
                            onDeleteLocalModel = onDeleteLocalModel,
                            onRefreshRecognitionCatalog = onRefreshRecognitionCatalog,
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
