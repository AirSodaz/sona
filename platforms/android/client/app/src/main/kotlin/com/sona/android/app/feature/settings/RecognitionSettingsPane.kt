package com.sona.android.app.feature.settings

import androidx.compose.foundation.BorderStroke
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
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.Visibility
import androidx.compose.material.icons.rounded.VisibilityOff
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.focus.focusTarget
import androidx.compose.ui.layout.onGloballyPositioned
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
import com.sona.android.application.recording.LocalAsrDeviceTier
import com.sona.android.application.recording.LocalAsrDownloadStage
import com.sona.android.application.recording.OnlineBatchProvider
import java.util.Locale

@Composable
internal fun RecognitionSettingsPane(
    bootstrapState: SonaBootstrapUiState,
    credentialState: CredentialSettingsUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    requestCredentialFocus: Boolean,
    onCredentialInputChanged: (String) -> Unit,
    onSaveCredential: () -> Unit,
    onClearCredential: () -> Unit,
    onCloudProviderSelected: (OnlineBatchProvider) -> Unit,
    onCloudApiKeyInputChanged: (String) -> Unit,
    onSaveCloudApiKey: () -> Unit,
    onClearCloudApiKey: () -> Unit,
    onSelectLocalModel: (String) -> Unit,
    onDownloadLocalModel: (String) -> Unit,
    onValidateLocalModel: (String) -> Unit,
    onDeleteLocalModel: (String) -> Unit,
    onRefreshRecognitionCatalog: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val initialFocusRequester = remember { FocusRequester() }
    val credentialFocusRequester = remember { FocusRequester() }
    val keyboardController = LocalSoftwareKeyboardController.current
    var focusInitialized by remember { mutableStateOf(false) }
    var credentialFieldPlaced by remember { mutableStateOf(false) }
    val onCredentialFieldPlaced = remember {
        { credentialFieldPlaced = true }
    }
    LaunchedEffect(requestCredentialFocus, credentialFieldPlaced) {
        if (requestCredentialFocus && credentialFieldPlaced) {
            credentialFocusRequester.requestFocus()
            keyboardController?.show()
        } else if (!requestCredentialFocus && !focusInitialized) {
            initialFocusRequester.requestFocus()
            keyboardController?.hide()
            focusInitialized = true
        }
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
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
            Card(
                shape = MaterialTheme.shapes.medium,
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                ),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    CredentialSettings(
                        state = credentialState,
                        focusRequester = credentialFocusRequester,
                        onCredentialFieldPlaced = onCredentialFieldPlaced,
                        onCredentialInputChanged = onCredentialInputChanged,
                        onSave = onSaveCredential,
                        onClear = onClearCredential,
                    )
                }
            }

            Card(
                shape = MaterialTheme.shapes.medium,
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                ),
                modifier = Modifier.fillMaxWidth()
            ) {
                LocalRecognitionSettings(
                    state = recognitionSettingsState,
                    onSelectModel = onSelectLocalModel,
                    onDownloadModel = onDownloadLocalModel,
                    onValidateModel = onValidateLocalModel,
                    onDeleteModel = onDeleteLocalModel,
                    onRefreshCatalog = onRefreshRecognitionCatalog,
                    modifier = Modifier.padding(16.dp),
                )
            }

            Card(
                shape = MaterialTheme.shapes.medium,
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                ),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(12.dp)
                ) {
                    CloudTranscriptionSettings(
                        state = cloudTranscriptionState,
                        onProviderSelected = onCloudProviderSelected,
                        onApiKeyInputChanged = onCloudApiKeyInputChanged,
                        onSave = onSaveCloudApiKey,
                        onClear = onClearCloudApiKey,
                    )
                }
            }

            Card(
                shape = MaterialTheme.shapes.medium,
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainer
                ),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.padding(16.dp),
                    verticalArrangement = Arrangement.spacedBy(10.dp)
                ) {
                    Text(
                        text = stringResource(R.string.runtime_heading),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                        color = MaterialTheme.colorScheme.primary
                    )
                    RuntimeStatus(bootstrapState)
                }
            }
        }
    }
}

