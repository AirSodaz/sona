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
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.app.feature.library.LibraryScreen
import com.sona.android.app.feature.recording.RecordScreen
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.feature.settings.CredentialSettingsUiState
import com.sona.android.app.feature.settings.SettingsScreen
import com.sona.android.app.ui.theme.SonaTheme
import com.sona.android.application.recording.LiveRecordingState

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun SonaApp(
    bootstrapState: SonaBootstrapUiState,
    recordingState: LiveRecordingState,
    credentialState: CredentialSettingsUiState,
    appLanguage: AppLanguage,
    microphonePermissionGranted: Boolean,
    onRecordAction: () -> Unit,
    onSaveCredential: (String) -> Unit,
    onClearCredential: () -> Unit,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onRetryBootstrap: () -> Unit,
) {
    var dynamicColorEnabled by rememberSaveable { mutableStateOf(false) }

    SonaTheme(dynamicColorEnabled = dynamicColorEnabled) {
        val navController = rememberNavController()
        val backStackEntry by navController.currentBackStackEntryAsState()
        val currentRoute = backStackEntry?.destination?.route ?: SonaDestination.RECORD.route
        val currentDestination = SonaDestination.entries.firstOrNull { it.route == currentRoute }
            ?: SonaDestination.RECORD

        NavigationSuiteScaffold(
            navigationSuiteItems = {
                SonaDestination.entries.forEach { destination ->
                    item(
                        selected = currentRoute == destination.route,
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
                    TopAppBar(
                        title = {
                            Column {
                                Text(
                                    text = stringResource(R.string.app_name),
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
                                navController.navigate(SonaDestination.SETTINGS.route) {
                                    launchSingleTop = true
                                }
                            },
                            onRetryBootstrap = onRetryBootstrap,
                        )
                    }
                    composable(SonaDestination.LIBRARY.route) {
                        LibraryScreen()
                    }
                    composable(SonaDestination.SETTINGS.route) {
                        SettingsScreen(
                            bootstrapState = bootstrapState,
                            credentialState = credentialState,
                            appLanguage = appLanguage,
                            dynamicColorEnabled = dynamicColorEnabled,
                            onAppLanguageChanged = onAppLanguageChanged,
                            onDynamicColorChanged = { dynamicColorEnabled = it },
                            onSaveCredential = onSaveCredential,
                            onClearCredential = onClearCredential,
                        )
                    }
                }
            }
        }
    }
}
