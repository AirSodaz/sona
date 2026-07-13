package com.sona.android.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledIconButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.adaptive.navigationsuite.NavigationSuiteScaffold
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.currentBackStackEntryAsState
import androidx.navigation.compose.rememberNavController
import com.sona.android.app.ui.theme.SonaTheme

@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SonaApp(
    bootstrapState: SonaBootstrapUiState,
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
                            onRetryBootstrap = onRetryBootstrap,
                        )
                    }
                    composable(SonaDestination.LIBRARY.route) {
                        EmptyLibraryScreen()
                    }
                    composable(SonaDestination.SETTINGS.route) {
                        SettingsScreen(
                            bootstrapState = bootstrapState,
                            dynamicColorEnabled = dynamicColorEnabled,
                            onDynamicColorChanged = { dynamicColorEnabled = it },
                        )
                    }
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun RecordScreen(
    bootstrapState: SonaBootstrapUiState,
    onRetryBootstrap: () -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 20.dp),
    ) {
        Text(
            text = stringResource(R.string.record_heading),
            style = MaterialTheme.typography.headlineSmall,
            fontWeight = FontWeight.SemiBold,
        )
        Spacer(Modifier.height(8.dp))
        BootstrapStatus(bootstrapState, onRetryBootstrap)
        Spacer(Modifier.height(24.dp))
        HorizontalDivider()
        Spacer(Modifier.height(20.dp))
        Text(
            text = stringResource(R.string.engine_label),
            style = MaterialTheme.typography.labelLarge,
        )
        Spacer(Modifier.height(10.dp))
        SingleChoiceSegmentedButtonRow(modifier = Modifier.fillMaxWidth()) {
            SegmentedButton(
                selected = true,
                onClick = {},
                shape = SegmentedButtonDefaults.itemShape(index = 0, count = 2),
                label = { Text(stringResource(R.string.engine_online)) },
            )
            SegmentedButton(
                selected = false,
                onClick = {},
                enabled = false,
                shape = SegmentedButtonDefaults.itemShape(index = 1, count = 2),
                label = { Text(stringResource(R.string.engine_local)) },
            )
        }
        Spacer(Modifier.weight(1f))
        Column(
            modifier = Modifier.fillMaxWidth(),
            horizontalAlignment = Alignment.CenterHorizontally,
        ) {
            Text(
                text = stringResource(R.string.record_timer_idle),
                style = MaterialTheme.typography.displaySmall,
            )
            Spacer(Modifier.height(20.dp))
            FilledIconButton(
                onClick = {},
                enabled = false,
                modifier = Modifier.size(88.dp),
            ) {
                Icon(
                    imageVector = Icons.Rounded.Mic,
                    contentDescription = stringResource(R.string.record_action_description),
                    modifier = Modifier.size(34.dp),
                )
            }
            Spacer(Modifier.height(12.dp))
            Text(
                text = stringResource(R.string.status_preparing),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        Spacer(Modifier.weight(1f))
    }
}

@Composable
private fun BootstrapStatus(
    bootstrapState: SonaBootstrapUiState,
    onRetryBootstrap: () -> Unit,
) {
    when (bootstrapState) {
        SonaBootstrapUiState.Loading -> Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            Spacer(Modifier.width(10.dp))
            Text(stringResource(R.string.status_loading))
        }

        is SonaBootstrapUiState.Ready -> Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = Icons.Rounded.CheckCircle,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.primary,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(10.dp))
            Text(stringResource(R.string.status_ready))
        }

        is SonaBootstrapUiState.Error -> Row(
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.status_error),
                color = MaterialTheme.colorScheme.error,
            )
            Spacer(Modifier.width(8.dp))
            TextButton(onClick = onRetryBootstrap) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}

@Composable
private fun EmptyLibraryScreen() {
    Box(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        contentAlignment = Alignment.Center,
    ) {
        Text(
            text = stringResource(R.string.library_empty),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
}

@Composable
private fun SettingsScreen(
    bootstrapState: SonaBootstrapUiState,
    dynamicColorEnabled: Boolean,
    onDynamicColorChanged: (Boolean) -> Unit,
) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp, vertical = 20.dp),
        verticalArrangement = Arrangement.spacedBy(18.dp),
    ) {
        Text(
            text = stringResource(R.string.appearance_heading),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.SpaceBetween,
        ) {
            Text(stringResource(R.string.dynamic_color))
            Switch(
                checked = dynamicColorEnabled,
                onCheckedChange = onDynamicColorChanged,
            )
        }
        HorizontalDivider()
        Text(
            text = stringResource(R.string.runtime_heading),
            style = MaterialTheme.typography.titleMedium,
            fontWeight = FontWeight.SemiBold,
        )
        RuntimeStatus(bootstrapState)
    }
}

@Composable
private fun RuntimeStatus(bootstrapState: SonaBootstrapUiState) {
    val text = when (bootstrapState) {
        SonaBootstrapUiState.Loading -> stringResource(R.string.status_loading)
        is SonaBootstrapUiState.Error -> stringResource(R.string.status_error)
        is SonaBootstrapUiState.Ready -> if (bootstrapState.snapshot.localRuntimePackaged) {
            stringResource(R.string.local_runtime_ready)
        } else {
            stringResource(R.string.local_runtime_unavailable)
        }
    }
    Text(
        text = text,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
