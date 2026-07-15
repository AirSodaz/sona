package com.sona.android.app.feature.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.HorizontalDivider
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
import com.sona.android.application.recording.TranscriptSegment

@Composable
internal fun LibraryDetailScreen(
    historyId: String,
    item: RecordingLibraryItem?,
    detail: LibraryDetailUiState,
    onRetry: () -> Unit,
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
                style = MaterialTheme.typography.headlineSmall,
                fontWeight = FontWeight.SemiBold,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
            )
            item?.let { LibraryItemMetadata(it) }
            if (item?.status == RecordingLibraryItemStatus.DRAFT) {
                Spacer(Modifier.height(12.dp))
                Text(
                    text = stringResource(R.string.library_draft_notice),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.tertiary,
                )
            }
            Spacer(Modifier.height(16.dp))
            HorizontalDivider()
            Spacer(Modifier.height(16.dp))
            Text(
                text = stringResource(R.string.library_transcript_heading),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
            )
            Spacer(Modifier.height(12.dp))
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
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        items(segments, key = TranscriptSegment::id) { segment ->
            Column(modifier = Modifier.fillMaxWidth()) {
                segment.speaker?.label?.takeIf(String::isNotBlank)?.let { speaker ->
                    Text(
                        text = speaker,
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    Spacer(Modifier.height(2.dp))
                }
                Text(
                    text = segment.text,
                    style = MaterialTheme.typography.bodyLarge,
                    color = if (segment.isFinal) {
                        MaterialTheme.colorScheme.onSurface
                    } else {
                        MaterialTheme.colorScheme.primary
                    },
                )
            }
        }
    }
}

@Composable
private fun LibraryTranscriptError(
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = stringResource(R.string.library_transcript_load_failed),
                color = MaterialTheme.colorScheme.error,
            )
            TextButton(onClick = onRetry) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}
