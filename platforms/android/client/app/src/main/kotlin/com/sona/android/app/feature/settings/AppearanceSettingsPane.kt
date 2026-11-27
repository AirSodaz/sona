package com.sona.android.app.feature.settings

import android.os.Build
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import com.sona.android.app.ui.component.InsetGroupedCard
import com.sona.android.app.ui.component.SonaOptionItem
import com.sona.android.app.ui.component.SonaOptionSettingRow
import com.sona.android.app.ui.component.SonaSingleChoiceDialog
import com.sona.android.app.ui.component.SonaSwitch
import com.sona.android.app.ui.component.springOverscroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun AppearanceSettingsPane(
    state: AppearanceSettingsUiState,
    appLanguage: AppLanguage,
    onAppLanguageChanged: (AppLanguage) -> Unit,
    onDynamicColorChanged: (Boolean) -> Unit,
    modifier: Modifier = Modifier,
    extraBottomPadding: Dp = 0.dp,
) {
    val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
    Box(modifier = modifier.fillMaxSize()) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxWidth()
                .springOverscroll()
                .verticalScroll(rememberScrollState())
                .padding(
                    start = 24.dp,
                    end = 24.dp,
                    top = 20.dp,
                    bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
                )
                .align(Alignment.TopCenter),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            InsetGroupedCard(
                modifier = Modifier.fillMaxWidth(),
            ) {
                LanguageSelector(
                    selectedLanguage = appLanguage,
                    onLanguageChanged = onAppLanguageChanged,
                )
            }
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                val enabled = state.isLoaded && !state.operationInProgress
                InsetGroupedCard(
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(
                        modifier = Modifier.padding(16.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.appearance_color_heading),
                            style = MaterialTheme.typography.titleMedium,
                            fontWeight = FontWeight.SemiBold,
                            color = MaterialTheme.colorScheme.primary,
                        )
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .clickable(enabled = enabled) {
                                    onDynamicColorChanged(!state.dynamicColorEnabled)
                                }
                                .padding(vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.SpaceBetween,
                        ) {
                            Column(modifier = Modifier.weight(1f)) {
                                Text(
                                    text = stringResource(R.string.dynamic_color),
                                    style = MaterialTheme.typography.bodyLarge,
                                    color = MaterialTheme.colorScheme.onSurface,
                                )
                                Text(
                                    text = stringResource(R.string.appearance_dynamic_color_description),
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                            Spacer(Modifier.width(16.dp))
                            SonaSwitch(
                                checked = state.dynamicColorEnabled,
                                onCheckedChange = onDynamicColorChanged,
                                enabled = enabled,
                            )
                        }
                        if (state.hasError) {
                            Text(
                                text = stringResource(R.string.appearance_settings_error),
                                color = MaterialTheme.colorScheme.error,
                                style = MaterialTheme.typography.bodySmall,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LanguageSelector(
    selectedLanguage: AppLanguage,
    onLanguageChanged: (AppLanguage) -> Unit,
) {
    var dialogVisible by rememberSaveable { mutableStateOf(false) }
    val languageOptions = AppLanguage.entries.map { language ->
        SonaOptionItem(
            value = language,
            label = stringResource(language.labelRes),
        )
    }

    SonaOptionSettingRow(
        headline = stringResource(R.string.language_label),
        selectedValueText = stringResource(selectedLanguage.labelRes),
        onClick = { dialogVisible = true },
    )

    if (dialogVisible) {
        SonaSingleChoiceDialog(
            title = stringResource(R.string.language_label),
            options = languageOptions,
            selectedValue = selectedLanguage,
            onSelect = { language ->
                if (language != selectedLanguage) {
                    onLanguageChanged(language)
                }
            },
            onDismiss = { dialogVisible = false },
        )
    }
}
