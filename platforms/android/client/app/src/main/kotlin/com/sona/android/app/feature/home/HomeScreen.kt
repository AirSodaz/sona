package com.sona.android.app.feature.home

import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.IntrinsicSize
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.library.LibraryItemRow
import com.sona.android.app.feature.library.LibraryUiState
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.application.recording.AsrModelSelection
import com.sona.android.application.recording.AudioImportJobState
import com.sona.android.application.recording.LiveRecordingState
import com.sona.android.application.recording.OnlineAsrProvider

@Composable
internal fun HomeScreen(
    recordingState: LiveRecordingState,
    libraryState: LibraryUiState,
    recognitionSettings: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
    onOpenLive: () -> Unit,
    onOpenFile: () -> Unit,
    onOpenLibrary: () -> Unit,
    onOpenItem: (String) -> Unit,
) {
    val liveAvailable = recognitionSettings.liveSelection.isConfigured(
        recognitionSettings,
        configuredProviders,
    )
    val batchAvailable = recognitionSettings.batchSelection.isConfigured(
        recognitionSettings,
        configuredProviders,
    )
    Box(Modifier.fillMaxSize(), contentAlignment = Alignment.TopCenter) {
        Column(
            modifier = Modifier
                .widthIn(max = 960.dp)
                .fillMaxWidth()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 24.dp, vertical = 20.dp),
            verticalArrangement = Arrangement.spacedBy(20.dp),
        ) {
        Text(
            text = stringResource(R.string.home_heading),
            style = MaterialTheme.typography.headlineMedium,
            fontWeight = FontWeight.SemiBold,
            color = MaterialTheme.colorScheme.primary,
        )
        BoxWithConstraints(Modifier.fillMaxWidth()) {
            val wide = maxWidth >= 600.dp
            if (wide) {
                Row(
                    modifier = Modifier.height(IntrinsicSize.Max),
                    horizontalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    HomeActionCard(
                        title = stringResource(R.string.home_live_title),
                        subtitle = recognitionSettings.liveSelection.modelLabel(recognitionSettings),
                        status = recordingState.liveStatus(liveAvailable),
                        icon = { Icon(Icons.Rounded.Mic, null) },
                        onClick = onOpenLive,
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                    HomeActionCard(
                        title = stringResource(R.string.home_file_title),
                        subtitle = recognitionSettings.batchSelection.modelLabel(recognitionSettings),
                        status = libraryState.audioImport.fileStatus(batchAvailable),
                        icon = { Icon(Icons.Rounded.AudioFile, null) },
                        onClick = onOpenFile,
                        modifier = Modifier.weight(1f).fillMaxHeight(),
                    )
                }
            } else {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    HomeActionCard(
                        title = stringResource(R.string.home_live_title),
                        subtitle = recognitionSettings.liveSelection.modelLabel(recognitionSettings),
                        status = recordingState.liveStatus(liveAvailable),
                        icon = { Icon(Icons.Rounded.Mic, null) },
                        onClick = onOpenLive,
                    )
                    HomeActionCard(
                        title = stringResource(R.string.home_file_title),
                        subtitle = recognitionSettings.batchSelection.modelLabel(recognitionSettings),
                        status = libraryState.audioImport.fileStatus(batchAvailable),
                        icon = { Icon(Icons.Rounded.AudioFile, null) },
                        onClick = onOpenFile,
                    )
                }
            }
        }
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                text = stringResource(R.string.home_recent_title),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onOpenLibrary) {
                Text(stringResource(R.string.home_view_all))
                Icon(Icons.AutoMirrored.Rounded.ArrowForward, null)
            }
        }
        if (libraryState.items.isEmpty()) {
            Text(
                text = stringResource(R.string.library_empty),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.padding(vertical = 24.dp),
            )
        } else {
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                libraryState.items.take(3).forEach { item ->
                    LibraryItemRow(item, onClick = { onOpenItem(item.historyId) })
                }
            }
        }
        }
    }
}

@Composable
private fun HomeActionCard(
    title: String,
    subtitle: String,
    status: String,
    icon: @Composable () -> Unit,
    onClick: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Card(
        modifier = modifier
            .fillMaxWidth()
            .heightIn(min = 164.dp)
            .clickable(onClick = onClick),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
        shape = MaterialTheme.shapes.medium,
    ) {
        Column(
            modifier = Modifier.padding(20.dp),
            verticalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            androidx.compose.material3.Surface(
                color = MaterialTheme.colorScheme.primaryContainer,
                contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                shape = MaterialTheme.shapes.small,
            ) {
                androidx.compose.foundation.layout.Box(
                    Modifier.padding(10.dp).size(24.dp),
                    contentAlignment = Alignment.Center,
                ) { icon() }
            }
            Spacer(Modifier.height(8.dp))
            Text(title, style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Text(subtitle, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
            Text(status, style = MaterialTheme.typography.labelMedium, color = MaterialTheme.colorScheme.primary)
        }
    }
}

@Composable
private fun AsrModelSelection?.modelLabel(state: RecognitionSettingsUiState): String = when (this) {
    is AsrModelSelection.Local -> state.installedModels.firstOrNull { it.id == modelId }?.displayName
        ?: stringResource(R.string.recognition_model_not_selected)
    is AsrModelSelection.Online -> when (provider) {
        OnlineAsrProvider.VOLCENGINE_DOUBAO -> stringResource(R.string.batch_provider_volcengine_doubao)
        OnlineAsrProvider.GROQ_WHISPER -> stringResource(R.string.batch_provider_groq_whisper)
        OnlineAsrProvider.MISTRAL_VOXTRAL -> stringResource(R.string.batch_provider_mistral_voxtral)
    }
    null -> stringResource(R.string.recognition_model_not_selected)
}

@Composable
private fun LiveRecordingState.liveStatus(available: Boolean): String = when (this) {
    is LiveRecordingState.Recording -> stringResource(R.string.home_status_recording)
    is LiveRecordingState.Preparing -> stringResource(R.string.home_status_preparing)
    else -> stringResource(if (available) R.string.home_status_idle else R.string.home_status_unavailable)
}

@Composable
private fun AudioImportJobState.fileStatus(available: Boolean): String = when (this) {
    is AudioImportJobState.Running -> stringResource(R.string.home_status_processing)
    is AudioImportJobState.Completed -> stringResource(R.string.home_status_completed)
    is AudioImportJobState.Failed -> stringResource(R.string.home_status_failed)
    AudioImportJobState.Idle -> stringResource(if (available) R.string.home_status_idle else R.string.home_status_unavailable)
}

private fun AsrModelSelection?.isConfigured(
    state: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
): Boolean = when (this) {
    is AsrModelSelection.Local -> state.installedModels.any { it.id == modelId }
    is AsrModelSelection.Online -> provider in configuredProviders
    null -> false
}
