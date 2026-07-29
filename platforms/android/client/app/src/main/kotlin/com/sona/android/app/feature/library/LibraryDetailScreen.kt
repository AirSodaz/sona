package com.sona.android.app.feature.library

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.TagRecord
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.TranscriptSnapshotDetail
import com.sona.android.application.recording.CloudTranscriptionFailure
import com.sona.android.application.recording.TranscriptSegment

@Composable
internal fun LibraryDetailScreen(
    historyId: String,
    item: HistoryItem?,
    detail: LibraryDetailUiState,
    cloudTranscription: CloudTranscriptionUiState,
    tags: List<TagRecord>,
    snapshots: List<TranscriptSnapshot>,
    snapshotDetail: TranscriptSnapshotDetail?,
    operationInProgress: Boolean,
    operationError: Boolean,
    onRetry: () -> Unit,
    onTranscribeWithCloud: (HistoryItem) -> Unit,
    onTranscribeWithCurrentEngine: (HistoryItem) -> Unit,
    onUpdateTitle: (String) -> Unit,
    onUpdateTags: (Set<String>) -> Unit,
    onCreateTag: (String) -> Unit,
    onLoadSnapshot: (String) -> Unit,
    onCloseSnapshot: () -> Unit,
    onExportTranscript: (String, TranscriptExportFormat, TranscriptExportMode) -> Unit,
) {
    val resolvedDetail = detail.forHistory(historyId)
    val fallbackTitle = stringResource(R.string.library_detail_heading)
    var titleEditorVisible by remember { mutableStateOf(false) }
    var titleInput by remember(item?.historyId, item?.title) { mutableStateOf(item?.title.orEmpty()) }
    var tagCreatorVisible by remember { mutableStateOf(false) }
    var tagNameInput by remember { mutableStateOf("") }
    var exportDialogVisible by remember { mutableStateOf(false) }
    var exportFormat by remember { mutableStateOf(TranscriptExportFormat.TXT) }
    var exportMode by remember { mutableStateOf(TranscriptExportMode.ORIGINAL) }
    var pendingExport by remember { mutableStateOf<Pair<TranscriptExportFormat, TranscriptExportMode>?>(null) }
    val exportLauncher = rememberLauncherForActivityResult(
        ActivityResultContracts.CreateDocument("*/*"),
    ) { uri ->
        val selection = pendingExport
        pendingExport = null
        if (uri != null && selection != null) {
            onExportTranscript(uri.toString(), selection.first, selection.second)
        }
    }

    if (titleEditorVisible) {
        AlertDialog(
            onDismissRequest = { titleEditorVisible = false },
            title = { Text(stringResource(R.string.history_edit_title)) },
            text = {
                OutlinedTextField(
                    value = titleInput,
                    onValueChange = { titleInput = it },
                    singleLine = true,
                )
            },
            confirmButton = {
                TextButton(
                    enabled = titleInput.isNotBlank() && !operationInProgress,
                    onClick = {
                        onUpdateTitle(titleInput.trim())
                        titleEditorVisible = false
                    },
                ) { Text(stringResource(R.string.action_save)) }
            },
            dismissButton = {
                TextButton(onClick = { titleEditorVisible = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }
    if (tagCreatorVisible) {
        AlertDialog(
            onDismissRequest = { tagCreatorVisible = false },
            title = { Text(stringResource(R.string.history_create_tag)) },
            text = {
                OutlinedTextField(
                    value = tagNameInput,
                    onValueChange = { tagNameInput = it },
                    label = { Text(stringResource(R.string.history_tag_name)) },
                    singleLine = true,
                )
            },
            confirmButton = {
                TextButton(
                    enabled = tagNameInput.isNotBlank() && !operationInProgress,
                    onClick = {
                        onCreateTag(tagNameInput.trim())
                        tagNameInput = ""
                        tagCreatorVisible = false
                    },
                ) { Text(stringResource(R.string.history_create_tag)) }
            },
            dismissButton = {
                TextButton(onClick = { tagCreatorVisible = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }
    if (exportDialogVisible) {
        TranscriptExportDialog(
            format = exportFormat,
            mode = exportMode,
            enabled = resolvedDetail is LibraryDetailUiState.Ready && !operationInProgress,
            onFormatChanged = { exportFormat = it },
            onModeChanged = { exportMode = it },
            onDismiss = { exportDialogVisible = false },
            onExport = {
                exportDialogVisible = false
                pendingExport = exportFormat to exportMode
                exportLauncher.launch(transcriptFileName(item?.title, exportFormat))
            },
        )
    }
    snapshotDetail?.takeIf { it.metadata.historyId == historyId }?.let { snapshot ->
        AlertDialog(
            onDismissRequest = onCloseSnapshot,
            title = {
                Text(stringResource(R.string.history_snapshot_title, snapshot.metadata.reason.name.lowercase()))
            },
            text = { TranscriptDetail(snapshot.segments) },
            confirmButton = {
                TextButton(onClick = onCloseSnapshot) { Text(stringResource(R.string.action_close)) }
            },
        )
    }

    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .widthIn(max = 840.dp)
                .padding(horizontal = 24.dp, vertical = 20.dp),
        ) {
            androidx.compose.foundation.layout.Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = item?.title?.ifBlank { fallbackTitle } ?: fallbackTitle,
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.weight(1f),
                )
                IconButton(
                    enabled = item != null && !operationInProgress,
                    onClick = { titleEditorVisible = true },
                ) {
                    Icon(Icons.Rounded.Edit, stringResource(R.string.history_edit_title))
                }
                IconButton(
                    enabled = resolvedDetail is LibraryDetailUiState.Ready && !operationInProgress,
                    onClick = { exportDialogVisible = true },
                ) {
                    Icon(Icons.Rounded.FileDownload, stringResource(R.string.history_export_transcript))
                }
            }
            Spacer(Modifier.height(6.dp))
            item?.let { LibraryItemMetadata(it) }

            if (item != null) {
                Spacer(Modifier.height(12.dp))
                Text(
                    text = stringResource(R.string.history_tags),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                androidx.compose.foundation.layout.Row(
                    modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    tags.forEach { tag ->
                        val selected = tag.id in item.tagIds
                        FilterChip(
                            selected = selected,
                            enabled = !operationInProgress,
                            onClick = {
                                val updated = item.tagIds.toMutableSet().apply {
                                    if (selected) remove(tag.id) else add(tag.id)
                                }
                                onUpdateTags(updated)
                            },
                            label = { Text(tag.name) },
                        )
                    }
                    TextButton(
                        enabled = !operationInProgress,
                        onClick = { tagCreatorVisible = true },
                    ) { Text(stringResource(R.string.history_create_tag)) }
                }
            }
            if (operationError) {
                Text(
                    text = stringResource(R.string.history_operation_failed),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodyMedium,
                )
            }

            if (item?.status == HistoryItemStatus.DRAFT) {
                Spacer(Modifier.height(12.dp))
                Card(
                    shape = MaterialTheme.shapes.small,
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.tertiaryContainer,
                        contentColor = MaterialTheme.colorScheme.onTertiaryContainer
                    ),
                    modifier = Modifier.fillMaxWidth()
                ) {
                    Text(
                        text = stringResource(R.string.library_draft_notice),
                        style = MaterialTheme.typography.bodyMedium,
                        fontWeight = FontWeight.Medium,
                        modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp)
                    )
                }
            }
            Spacer(Modifier.height(16.dp))
            HorizontalDivider()
            Spacer(Modifier.height(16.dp))
            Text(
                text = stringResource(R.string.library_transcript_heading),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
            if (item != null) {
                Spacer(Modifier.height(8.dp))
                Column(
                    modifier = Modifier.fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    FilledTonalButton(
                        onClick = { onTranscribeWithCurrentEngine(item) },
                        enabled = item.audioAvailable,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Replay,
                            contentDescription = null,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(stringResource(R.string.action_transcribe_current_engine))
                    }
                    CloudTranscriptionAction(
                        item = item,
                        cloudTranscription = cloudTranscription,
                        onTranscribeWithCloud = onTranscribeWithCloud,
                        modifier = Modifier.fillMaxWidth(),
                    )
                }
            }
            CloudTranscriptionStatus(historyId = historyId, state = cloudTranscription)
            Text(
                text = stringResource(R.string.history_snapshots),
                style = MaterialTheme.typography.titleSmall,
                fontWeight = FontWeight.SemiBold,
            )
            if (snapshots.isEmpty()) {
                Text(
                    text = stringResource(R.string.history_snapshot_empty),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            } else {
                androidx.compose.foundation.layout.Row(
                    modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    snapshots.forEach { snapshot ->
                        TextButton(onClick = { onLoadSnapshot(snapshot.id) }) {
                            Text(snapshot.reason.name.lowercase())
                        }
                    }
                }
            }
            Spacer(Modifier.height(8.dp))

            when (resolvedDetail) {
                is LibraryDetailUiState.Loading -> LibraryLoading(modifier = Modifier.weight(1f))
                is LibraryDetailUiState.Failed -> LibraryTranscriptError(
                    onRetry = onRetry,
                    modifier = Modifier.weight(1f),
                )
                is LibraryDetailUiState.Ready -> TranscriptDetail(
                    segments = resolvedDetail.segments,
                    modifier = Modifier.weight(1f),
                )
                LibraryDetailUiState.None -> Unit
            }
        }
    }
}

@Composable
private fun TranscriptExportDialog(
    format: TranscriptExportFormat,
    mode: TranscriptExportMode,
    enabled: Boolean,
    onFormatChanged: (TranscriptExportFormat) -> Unit,
    onModeChanged: (TranscriptExportMode) -> Unit,
    onDismiss: () -> Unit,
    onExport: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = onDismiss,
        title = { Text(stringResource(R.string.history_export_transcript)) },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(stringResource(R.string.history_export_format), fontWeight = FontWeight.SemiBold)
                androidx.compose.foundation.layout.Row(
                    modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    TranscriptExportFormat.entries.forEach { value ->
                        FilterChip(
                            selected = format == value,
                            onClick = { onFormatChanged(value) },
                            label = { Text(value.name.lowercase()) },
                        )
                    }
                }
                Text(stringResource(R.string.history_export_content), fontWeight = FontWeight.SemiBold)
                TranscriptExportMode.entries.forEach { value ->
                    FilterChip(
                        selected = mode == value,
                        onClick = { onModeChanged(value) },
                        label = {
                            Text(
                                stringResource(
                                    when (value) {
                                        TranscriptExportMode.ORIGINAL -> R.string.history_export_original
                                        TranscriptExportMode.TRANSLATION -> R.string.history_export_translation
                                        TranscriptExportMode.BILINGUAL -> R.string.history_export_bilingual
                                    },
                                ),
                            )
                        },
                    )
                }
            }
        },
        confirmButton = {
            TextButton(enabled = enabled, onClick = onExport) {
                Text(stringResource(R.string.action_export))
            }
        },
        dismissButton = {
            TextButton(onClick = onDismiss) { Text(stringResource(R.string.action_cancel)) }
        },
    )
}

private fun transcriptFileName(title: String?, format: TranscriptExportFormat): String {
    val stem = title.orEmpty().trim().ifBlank { "sona-transcript" }
        .replace(Regex("[^A-Za-z0-9._ -]"), "_")
    val extension = when (format) {
        TranscriptExportFormat.JSON -> "json"
        TranscriptExportFormat.TXT -> "txt"
        TranscriptExportFormat.SRT -> "srt"
        TranscriptExportFormat.VTT -> "vtt"
        TranscriptExportFormat.MARKDOWN -> "md"
    }
    return "$stem.$extension"
}

@Composable
private fun CloudTranscriptionAction(
    item: HistoryItem,
    cloudTranscription: CloudTranscriptionUiState,
    onTranscribeWithCloud: (HistoryItem) -> Unit,
    modifier: Modifier = Modifier,
) {
    val running = cloudTranscription is CloudTranscriptionUiState.Running
    FilledTonalButton(
        onClick = { onTranscribeWithCloud(item) },
        enabled = item.audioAvailable && !running,
        modifier = modifier,
    ) {
        if (running) {
            CircularProgressIndicator(Modifier.size(16.dp), strokeWidth = 2.dp)
            Spacer(Modifier.width(8.dp))
        } else {
            Icon(
                imageVector = Icons.Rounded.CloudSync,
                contentDescription = null,
                modifier = Modifier.size(18.dp),
            )
            Spacer(Modifier.width(8.dp))
        }
        Text(
            text = stringResource(
                if (running) {
                    R.string.cloud_transcription_running
                } else {
                    R.string.action_cloud_transcribe
                },
            ),
        )
    }
}

@Composable
private fun CloudTranscriptionStatus(
    historyId: String,
    state: CloudTranscriptionUiState,
) {
    val message = when (state) {
        CloudTranscriptionUiState.Idle -> null
        is CloudTranscriptionUiState.Running -> null
        is CloudTranscriptionUiState.Completed ->
            state.takeIf { it.historyId == historyId }?.let {
                CloudTranscriptionMessage(R.string.cloud_transcription_completed, isError = false)
            }
        is CloudTranscriptionUiState.Failed ->
            state.takeIf { it.historyId == historyId }?.let {
                CloudTranscriptionMessage(it.reason.messageRes, isError = true)
            }
    } ?: return

    Spacer(Modifier.height(8.dp))
    Card(
        shape = MaterialTheme.shapes.small,
        colors = CardDefaults.cardColors(
            containerColor = if (message.isError) {
                MaterialTheme.colorScheme.errorContainer
            } else {
                MaterialTheme.colorScheme.secondaryContainer
            },
            contentColor = if (message.isError) {
                MaterialTheme.colorScheme.onErrorContainer
            } else {
                MaterialTheme.colorScheme.onSecondaryContainer
            },
        ),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = stringResource(message.textRes),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
        )
    }
}

private data class CloudTranscriptionMessage(
    @param:StringRes val textRes: Int,
    val isError: Boolean,
)

@get:StringRes
private val CloudTranscriptionFailure.messageRes: Int
    get() = when (this) {
        CloudTranscriptionFailure.MISSING_CREDENTIAL ->
            R.string.cloud_transcription_missing_credential
        CloudTranscriptionFailure.MISSING_AUDIO -> R.string.cloud_transcription_missing_audio
        CloudTranscriptionFailure.TRANSCRIPTION_FAILED -> R.string.cloud_transcription_failed
        CloudTranscriptionFailure.EMPTY_TRANSCRIPT -> R.string.cloud_transcription_empty
        CloudTranscriptionFailure.PERSISTENCE_FAILED ->
            R.string.cloud_transcription_persist_failed
    }

private fun LibraryDetailUiState.forHistory(historyId: String): LibraryDetailUiState = when (this) {
    is LibraryDetailUiState.Ready -> if (this.historyId == historyId) {
        this
    } else {
        LibraryDetailUiState.Loading(historyId)
    }
    is LibraryDetailUiState.Failed -> if (this.historyId == historyId) {
        this
    } else {
        LibraryDetailUiState.Loading(historyId)
    }
    is LibraryDetailUiState.Loading -> if (this.historyId == historyId) {
        this
    } else {
        LibraryDetailUiState.Loading(historyId)
    }
    LibraryDetailUiState.None -> LibraryDetailUiState.Loading(historyId)
}

@Composable
private fun TranscriptDetail(
    segments: List<TranscriptSegment>,
    modifier: Modifier = Modifier,
) {
    if (segments.isEmpty()) {
        Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
            Text(
                text = stringResource(R.string.library_transcript_empty),
                style = MaterialTheme.typography.bodyLarge,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        return
    }
    LazyColumn(
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        contentPadding = PaddingValues(vertical = 8.dp)
    ) {
        items(segments, key = TranscriptSegment::id) { segment ->
            Card(
                shape = MaterialTheme.shapes.medium,
                colors = CardDefaults.cardColors(
                    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh
                ),
                modifier = Modifier.fillMaxWidth()
            ) {
                Column(
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp)
                ) {
                    segment.speaker?.label?.takeIf(String::isNotBlank)?.let { speaker ->
                        Card(
                            shape = MaterialTheme.shapes.extraSmall,
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.12f),
                                contentColor = MaterialTheme.colorScheme.primary
                            ),
                            modifier = Modifier.padding(bottom = 6.dp)
                        ) {
                            Text(
                                text = speaker,
                                style = MaterialTheme.typography.labelSmall,
                                fontWeight = FontWeight.Bold,
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp)
                            )
                        }
                    }
                    Text(
                        text = segment.text,
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurface,
                    )
                }
            }
        }
    }
}

@Composable
private fun LibraryTranscriptError(
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .background(
                color = MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.3f),
                shape = MaterialTheme.shapes.medium
            )
            .padding(32.dp),
        contentAlignment = Alignment.Center
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = stringResource(R.string.library_transcript_load_failed),
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium
            )
            Spacer(Modifier.height(12.dp))
            FilledTonalButton(onClick = onRetry) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}