@Composable
private fun LocalRecognitionSettings(
    state: RecognitionSettingsUiState,
    onSelectModel: (String) -> Unit,
    onDownloadModel: (String) -> Unit,
    onValidateModel: (String) -> Unit,
    onDeleteModel: (String) -> Unit,
    onRefreshCatalog: () -> Unit,
    modifier: Modifier = Modifier,
) {
    var pendingDeleteModelId by remember { mutableStateOf<String?>(null) }
    val busy = state.operationModelId != null

    pendingDeleteModelId?.let { modelId ->
        val modelName = state.installedModels.firstOrNull { it.id == modelId }?.displayName.orEmpty()
        AlertDialog(
            onDismissRequest = { pendingDeleteModelId = null },
            title = { Text(stringResource(R.string.local_model_delete_title)) },
            text = { Text(stringResource(R.string.local_model_delete_message, modelName)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        pendingDeleteModelId = null
                        onDeleteModel(modelId)
                    },
                ) {
                    Text(stringResource(R.string.action_delete))
                }
            },
            dismissButton = {
                TextButton(onClick = { pendingDeleteModelId = null }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    Column(
        modifier = modifier,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.local_recognition_heading),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.weight(1f),
            )
            IconButton(onClick = onRefreshCatalog, enabled = !busy && !state.catalogLoading) {
                Icon(
                    Icons.Rounded.Refresh,
                    contentDescription = stringResource(R.string.action_refresh_model_catalog),
                )
            }
        }

        state.deviceCapabilities?.let { capabilities ->
            val tier = stringResource(
                when (capabilities.tier) {
                    LocalAsrDeviceTier.LIMITED -> R.string.local_device_tier_limited
                    LocalAsrDeviceTier.STANDARD -> R.string.local_device_tier_standard
                    LocalAsrDeviceTier.HIGH -> R.string.local_device_tier_high
                },
            )
            Text(
                text = stringResource(
                    R.string.local_device_summary,
                    tier,
                    capabilities.cpuCores.toString(),
                    formatBytes(capabilities.totalMemoryBytes),
                    formatBytes(capabilities.availableStorageBytes),
                ),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (!capabilities.supported) {
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Icon(
                        Icons.Rounded.WarningAmber,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.error,
                    )
                    Text(
                        text = stringResource(R.string.local_device_unsupported),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            }
        }

        Text(
            text = state.localModel?.let { model ->
                stringResource(R.string.local_model_installed, model.displayName)
            } ?: stringResource(R.string.local_model_not_installed),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (state.operationError) {
            Text(
                text = stringResource(R.string.local_model_operation_failed),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.error,
            )
        }

        Text(
            text = stringResource(R.string.local_model_installed_heading),
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.SemiBold,
        )
        if (state.installedModels.isEmpty()) {
            Text(
                text = stringResource(R.string.local_model_not_installed),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        state.installedModels.forEachIndexed { index, model ->
            if (index > 0) HorizontalDivider()
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                RadioButton(
                    selected = state.localModel?.id == model.id,
                    onClick = { onSelectModel(model.id) },
                    enabled = !busy,
                )
                Column(modifier = Modifier.weight(1f)) {
                    Text(model.displayName, style = MaterialTheme.typography.bodyMedium)
                    Text(
                        text = stringResource(
                            R.string.local_model_details,
                            model.config.modelType,
                            formatBytes(model.sizeBytes),
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                state.validationByModelId[model.id]?.let { valid ->
                    Icon(
                        imageVector = if (valid) Icons.Rounded.CheckCircle else Icons.Rounded.WarningAmber,
                        contentDescription = stringResource(
                            if (valid) R.string.local_model_valid else R.string.local_model_invalid,
                        ),
                        tint = if (valid) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.error,
                        modifier = Modifier.size(20.dp),
                    )
                }
                IconButton(
                    onClick = { onValidateModel(model.id) },
                    enabled = !busy,
                ) {
                    Icon(
                        Icons.Rounded.CheckCircle,
                        contentDescription = stringResource(R.string.action_validate_model),
                    )
                }
                IconButton(
                    onClick = { pendingDeleteModelId = model.id },
                    enabled = !busy,
                ) {
                    Icon(
                        Icons.Rounded.DeleteOutline,
                        contentDescription = stringResource(R.string.action_delete_model),
                    )
                }
            }
        }

        HorizontalDivider()
        Text(
            text = stringResource(R.string.local_model_catalog_heading),
            style = MaterialTheme.typography.labelLarge,
            fontWeight = FontWeight.SemiBold,
        )
        if (state.catalogLoading) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.Center,
            ) {
                CircularProgressIndicator(Modifier.size(22.dp), strokeWidth = 2.dp)
            }
        }
        state.catalogModels.forEachIndexed { index, model ->
            if (index > 0) HorizontalDivider()
            val installed = state.installedModels.any { it.id == model.id }
            val downloading = state.operationModelId == model.id && state.downloadProgress != null
            Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Column(modifier = Modifier.weight(1f)) {
                        Text(model.displayName, style = MaterialTheme.typography.bodyMedium)
                        Text(
                            text = stringResource(
                                R.string.local_catalog_model_details,
                                model.language,
                                model.sizeLabel,
                            ),
                            style = MaterialTheme.typography.bodySmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                    Button(
                        onClick = { onDownloadModel(model.id) },
                        enabled = !busy && !installed &&
                            state.deviceCapabilities?.supported != false,
                    ) {
                        Icon(Icons.Rounded.Download, contentDescription = null)
                        Spacer(Modifier.width(8.dp))
                        Text(
                            stringResource(
                                if (installed) R.string.local_model_downloaded
                                else R.string.action_download_model,
                            ),
                        )
                    }
                }
                if (downloading) {
                    val progress = checkNotNull(state.downloadProgress)
                    val fraction = if (progress.totalBytes > 0) {
                        (progress.downloadedBytes.toFloat() / progress.totalBytes).coerceIn(0f, 1f)
                    } else {
                        null
                    }
                    if (fraction == null) {
                        LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                    } else {
                        LinearProgressIndicator(
                            progress = { fraction },
                            modifier = Modifier.fillMaxWidth(),
                        )
                    }
                    Text(
                        text = stringResource(
                            when (progress.stage) {
                                LocalAsrDownloadStage.DOWNLOADING -> R.string.local_model_downloading
                                LocalAsrDownloadStage.VERIFYING -> R.string.local_model_verifying
                                LocalAsrDownloadStage.INSTALLING -> R.string.local_model_installing
                            },
                        ),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

private fun formatBytes(bytes: Long): String {
    if (bytes <= 0) return "--"
    val gib = bytes.toDouble() / (1_024 * 1_024 * 1_024)
    return if (gib >= 1) {
        String.format(Locale.getDefault(), "%.1f GB", gib)
    } else {
        String.format(Locale.getDefault(), "%.0f MB", bytes.toDouble() / (1_024 * 1_024))
    }
}

@Composable
private fun CredentialSettings(
    state: CredentialSettingsUiState,
    focusRequester: FocusRequester,
    onCredentialFieldPlaced: () -> Unit,
    onCredentialInputChanged: (String) -> Unit,
    onSave: () -> Unit,
    onClear: () -> Unit,
) {
    var apiKeyVisible by remember { mutableStateOf(false) }

    Text(
        text = stringResource(R.string.online_recognition_heading),
        style = MaterialTheme.typography.titleMedium,
        fontWeight = FontWeight.SemiBold,
        color = MaterialTheme.colorScheme.primary
    )

    val statusColor = when (state.status) {
        CredentialStatus.CONFIGURED -> MaterialTheme.colorScheme.primary
        CredentialStatus.NOT_CONFIGURED -> MaterialTheme.colorScheme.error
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = stringResource(R.string.credential_status_label) + ": ",
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Card(
            shape = MaterialTheme.shapes.extraSmall,
            colors = CardDefaults.cardColors(
                containerColor = statusColor.copy(alpha = 0.12f),
                contentColor = statusColor
            )
        ) {
            Text(
                text = when (state.status) {
                    CredentialStatus.CONFIGURED -> stringResource(R.string.credential_configured).uppercase()
                    CredentialStatus.NOT_CONFIGURED -> stringResource(R.string.credential_not_configured).uppercase()
                },
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
            )
        }
    }

    OutlinedTextField(
        value = state.credentialInput,
        onValueChange = onCredentialInputChanged,
        enabled = !state.operationInProgress,
        modifier = Modifier
            .fillMaxWidth()
            .focusRequester(focusRequester)
            .onGloballyPositioned { onCredentialFieldPlaced() },
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
        verticalAlignment = Alignment.CenterVertically
    ) {
        if (state.operationInProgress) {
            CircularProgressIndicator(Modifier.size(20.dp), strokeWidth = 2.dp)
            Spacer(Modifier.width(4.dp))
        }
        if (state.status == CredentialStatus.CONFIGURED) {
            OutlinedButton(
                onClick = onClear,
                enabled = !state.operationInProgress,
                colors = ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.error
                ),
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.error.copy(alpha = 0.5f)),
                shape = MaterialTheme.shapes.medium
            ) {
                Icon(Icons.Rounded.DeleteOutline, contentDescription = null, modifier = Modifier.size(18.dp))
                Spacer(Modifier.width(8.dp))
                Text(stringResource(R.string.action_clear_credential))
            }
        }
        Button(
            onClick = onSave,
            enabled = state.credentialInput.isNotBlank() && !state.operationInProgress,
            shape = MaterialTheme.shapes.medium
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

    val statusColor = when (bootstrapState) {
        is SonaBootstrapUiState.Ready -> if (bootstrapState.snapshot.localRuntimePackaged) {
            MaterialTheme.colorScheme.primary
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        }
        is SonaBootstrapUiState.Error -> MaterialTheme.colorScheme.error
        else -> MaterialTheme.colorScheme.primary
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        if (bootstrapState is SonaBootstrapUiState.Loading) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
        } else {
            Icon(
                imageVector = when (bootstrapState) {
                    is SonaBootstrapUiState.Error -> Icons.Rounded.WarningAmber
                    is SonaBootstrapUiState.Ready -> if (bootstrapState.snapshot.localRuntimePackaged) Icons.Rounded.CheckCircle else Icons.Rounded.WarningAmber
                    else -> Icons.Rounded.CheckCircle
                },
                contentDescription = null,
                tint = statusColor,
                modifier = Modifier.size(20.dp)
            )
        }
        Text(
            text = text,
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.onSurface
        )
    }
}
