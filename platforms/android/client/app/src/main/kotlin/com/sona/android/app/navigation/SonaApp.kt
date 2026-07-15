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
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
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
import com.sona.android.app.feature.recording.ForegroundRecordingLifecycleEffect
import com.sona.android.app.feature.recording.RecordScreen
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.AppearanceSettingsUiState
import com.sona.android.app.feature.settings.CredentialSettingsUiState
import com.sona.android.app.feature.settings.SettingsScreen
import com.sona.android.app.feature.settings.SettingsSection
import com.sona.android.app.ui.theme.SonaTheme
import com.sona.android.application.recording.LiveRecordingState
import java.net.URLEncoder
import java.nio.charset.StandardCharsets

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SonaApp(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    libraryState: LibraryUiState,
    appearanceState: AppearanceSettingsUiState,
    credentialState: CredentialSettingsUiState,
    appLanguage: AppLanguage,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onRetryBootstrap: () -> Unit,
    onStartRecording: () -> Unit,
    onStopRecording: () -> Unit,
    onAppBackground: () -> Unit,
    onRefreshLibrary: () -> Unit,
    onLoadMoreLibrary: () -> Unit,
    onRetryLibrary: () -> Unit,
    onLoadLibraryTranscript: (String) -> Unit,
    onCredentialInputChanged: (String) -> Unit,
    onSaveCredential: () -> Unit,
    onClearCredential: () -> Unit,
) {
    var credentialFocusRequested by remember { mutableStateOf(false) }

    ForegroundRecordingLifecycleEffect(onAppBackground)

    SonaTheme(dynamicColorEnabled = appearanceState.dynamicColorEnabled) {
        val navController = rememberNavController()
        val backStackEntry by navController.currentBackStackEntryAsState()
        val currentRoute = backStackEntry?.destination?.route ?: SonaDestination.RECORD.route
        val currentDestination = SonaDestination.entries.firstOrNull { it.matches(currentRoute) }
            ?: SonaDestination.RECORD
        val isLibraryDetail = currentRoute == LIBRARY_DETAIL_ROUTE
        val onConfigureCredential = {
            credentialFocusRequested = true
            navController.navigate(settingsRoute(SettingsSection.RECOGNITION)) {
                popUpTo(SonaDestination.RECORD.route) { saveState = true }
                launchSingleTop = true
                restoreState = true
            }
        }

        NavigationSuiteScaffold(
            navigationSuiteItems = {
                SonaDestination.entries.forEach { destination ->
                    item(
                        selected = destination.matches(currentRoute),
                        onClick = {
                            navController.navigate(destination.route) {
                                popUpTo(SonaDestination.RECORD.route) { saveState = true }
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
                topBar = {
                    if (currentDestination != SonaDestination.SETTINGS) {
                        TopAppBar(
                            navigationIcon = {
                                if (isLibraryDetail) {
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
                                        style = MaterialTheme.typography.titleMedium,
                                        fontWeight = FontWeight.SemiBold,
                                    )
                                    Text(
                                        text = stringResource(currentDestination.labelRes),
                                        style = MaterialTheme.typography.labelMedium,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            },
                        )
                    }
                },
            ) { contentPadding ->
                NavHost(
                    navController = navController,
                    startDestination = SonaDestination.RECORD.route,
                    modifier = Modifier
                        .fillMaxSize()
                        .padding(contentPadding),
                ) {
                    composable(SonaDestination.RECORD.route) {
                        RecordScreen(
                            bootstrapState = bootstrapState,
                            recordingState = recordingState,
                            credentialStatus = credentialState.status,
                            onRetryBootstrap = onRetryBootstrap,
                            onStartRecording = onStartRecording,
                            onStopRecording = onStopRecording,
                            onConfigureCredential = onConfigureCredential,
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
                            onRetry = { onLoadLibraryTranscript(historyId) },
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
                            credentialState = credentialState,
                            appLanguage = appLanguage,
                            requestCredentialFocus = credentialFocusRequested,
                            onAppLanguageChanged = onAppLanguageChanged,
                            onDynamicColorChanged = onDynamicColorChanged,
                            onCredentialInputChanged = onCredentialInputChanged,
                            onSaveCredential = onSaveCredential,
                            onClearCredential = onClearCredential,
                            onCredentialFocusConsumed = {
                                credentialFocusRequested = false
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
