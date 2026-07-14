package com.sona.android.app.feature.settings

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.automirrored.rounded.KeyboardArrowRight
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
import androidx.compose.material3.adaptive.navigation.NavigableListDetailPaneScaffold
import androidx.compose.material3.adaptive.navigation.rememberListDetailPaneScaffoldNavigator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
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
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    onCredentialInputChanged: (String) -> Unit,
    onSaveCredential: () -> Unit,
    onClearCredential: () -> Unit,
    onCredentialFocusConsumed: () -> Unit,
) {
    val navigator = rememberListDetailPaneScaffoldNavigator<SettingsSection>()
    val scope = rememberCoroutineScope()
    var selectedSectionRoute by rememberSaveable {
        mutableStateOf(initialSection?.route ?: SettingsSection.APPEARANCE.route)
    }
    val selectedSection = SettingsSection.fromRoute(selectedSectionRoute)
        ?: SettingsSection.APPEARANCE
    val listPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.List] ==
        PaneAdaptedValue.Expanded
    val detailPaneVisible = navigator.scaffoldValue[ListDetailPaneScaffoldRole.Detail] ==
        PaneAdaptedValue.Expanded
    val isTwoPane = listPaneVisible && detailPaneVisible
    val canNavigateBack = !listPaneVisible && navigator.canNavigateBack()

    LaunchedEffect(initialSection) {
        initialSection?.let { section ->
            selectedSectionRoute = section.route
            navigator.navigateTo(ListDetailPaneScaffoldRole.Detail, section)
        }
    }
    BackHandler(enabled = canNavigateBack) {
        scope.launch { navigator.navigateBack() }
    }

    NavigableListDetailPaneScaffold(
        navigator = navigator,
        listPane = {
            AnimatedPane {
                SettingsSectionList(
                    selectedSection = selectedSection,
                    showSelection = isTwoPane,
                    onSectionSelected = { section ->
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
                    requestCredentialFocus = requestCredentialFocus,
                    onBack = { scope.launch { navigator.navigateBack() } },
                    onAppLanguageChanged = onAppLanguageChanged,
                    onDynamicColorChanged = onDynamicColorChanged,
                    onCredentialInputChanged = onCredentialInputChanged,
                    onSaveCredential = onSaveCredential,
                    onClearCredential = onClearCredential,
                    onCredentialFocusConsumed = onCredentialFocusConsumed,
                )
            }
        },
    )
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsSectionList(
    selectedSection: SettingsSection,
    showSelection: Boolean,
    onSectionSelected: (SettingsSection) -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize()) {
        TopAppBar(title = { Text(stringResource(R.string.destination_settings)) })
        LazyColumn(modifier = Modifier.fillMaxSize()) {
            items(SettingsSection.entries, key = { it.route }) { section ->
                val selected = showSelection && section == selectedSection
                ListItem(
                    headlineContent = { Text(stringResource(section.labelRes)) },
                    supportingContent = { Text(stringResource(section.summaryRes)) },
                    leadingContent = {
                        Icon(section.icon, contentDescription = null)
                    },
                    trailingContent = {
                        if (!showSelection) {
                            Icon(
                                Icons.AutoMirrored.Rounded.KeyboardArrowRight,
                                contentDescription = null,
                            )
                        }
                    },
                    colors = ListItemDefaults.colors(
                        containerColor = if (selected) {
                            MaterialTheme.colorScheme.secondaryContainer
                        } else {
                            MaterialTheme.colorScheme.surface
                        },
                    ),
                    modifier = Modifier.clickable { onSectionSelected(section) },
                )
                if (section != SettingsSection.entries.last()) {
                    HorizontalDivider()
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
    onCredentialFocusConsumed: () -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize()) {
        TopAppBar(
            title = { Text(stringResource(section.labelRes)) },
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
                onCredentialFocusConsumed = onCredentialFocusConsumed,
                modifier = Modifier.weight(1f),
            )
        }
    }
}
