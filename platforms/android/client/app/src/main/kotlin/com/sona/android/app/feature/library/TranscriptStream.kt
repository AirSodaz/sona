package com.sona.android.app.feature.library

import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.recording.TranscriptSegment
import java.util.Locale

@Composable
internal fun TranscriptStream(
    segments: List<TranscriptSegment>,
    modifier: Modifier = Modifier,
    editor: TranscriptEditorUiState? = null,
    playbackPositionMillis: Long? = null,
    onSeek: (Long) -> Unit = {},
    onEditSegment: (String?) -> Unit = {},
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

    val listState = rememberLazyListState()
    val activeIndex = playbackPositionMillis?.let { position ->
        segments.indexOfFirst { segment ->
            position >= (segment.startSeconds * 1_000).toLong() &&
                position < (segment.endSeconds * 1_000).toLong()
        }.takeIf { it >= 0 }
    }

    LaunchedEffect(activeIndex, editor?.active) {
        if (activeIndex != null && editor?.active != true) {
            listState.animateScrollToItem(activeIndex)
        }
    }

    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxWidth(),
        contentPadding = PaddingValues(vertical = 8.dp),
    ) {
        items(segments, key = TranscriptSegment::id) { segment ->
            val active = playbackPositionMillis?.let { position ->
                position >= (segment.startSeconds * 1_000).toLong() &&
                    position < (segment.endSeconds * 1_000).toLong()
            } == true
            val isEditingTarget = editor?.active == true && editor.editingSegmentId == segment.id

            val rowBackground by animateColorAsState(
                targetValue = when {
                    isEditingTarget -> MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.55f)
                    active -> MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.7f)
                    else -> Color.Transparent
                },
                label = "transcriptRowBg",
            )

            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .clip(RoundedCornerShape(16.dp))
                    .background(rowBackground)
                    .clickable {
                        if (editor?.active == true) {
                            onEditSegment(segment.id)
                        } else {
                            onSeek((segment.startSeconds * 1_000).toLong())
                        }
                    }
                    .padding(horizontal = 14.dp, vertical = 12.dp),
                verticalArrangement = Arrangement.spacedBy(6.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Surface(
                        shape = RoundedCornerShape(8.dp),
                        color = if (active) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.surfaceContainerHigh
                        },
                        contentColor = if (active) {
                            MaterialTheme.colorScheme.onPrimary
                        } else {
                            MaterialTheme.colorScheme.primary
                        },
                    ) {
                        Text(
                            text = formatMediaTime((segment.startSeconds * 1_000).toLong()),
                            style = MaterialTheme.typography.labelSmall,
                            fontWeight = FontWeight.SemiBold,
                            modifier = Modifier.padding(horizontal = 7.dp, vertical = 2.dp),
                        )
                    }

                    segment.speaker?.label?.takeIf(String::isNotBlank)?.let { speaker ->
                        Surface(
                            shape = RoundedCornerShape(8.dp),
                            color = MaterialTheme.colorScheme.secondaryContainer,
                            contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                        ) {
                            Text(
                                text = speaker,
                                style = MaterialTheme.typography.labelSmall,
                                fontWeight = FontWeight.Bold,
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 2.dp),
                            )
                        }
                    }

                    Spacer(Modifier.weight(1f))

                    if (editor?.active == true) {
                        Icon(
                            imageVector = Icons.Rounded.Edit,
                            contentDescription = stringResource(R.string.transcript_edit),
                            tint = MaterialTheme.colorScheme.primary.copy(alpha = 0.8f),
                            modifier = Modifier.size(16.dp),
                        )
                    }
                }

                Text(
                    text = highlightedSegmentText(
                        segment = segment,
                        playbackPositionMillis = playbackPositionMillis,
                        highlightColor = MaterialTheme.colorScheme.primaryContainer,
                    ),
                    style = MaterialTheme.typography.bodyLarge,
                    color = MaterialTheme.colorScheme.onSurface,
                )

                segment.translation?.takeIf(String::isNotBlank)?.let { translation ->
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 2.dp),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Box(
                            modifier = Modifier
                                .width(2.5.dp)
                                .height(18.dp)
                                .background(
                                    color = MaterialTheme.colorScheme.primary.copy(alpha = 0.45f),
                                    shape = RoundedCornerShape(2.dp),
                                ),
                        )
                        Text(
                            text = translation,
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                            modifier = Modifier.weight(1f),
                        )
                    }
                }
            }

            HorizontalDivider(
                modifier = Modifier.padding(horizontal = 14.dp),
                thickness = 0.5.dp,
                color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.3f),
            )
        }
    }
}

internal fun highlightedSegmentText(
    segment: TranscriptSegment,
    playbackPositionMillis: Long?,
    highlightColor: Color,
) = buildAnnotatedString {
    append(segment.text)
    val positionSeconds = playbackPositionMillis?.div(1_000.0) ?: return@buildAnnotatedString
    val unit = segment.timing?.units?.firstOrNull {
        positionSeconds >= it.startSeconds && positionSeconds < it.endSeconds
    } ?: return@buildAnnotatedString
    val start = segment.text.indexOf(unit.text).takeIf { it >= 0 } ?: return@buildAnnotatedString
    addStyle(
        SpanStyle(background = highlightColor),
        start,
        (start + unit.text.length).coerceAtMost(segment.text.length),
    )
}

internal fun formatMediaTime(millis: Long): String {
    val totalSeconds = millis.coerceAtLeast(0L) / 1_000
    val minutes = totalSeconds / 60
    val seconds = totalSeconds % 60
    return "%d:%02d".format(Locale.US, minutes, seconds)
}
