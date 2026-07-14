package com.sona.android.app.navigation

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
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
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.app.feature.library.LibraryScreen
import com.sona.android.app.feature.recording.RecordScreen
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.AppearanceSettingsUiState
import com.sona.android.app.feature.settings.CredentialSettingsUiState
import com.sona.android.app.feature.settings.SettingsScreen
import com.sona.android.app.feature.settings.SettingsSection
import com.sona.android.app.ui.theme.SonaTheme
import com.sona.android.application.recording.LiveRecordingState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SonaApp(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    appearanceState: AppearanceSettingsUiState,
    credentialState: CredentialSettingsUiState,
    appLanguage: AppLanguage,
    microphonePermissionGranted: Boolean,
    onRecordAction: () -> Unit,
    onSaveCredential: (String) -> Unit,
    onClearCredential: () -> Unit,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onRetryBootstrap: () -> Unit,
) {
    SonaTheme(dynamicColorEnabled = appearanceState.dynamicColorEnabled) {
        val navController = rememberNavController()
        val backStackEntry by navController.currentBackStackEntryAsState()
        val currentRoute = backStackEntry?.destination?.route ?: SonaDestination.RECORD.route
        val currentDestination = SonaDestination.entries.firstOrNull { it.matches(currentRoute) }
            ?: SonaDestination.RECORD

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
                            microphonePermissionGranted = microphonePermissionGranted,
                            onRecordAction = onRecordAction,
                            onOpenSettings = {
                                navController.navigate(settingsRoute(SettingsSection.RECOGNITION)) {
                                    launchSingleTop = true
                                }
                            },
                            onRetryBootstrap = onRetryBootstrap,
                        )
                    }
                    composable(SonaDestination.LIBRARY.route) {
                        LibraryScreen()
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
                            onAppLanguageChanged = onAppLanguageChanged,
                            onDynamicColorChanged = onDynamicColorChanged,
                            onSaveCredential = onSaveCredential,
                            onClearCredential = onClearCredential,
                        )
                    }
                }
            }
        }
    }
}

internal fun settingsRoute(section: SettingsSection): String =
    "${SonaDestination.SETTINGS.route}?$SETTINGS_SECTION_ARGUMENT=${section.route}"
