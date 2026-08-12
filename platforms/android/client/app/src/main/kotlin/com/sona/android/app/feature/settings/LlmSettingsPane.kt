package com.sona.android.app.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp

@Composable
internal fun LlmSettingsPane(state: LlmSettingsUiState, onProvider: (String) -> Unit, onModel: (String) -> Unit, onBaseUrl: (String) -> Unit, onPath: (String) -> Unit, onVersion: (String) -> Unit, onApiKey: (String) -> Unit, onSave: () -> Unit, onClear: () -> Unit, modifier: Modifier = Modifier) {
    Column(modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
        OutlinedTextField(state.providerId, onProvider, label = { Text("Provider") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.model, onModel, label = { Text("Model") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.baseUrl, onBaseUrl, label = { Text("Base URL") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.apiPath, onPath, label = { Text("API path (optional)") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField(state.apiVersion, onVersion, label = { Text("API version (optional)") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        OutlinedTextField("", onApiKey, label = { Text(if (state.hasApiKey) "API Key (stored)" else "API Key") }, modifier = Modifier.fillMaxWidth(), singleLine = true)
        if (state.error) Text("Configuration could not be saved")
        Button(onClick = onSave, enabled = !state.saving, modifier = Modifier.fillMaxWidth()) { Text(if (state.saved) "Saved" else "Save") }
        if (state.hasApiKey) Button(onClick = onClear, modifier = Modifier.fillMaxWidth()) { Text("Clear LLM configuration") }
    }
}
