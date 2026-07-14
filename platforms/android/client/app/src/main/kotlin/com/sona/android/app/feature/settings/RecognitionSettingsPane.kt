package com.sona.android.app.feature.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.recording.CredentialStatus

@Composable
internal fun RecognitionSettingsPane(
    bootstrapState: SonaBootstrapUiState,
    credentialState: CredentialSettingsUiState,
    onSaveCredential: (String) -> Unit,
    onClearCredential: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val initialFocusRequester = remember { FocusRequester() }
    val keyboardController = LocalSoftwareKeyboardController.current
    LaunchedEffect(Unit) {
        initialFocusRequester.requestFocus()
        keyboardController?.hide()
    }

    Box(
        modifier = modifier
            .fillMaxSize()
            .focusRequester(initialFocusRequester)
            .focusTarget(),
    ) {
        Column(
            modifier = Modifier
                .widthIn(max = 720.dp)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 20.dp)
                .align(Alignment.TopCenter),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            CredentialSettings(
                state = credentialState,
                onSave = onSaveCredential,
                onClear = onClearCredential,
            )
            HorizontalDivider()
            Text(
                text = stringResource(R.string.runtime_heading),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            RuntimeStatus(bootstrapState)
        }
    }
}

@Composable
private fun CredentialSettings(
    state: CredentialSettingsUiState,
    onSave: (String) -> Unit,
    onClear: () -> Unit,
) {
    var apiKey by remember { mutableStateOf("") }
    var apiKeyVisible by rememberSaveable { mutableStateOf(false) }
    LaunchedEffect(state.status, state.operationInProgress) {
        if (state.status == CredentialStatus.CONFIGURED && !state.operationInProgress) {
            apiKey = ""
        }
    }

    Text(
        text = stringResource(R.string.online_credentials_heading),
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
    )
    Text(
        text = when (state.status) {
            null -> stringResource(R.string.status_loading)
            CredentialStatus.CONFIGURED -> stringResource(R.string.credentials_configured)
            CredentialStatus.NOT_CONFIGURED -> stringResource(R.string.credentials_not_configured)
        },
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    OutlinedTextField(
        value = apiKey,
        onValueChange = { apiKey = it },
        enabled = !state.operationInProgress,
        modifier = Modifier.fillMaxWidth(),
        label = { Text(stringResource(R.string.api_key_label)) },
        singleLine = true,
        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password),
        visualTransformation = if (apiKeyVisible) {
            VisualTransformation.None
        } else {
            PasswordVisualTransformation()
        },
        trailingIcon = {
            IconButton(onClick = { apiKeyVisible = !apiKeyVisible }) {
                Icon(
                    imageVector = if (apiKeyVisible) {
                        Icons.Rounded.VisibilityOff
                    } else {
                        Icons.Rounded.Visibility
                    },
                    contentDescription = stringResource(
                        if (apiKeyVisible) R.string.hide_api_key else R.string.show_api_key,
                    ),
                )
            }
        },
    )
    if (state.hasError) {
        Text(
            text = stringResource(R.string.credentials_error),
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
        )
    }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
    ) {
        if (state.status == CredentialStatus.CONFIGURED) {
            TextButton(
                onClick = onClear,
                enabled = !state.operationInProgress,
            ) {
                Icon(Icons.Rounded.DeleteOutline, contentDescription = null)
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.action_clear))
            }
        }
        Button(
            onClick = { onSave(apiKey) },
            enabled = apiKey.isNotBlank() && !state.operationInProgress,
        ) {
            Icon(
                imageVector = Icons.Rounded.Save,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.action_save))
        }
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
