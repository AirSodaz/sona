package com.sona.android.app.feature.home

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalWindowInfo
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AudioImportFailure
import com.sona.android.application.recording.AudioImportJobState
import com.sona.android.application.recording.AudioImportStage
import com.sona.android.application.recording.OnlineAsrProvider

@Composable
internal fun FileTranscriptionScreen(
    importState: AudioImportJobState,
    recognitionSettings: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
    onStart: (String) -> Unit,
    onCancel: () -> Unit,
    onConfigure: () -> Unit,
    onViewResult: (String) -> Unit,
) {
    val context = LocalContext.current
    val windowHeight = LocalWindowInfo.current.containerSize.height
    val verticallyConstrained = with(LocalDensity.current) { windowHeight.toDp() < 600.dp }
    val scrollState = rememberScrollState()
    var selectedLocator by rememberSaveable { mutableStateOf<String?>(null) }
    var selectedName by rememberSaveable { mutableStateOf<String?>(null) }
    var selectedSize by rememberSaveable { mutableStateOf<Long?>(null) }
    var preparingNew by rememberSaveable { mutableStateOf(false) }
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        uri?.let {
            val metadata = context.documentMetadata(it)
            selectedLocator = it.toString()
            selectedName = metadata.first
            selectedSize = metadata.second
            preparingNew = true
        }
    }
    val displayedState = if (
        preparingNew && (importState is AudioImportJobState.Completed || importState is AudioImportJobState.Failed)
    ) {
        AudioImportJobState.Idle
    } else {
        importState
    }
    val configurationAvailable = when (val selection = recognitionSettings.batchSelection) {
        is AsrModelSelection.Local -> recognitionSettings.installedModels.any {
            it.id == selection.modelId
        }
        is AsrModelSelection.Online -> selection.provider in configuredProviders
        null -> false
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .then(if (verticallyConstrained) Modifier.verticalScroll(scrollState) else Modifier)
            .padding(horizontal = 24.dp, vertical = 20.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        Text(
            stringResource(R.string.file_workspace_heading),
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.primary,
        )
        if (!configurationAvailable && displayedState is AudioImportJobState.Idle) {
            Text(
                stringResource(R.string.home_status_unavailable),
                color = MaterialTheme.colorScheme.error,
            )
            FilledTonalButton(onClick = onConfigure) {
                Text(stringResource(R.string.action_configure))
            }
        }
        Text(
            recognitionSettings.batchSelection.batchModelLabel(recognitionSettings),
            style = MaterialTheme.typography.bodyMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        when (displayedState) {
            is AudioImportJobState.Running -> RunningImport(displayedState, onCancel)
            is AudioImportJobState.Completed -> CompletedImport(
                displayedState,
                onViewResult,
                onChooseAnother = { picker.launch(arrayOf("audio/*")) },
            )
            is AudioImportJobState.Failed -> FailedImport(
                displayedState,
                selectedName,
                selectedSize,
                onRetry = { selectedLocator?.let(onStart) },
                onConfigure = onConfigure,
                onChooseAnother = { picker.launch(arrayOf("audio/*")) },
            )
            AudioImportJobState.Idle -> {
                SelectedFileCard(selectedName, selectedSize)
                if (verticallyConstrained) {
                    Spacer(Modifier.size(24.dp))
                } else {
                    Spacer(Modifier.weight(1f))
                }
                FilledTonalButton(
                    onClick = { picker.launch(arrayOf("audio/*")) },
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Icon(Icons.Rounded.AudioFile, null)
                    Spacer(Modifier.size(8.dp))
                    Text(stringResource(R.string.file_select_audio))
                }
                Button(
                    onClick = {
                        preparingNew = false
                        selectedLocator?.let(onStart)
                    },
                    enabled = selectedLocator != null && configurationAvailable,
                    modifier = Modifier.fillMaxWidth(),
                ) { Text(stringResource(R.string.file_start_transcription)) }
            }
        }
    }
}

@Composable
private fun SelectedFileCard(name: String?, sizeBytes: Long?) {
    Card(
        modifier = Modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(6.dp)) {
            Text(name ?: stringResource(R.string.file_no_selection), style = MaterialTheme.typography.titleMedium)
            sizeBytes?.let { Text(formatFileSize(it), color = MaterialTheme.colorScheme.onSurfaceVariant) }
        }
    }
}

