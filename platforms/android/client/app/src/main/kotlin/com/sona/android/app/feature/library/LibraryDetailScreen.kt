package com.sona.android.app.feature.library

import androidx.annotation.StringRes
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
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.library.RecordingLibraryItemStatus
import com.sona.android.application.recording.CloudTranscriptionFailure
import com.sona.android.application.recording.TranscriptSegment

@Composable
internal fun LibraryDetailScreen(
    historyId: String,
    item: RecordingLibraryItem?,
    detail: LibraryDetailUiState,
    cloudTranscription: CloudTranscriptionUiState,
    onRetry: () -> Unit,
    onTranscribeWithCloud: (RecordingLibraryItem) -> Unit,
    onTranscribeWithCurrentEngine: (RecordingLibraryItem) -> Unit,
) {
    val resolvedDetail = detail.forHistory(historyId)
    val fallbackTitle = stringResource(R.string.library_detail_heading)

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
            Text(
                text = item?.title?.ifBlank { fallbackTitle } ?: fallbackTitle,
                style = MaterialTheme.typography.headlineMedium,
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.SemiBold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            Spacer(Modifier.height(6.dp))
            item?.let { LibraryItemMetadata(it) }

            if (item?.status == RecordingLibraryItemStatus.DRAFT) {
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
private fun CloudTranscriptionAction(
    item: RecordingLibraryItem,
    cloudTranscription: CloudTranscriptionUiState,
    onTranscribeWithCloud: (RecordingLibraryItem) -> Unit,
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
