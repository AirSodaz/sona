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
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.DeleteOutline
import androidx.compose.material.icons.rounded.Download
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExposedDropdownMenuBox
import androidx.compose.material3.ExposedDropdownMenuDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.RadioButton
import androidx.compose.material3.OutlinedTextField
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
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.bootstrap.SonaBootstrapUiState
import com.sona.android.application.recording.LocalAsrDeviceTier
import com.sona.android.application.recording.LocalAsrDownloadStage
import com.sona.android.application.recording.AsrMode
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AsrSelectionSlot
import com.sona.android.application.recording.OnlineAsrProvider
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun RecognitionSettingsPane(
    bootstrapState: SonaBootstrapUiState,
    cloudTranscriptionState: CloudTranscriptionSettingsUiState,
    recognitionSettingsState: RecognitionSettingsUiState,
    requestCloudCredentialFocus: Boolean,
    onCloudProviderSelected: (OnlineAsrProvider) -> Unit,
    onCloudApiKeyInputChanged: (String) -> Unit,
    onSaveCloudApiKey: () -> Unit,
    onClearCloudApiKey: () -> Unit,
    onSelectModel: (AsrSelectionSlot, AsrModelSelection?) -> Unit,
    onDownloadLocalModel: (String) -> Unit,
    onValidateLocalModel: (String) -> Unit,
    onDeleteLocalModel: (String) -> Unit,
    onRefreshRecognitionCatalog: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val initialFocusRequester = remember { FocusRequester() }
    val keyboardController = LocalSoftwareKeyboardController.current
    var focusInitialized by remember { mutableStateOf(false) }

    LaunchedEffect(requestCloudCredentialFocus) {
        if (!requestCloudCredentialFocus && !focusInitialized) {
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
                LocalRecognitionSettings(
                    state = recognitionSettingsState,
                    configuredProviders = cloudTranscriptionState.configuredProviders,
                    onSelectModel = onSelectModel,
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
                        requestApiKeyFocus = requestCloudCredentialFocus,
                        onProviderSelected = onCloudProviderSelected,
                        onApiKeyInputChanged = onCloudApiKeyInputChanged,
                        onSave = onSaveCloudApiKey,
                        onClear = onClearCloudApiKey,
                    )
                }
            }

            if (shouldShowRuntimeStatus(bootstrapState)) {
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
}

@Composable
private fun LocalRecognitionSettings(
    state: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
    onSelectModel: (AsrSelectionSlot, AsrModelSelection?) -> Unit,
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

        ModelSelectionDropdown(
            label = stringResource(R.string.recognition_live_model),
            slot = AsrSelectionSlot.LIVE,
            mode = AsrMode.STREAMING,
            selected = state.liveSelection,
            state = state,
            configuredProviders = configuredProviders,
            enabled = !busy,
            onSelect = onSelectModel,
        )
        ModelSelectionDropdown(
            label = stringResource(R.string.recognition_batch_model),
            slot = AsrSelectionSlot.BATCH,
            mode = AsrMode.BATCH,
            selected = state.batchSelection,
            state = state,
            configuredProviders = configuredProviders,
            enabled = !busy,
            onSelect = onSelectModel,
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

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun ModelSelectionDropdown(
    label: String,
    slot: AsrSelectionSlot,
    mode: AsrMode,
    selected: AsrModelSelection?,
    state: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
    enabled: Boolean,
    onSelect: (AsrSelectionSlot, AsrModelSelection?) -> Unit,
) {
    var expanded by remember { mutableStateOf(false) }
    val localOptions = state.installedModels
        .filter { it.supports(mode) }
        .map { AsrModelSelection.Local(it.id) to it.displayName }
    val onlineOptions = OnlineAsrProvider.entries
        .filter { provider ->
            provider.supports(mode) &&
                (provider in configuredProviders || selected == AsrModelSelection.Online(provider))
        }
        .map { provider -> AsrModelSelection.Online(provider) to provider.displayLabel() }
    val options = localOptions + onlineOptions
    val selectedLabel = options.firstOrNull { it.first == selected }?.second
        ?: stringResource(R.string.recognition_model_not_selected)

    ExposedDropdownMenuBox(
        expanded = expanded,
        onExpandedChange = { if (enabled) expanded = !expanded },
    ) {
        OutlinedTextField(
            value = selectedLabel,
            onValueChange = {},
            readOnly = true,
            enabled = enabled,
            label = { Text(label) },
            trailingIcon = { ExposedDropdownMenuDefaults.TrailingIcon(expanded) },
            modifier = Modifier.menuAnchor().fillMaxWidth(),
        )
        ExposedDropdownMenu(expanded = expanded, onDismissRequest = { expanded = false }) {
            DropdownMenuItem(
                text = { Text(stringResource(R.string.recognition_model_not_selected)) },
                onClick = {
                    expanded = false
                    onSelect(slot, null)
                },
            )
            options.forEach { (selection, optionLabel) ->
                DropdownMenuItem(
                    text = { Text(optionLabel) },
                    onClick = {
                        expanded = false
                        onSelect(slot, selection)
                    },
                )
            }
        }
    }
}

@Composable
private fun OnlineAsrProvider.displayLabel(): String = when (this) {
    OnlineAsrProvider.VOLCENGINE_DOUBAO -> stringResource(R.string.batch_provider_volcengine_doubao)
    OnlineAsrProvider.GROQ_WHISPER -> stringResource(R.string.batch_provider_groq_whisper)
    OnlineAsrProvider.MISTRAL_VOXTRAL -> stringResource(R.string.batch_provider_mistral_voxtral)
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
private fun RuntimeStatus(bootstrapState: SonaBootstrapUiState) {
    val text = when (bootstrapState) {
        SonaBootstrapUiState.Loading -> stringResource(R.string.status_loading)
        is SonaBootstrapUiState.Error -> stringResource(R.string.status_error)
        is SonaBootstrapUiState.Ready -> stringResource(R.string.local_runtime_unavailable)
    }

    val statusColor = when (bootstrapState) {
        is SonaBootstrapUiState.Ready -> MaterialTheme.colorScheme.onSurfaceVariant
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
                    is SonaBootstrapUiState.Ready -> Icons.Rounded.WarningAmber
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

internal fun shouldShowRuntimeStatus(bootstrapState: SonaBootstrapUiState): Boolean =
    bootstrapState !is SonaBootstrapUiState.Ready ||
        !bootstrapState.snapshot.localRuntimePackaged