@Composable
private fun RunningImport(state: AudioImportJobState.Running, onCancel: () -> Unit) {
    Card(Modifier.fillMaxWidth()) {
        Column(Modifier.padding(20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                CircularProgressIndicator(Modifier.size(24.dp), strokeWidth = 2.dp)
                Spacer(Modifier.size(12.dp))
                Text(state.displayName ?: stringResource(R.string.home_status_processing), Modifier.weight(1f))
                IconButton(onClick = onCancel) { Icon(Icons.Rounded.Close, stringResource(R.string.library_import_cancel)) }
            }
            state.progressPercent?.let { value ->
                LinearProgressIndicator(progress = { value / 100f }, modifier = Modifier.fillMaxWidth())
            } ?: LinearProgressIndicator(Modifier.fillMaxWidth())
            Text(stringResource(state.stage.labelRes()), style = MaterialTheme.typography.bodySmall)
        }
    }
}

@Composable
private fun CompletedImport(
    state: AudioImportJobState.Completed,
    onViewResult: (String) -> Unit,
    onChooseAnother: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        Text(
            stringResource(if (state.transcriptionWarning) R.string.library_import_saved_warning else R.string.library_import_completed),
            color = if (state.transcriptionWarning) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
        )
        Button(onClick = { onViewResult(state.historyId) }) {
            Text(stringResource(R.string.file_view_result))
        }
        FilledTonalButton(onClick = onChooseAnother) {
            Text(stringResource(R.string.file_select_another))
        }
    }
}

@Composable
private fun FailedImport(
    state: AudioImportJobState.Failed,
    selectedName: String?,
    selectedSize: Long?,
    onRetry: () -> Unit,
    onConfigure: () -> Unit,
    onChooseAnother: () -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        SelectedFileCard(selectedName, selectedSize)
        Text(stringResource(R.string.home_status_failed), color = MaterialTheme.colorScheme.error)
        if (state.reason == AudioImportFailure.CONFIGURATION) {
            FilledTonalButton(onClick = onConfigure) { Text(stringResource(R.string.action_configure)) }
        } else {
            Button(onClick = onRetry, enabled = selectedName != null) {
                Text(stringResource(R.string.action_retry))
            }
        }
        FilledTonalButton(onClick = onChooseAnother) {
            Text(stringResource(R.string.file_select_another))
        }
    }
}

@Composable
private fun AsrModelSelection?.batchModelLabel(state: RecognitionSettingsUiState): String = when (this) {
    is AsrModelSelection.Local -> state.installedModels.firstOrNull { it.id == modelId }?.displayName
        ?: stringResource(R.string.recognition_model_not_selected)
    is AsrModelSelection.Online -> when (provider) {
        OnlineAsrProvider.VOLCENGINE_DOUBAO -> stringResource(R.string.batch_provider_volcengine_doubao)
        OnlineAsrProvider.GROQ_WHISPER -> stringResource(R.string.batch_provider_groq_whisper)
        OnlineAsrProvider.MISTRAL_VOXTRAL -> stringResource(R.string.batch_provider_mistral_voxtral)
    }
    null -> stringResource(R.string.recognition_model_not_selected)
}

private fun Context.documentMetadata(uri: Uri): Pair<String, Long?> {
    contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)
        ?.use { cursor ->
            if (cursor.moveToFirst()) {
                val name = cursor.getString(0) ?: uri.lastPathSegment.orEmpty()
                val size = if (cursor.isNull(1)) null else cursor.getLong(1)
                return name to size
            }
        }
    return uri.lastPathSegment.orEmpty() to null
}

private fun formatFileSize(bytes: Long): String = when {
    bytes >= 1024L * 1024L -> "%.1f MB".format(bytes / (1024.0 * 1024.0))
    bytes >= 1024L -> "%.1f KB".format(bytes / 1024.0)
    else -> "$bytes B"
}

private fun AudioImportStage.labelRes(): Int = when (this) {
    AudioImportStage.QUEUED -> R.string.library_import_queued
    AudioImportStage.STAGING -> R.string.library_import_staging
    AudioImportStage.TRANSCODING -> R.string.library_import_transcoding
    AudioImportStage.TRANSCRIBING -> R.string.library_import_transcribing
    AudioImportStage.SAVING -> R.string.library_import_saving
}
