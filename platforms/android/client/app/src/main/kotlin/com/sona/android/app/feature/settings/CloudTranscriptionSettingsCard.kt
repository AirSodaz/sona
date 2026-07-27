package com.sona.android.app.feature.settings

import androidx.annotation.StringRes
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Done
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.recording.CredentialStatus
import com.sona.android.application.recording.OnlineBatchProvider

@get:StringRes
internal val OnlineBatchProvider.labelRes: Int
    get() = when (this) {
        OnlineBatchProvider.VOLCENGINE_DOUBAO -> R.string.batch_provider_volcengine_doubao
        OnlineBatchProvider.GROQ_WHISPER -> R.string.batch_provider_groq_whisper
        OnlineBatchProvider.MISTRAL_VOXTRAL -> R.string.batch_provider_mistral_voxtral
    }

@Composable
internal fun CloudTranscriptionSettings(
    state: CloudTranscriptionSettingsUiState,
    onProviderSelected: (OnlineBatchProvider) -> Unit,
    onApiKeyInputChanged: (String) -> Unit,
    onSave: () -> Unit,
    onClear: () -> Unit,
) {
    var apiKeyVisible by remember { mutableStateOf(false) }

    Text(
        text = stringResource(R.string.cloud_transcription_heading),
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.primary,
    )
    Text(
        text = stringResource(R.string.cloud_transcription_summary),
        style = MaterialTheme.typography.bodyMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )

    Row(
        modifier = Modifier
            .fillMaxWidth()
            .horizontalScroll(rememberScrollState()),
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        OnlineBatchProvider.entries.forEach { provider ->
            FilterChip(
                selected = provider == state.selectedProvider,
                enabled = !state.operationInProgress,
                onClick = { onProviderSelected(provider) },
                label = { Text(stringResource(provider.labelRes)) },
                leadingIcon = if (provider in state.configuredProviders) {
                    {
                        Icon(
                            imageVector = Icons.Rounded.Done,
                            contentDescription = stringResource(R.string.credential_configured),
                            modifier = Modifier.size(16.dp),
                        )
                    }
                } else {
                    null
                },
            )
        }
    }

    val statusColor = when (state.selectedStatus) {
        CredentialStatus.CONFIGURED -> MaterialTheme.colorScheme.primary
        CredentialStatus.NOT_CONFIGURED -> MaterialTheme.colorScheme.error
    }
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            text = stringResource(R.string.credential_status_label) + ": ",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        Card(
            shape = MaterialTheme.shapes.extraSmall,
            colors = CardDefaults.cardColors(
                containerColor = statusColor.copy(alpha = 0.12f),
                contentColor = statusColor,
            ),
        ) {
            Text(
                text = when (state.selectedStatus) {
                    CredentialStatus.CONFIGURED ->
                        stringResource(R.string.credential_configured).uppercase()
                    CredentialStatus.NOT_CONFIGURED ->
                        stringResource(R.string.credential_not_configured).uppercase()
                },
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
            )
        }
    }

    OutlinedTextField(
        value = state.apiKeyInput,
        onValueChange = onApiKeyInputChanged,
        enabled = !state.operationInProgress,
        modifier = Modifier.fillMaxWidth(),
        label = { Text(stringResource(R.string.credential_api_key)) },
        singleLine = true,
        shape = MaterialTheme.shapes.medium,
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
                        if (apiKeyVisible) {
                            R.string.action_hide_credential
                        } else {
                            R.string.action_show_credential
                        },
                    ),
                )
            }
        },
    )
    if (state.operationFailed) {
        Text(
            text = stringResource(R.string.credential_operation_failed),
            color = MaterialTheme.colorScheme.error,
            style = MaterialTheme.typography.bodySmall,
        )
    }
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.spacedBy(8.dp, Alignment.End),
        verticalAlignment = Alignment.CenterVertically,
    ) {
        if (state.operationInProgress) {
            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
            Spacer(Modifier.width(4.dp))
        }
        if (state.selectedStatus == CredentialStatus.CONFIGURED) {
            OutlinedButton(
                onClick = onClear,
                enabled = !state.operationInProgress,
                colors = ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.error,
                ),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.error.copy(alpha = 0.5f)),
                shape = MaterialTheme.shapes.medium,
            ) {
                Icon(
                    imageVector = Icons.Rounded.DeleteOutline,
                    contentDescription = null,
                    modifier = Modifier.size(18.dp),
                )
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.action_clear_credential))
            }
        }
        Button(
            onClick = onSave,
            enabled = state.apiKeyInput.isNotBlank() && !state.operationInProgress,
            shape = MaterialTheme.shapes.medium,
        ) {
            Icon(
                imageVector = Icons.Rounded.Save,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
            Text(stringResource(R.string.action_save_credential))
        }
    }
}
