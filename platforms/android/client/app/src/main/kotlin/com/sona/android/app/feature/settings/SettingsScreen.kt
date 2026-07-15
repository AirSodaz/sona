package com.sona.android.app.feature.settings

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.ListItem
import androidx.compose.material3.ListItemDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
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
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import kotlinx.coroutines.launch

@OptIn(ExperimentalMaterial3AdaptiveApi::class)
@Composable
internal fun SettingsScreen(
    initialSection: SettingsSection?,
    bootstrapState: SonaBootstrapUiState,
    appearanceState: AppearanceSettingsUiState,
    credentialState: CredentialSettingsUiState,
    appLanguage: AppLanguage,
    requestCredentialFocus: Boolean,
    onCredentialFocusConsumed: () -> Unit,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onCredentialInputChanged: (String) -> Unit,
    onSaveCredential: () -> Unit,
    onClearCredential: () -> Unit,
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
    var credentialFocusSessionActive by remember {
        mutableStateOf(requestCredentialFocus)
    }
    val currentOnCredentialFocusConsumed by rememberUpdatedState(onCredentialFocusConsumed)
    val selectedSection = SettingsSection.fromRoute(selectedSectionRoute)
        ?: SettingsSection.APPEARANCE
    val listPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.List] ==
        PaneAdaptedValue.Expanded
    val detailPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.Detail] ==
        PaneAdaptedValue.Expanded
    val isTwoPane = listPaneVisible && detailPaneVisible
    val canNavigateBack = !listPaneVisible && navigator.canNavigateBack()
    val consumeCredentialFocusSession = {
        if (credentialFocusSessionActive) {
            credentialFocusSessionActive = false
            onCredentialFocusConsumed()
        }
    }
    val navigateBack: () -> Unit = {
        consumeCredentialFocusSession()
        scope.launch { navigator.navigateBack() }
    }

    LaunchedEffect(requestCredentialFocus) {
        if (requestCredentialFocus) {
            credentialFocusSessionActive = true
        }
    }
    DisposableEffect(Unit) {
        onDispose { currentOnCredentialFocusConsumed() }
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
        navigator = navigator,
        listPane = {
            AnimatedPane {
                SettingsSectionList(
                    selectedSection = selectedSection,
                    showSelection = isTwoPane,
                    onSectionSelected = { section ->
                        if (section != SettingsSection.RECOGNITION) {
                            consumeCredentialFocusSession()
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
                    credentialState = credentialState,
                    appLanguage = appLanguage,
                    requestCredentialFocus = credentialFocusSessionActive,
                    onBack = navigateBack,
                    onAppLanguageChanged = onAppLanguageChanged,
                    onDynamicColorChanged = onDynamicColorChanged,
                    onCredentialInputChanged = onCredentialInputChanged,
                    onSaveCredential = onSaveCredential,
                    onClearCredential = onClearCredential,
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
    Column(modifier = Modifier.fillMaxSize()) {
        TopAppBar(title = {
            Text(
                text = stringResource(R.string.destination_settings),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.primary
            )
        })
        LazyColumn(
            modifier = Modifier.fillMaxSize(),
            verticalArrangement = Arrangement.spacedBy(8.dp),
            contentPadding = PaddingValues(horizontal = 16.dp, vertical = 12.dp)
        ) {
            items(SettingsSection.entries, key = { it.route }) { section ->
                val selected = showSelection && section == selectedSection
                val containerColor = if (selected) {
                    MaterialTheme.colorScheme.secondaryContainer
                } else {
                    MaterialTheme.colorScheme.surfaceContainer
                }

                val contentColor = if (selected) {
                    MaterialTheme.colorScheme.onSecondaryContainer
                } else {
                    MaterialTheme.colorScheme.onSurface
                }

                Card(
                    shape = MaterialTheme.shapes.medium,
                    colors = CardDefaults.cardColors(
                        containerColor = containerColor,
                        contentColor = contentColor
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .clickable { onSectionSelected(section) }
                ) {
                    ListItem(
                        headlineContent = {
                            Text(
                                text = stringResource(section.labelRes),
                                fontWeight = FontWeight.SemiBold
                            )
                        },
                        supportingContent = {
                            Text(
                                text = stringResource(section.summaryRes),
                                color = if (selected) {
                                    MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.8f)
                                } else {
                                    MaterialTheme.colorScheme.onSurfaceVariant
                                }
                            )
                        },
                        leadingContent = {
                            Card(
                                shape = MaterialTheme.shapes.small,
                                colors = CardDefaults.cardColors(
                                    containerColor = if (selected) {
                                        MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.12f)
                                    } else {
                                        MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.7f)
                                    }
                                )
                            ) {
                                Box(
                                    modifier = Modifier.padding(8.dp),
                                    contentAlignment = Alignment.Center
                                ) {
                                    Icon(
                                        imageVector = section.icon,
                                        contentDescription = null,
                                        tint = if (selected) {
                                            MaterialTheme.colorScheme.onSecondaryContainer
                                        } else {
                                            MaterialTheme.colorScheme.onPrimaryContainer
                                        },
                                        modifier = Modifier.size(20.dp)
                                    )
                                }
                            }
                        },
                        trailingContent = {
                            if (!showSelection) {
                                Icon(
                                    imageVector = Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                                    contentDescription = null,
                                    tint = contentColor.copy(alpha = 0.6f)
                                )
                            }
                        },
                        colors = ListItemDefaults.colors(
                            containerColor = Color.Transparent,
                            headlineColor = contentColor,
                            supportingColor = contentColor
                        )
                    )
                }
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
    credentialState: CredentialSettingsUiState,
    appLanguage: AppLanguage,
    requestCredentialFocus: Boolean,
    onBack: () -> Unit,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onCredentialInputChanged: (String) -> Unit,
    onSaveCredential: () -> Unit,
    onClearCredential: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize()) {
        TopAppBar(
            title = {
                Text(
                    text = stringResource(section.labelRes),
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.SemiBold,
                    color = MaterialTheme.colorScheme.primary
                )
            },
            navigationIcon = {
                if (showBack) {
                    IconButton(onClick = onBack) {
                        Icon(
                            Icons.AutoMirrored.Rounded.ArrowBack,
                            contentDescription = stringResource(R.string.action_back),
                        )
                    }
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
                credentialState = credentialState,
                requestCredentialFocus = requestCredentialFocus,
                onCredentialInputChanged = onCredentialInputChanged,
                onSaveCredential = onSaveCredential,
                onClearCredential = onClearCredential,
                modifier = Modifier.weight(1f),
            )
        }
    }
}
