package com.sona.android.app.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.material3.Text
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.setValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.Composable
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LlmSettingsPane(state: LlmSettingsUiState, onProvider: (String) -> Unit, onModel: (String) -> Unit, onBaseUrl: (String) -> Unit, onPath: (String) -> Unit, onVersion: (String) -> Unit, onApiKey: (String) -> Unit, onSave: () -> Unit, onClear: () -> Unit, modifier: Modifier = Modifier) {
    var providerMenuExpanded by remember { mutableStateOf(false) }
    Column(modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        ExposedDropdownMenuBox(expanded = providerMenuExpanded, onExpandedChange = { providerMenuExpanded = !providerMenuExpanded }) {
            OutlinedTextField(state.providerId, {}, readOnly = true, label = { Text(stringResource(com.sona.android.app.R.string.llm_provider)) }, trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(providerMenuExpanded) }, modifier = Modifier.menuAnchor().fillMaxWidth(), singleLine = true)
            ExposedDropdownMenu(expanded = providerMenuExpanded, onDismissRequest = { providerMenuExpanded = false }) {
                state.providers.forEach { provider -> DropdownMenuItem(text = { Text(provider.id) }, onClick = { onProvider(provider.id); providerMenuExpanded = false }) }
            }
        }
        OutlinedTextField(state.model, onModel, label = { Text(stringResource(com.sona.android.app.R.string.llm_model)) }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.baseUrl, onBaseUrl, label = { Text(stringResource(com.sona.android.app.R.string.llm_base_url)) }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.apiPath, onPath, label = { Text(stringResource(com.sona.android.app.R.string.llm_api_path)) }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.apiVersion, onVersion, label = { Text(stringResource(com.sona.android.app.R.string.llm_api_version)) }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField("", onApiKey, label = { Text(stringResource(if (state.hasApiKey) com.sona.android.app.R.string.llm_api_key_stored else com.sona.android.app.R.string.llm_api_key)) }, visualTransformation = PasswordVisualTransformation(), modifier = Modifier.fillMaxWidth(), singleLine = true)
        if (state.error) Text(stringResource(com.sona.android.app.R.string.llm_configuration_failed))
        Button(onClick = onSave, enabled = !state.saving, modifier = Modifier.fillMaxWidth()) { Text(stringResource(if (state.saved) com.sona.android.app.R.string.llm_saved else com.sona.android.app.R.string.llm_save)) }
        if (state.hasApiKey) Button(onClick = onClear, modifier = Modifier.fillMaxWidth()) { Text(stringResource(com.sona.android.app.R.string.llm_clear)) }
    }
}
