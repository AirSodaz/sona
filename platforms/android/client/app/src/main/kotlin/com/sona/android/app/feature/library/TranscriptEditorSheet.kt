package com.sona.android.app.feature.library

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.CallMerge
import androidx.compose.material.icons.automirrored.rounded.CallSplit
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.application.recording.TranscriptSegment
import kotlin.math.abs

/**
 * Spacious bottom-sheet editor for a single transcript segment, featuring
 * interactive visual split positioning (slider + live two-part preview),
 * merge with next, and delete.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun TranscriptEditorSheet(
    segment: TranscriptSegment,
    hasNext: Boolean,
    onUpdateText: (String) -> Unit,
    onUpdateTranslation: (String) -> Unit,
    onDelete: () -> Unit,
    onMerge: () -> Unit,
    onSplit: (String, String, String?, String?) -> Unit,
    onDismiss: () -> Unit,
) {
    var textValue by remember(segment.id) { mutableStateOf(TextFieldValue(segment.text)) }
    var translationValue by remember(segment.id) { mutableStateOf(TextFieldValue(segment.translation.orEmpty())) }
    var splitMode by remember(segment.id) { mutableStateOf(false) }

    LaunchedEffect(segment.id, segment.text) {
        if (textValue.text != segment.text) {
            textValue = TextFieldValue(segment.text)
        }
    }
    LaunchedEffect(segment.id, segment.translation) {
        val externalTranslation = segment.translation.orEmpty()
        if (translationValue.text != externalTranslation) {
            translationValue = TextFieldValue(externalTranslation)
        }
    }

    val maxSplitIndex = (textValue.text.length - 1).coerceAtLeast(1)
    var splitIndex by remember(segment.id, textValue.text.length) {
        val initial = (textValue.text.length / 2).coerceIn(1, maxSplitIndex)
        mutableIntStateOf(initial)
    }
    var leftTranslation by remember(segment.id) { mutableStateOf("") }
    var rightTranslation by remember(segment.id) { mutableStateOf("") }

    LaunchedEffect(splitMode, splitIndex, translationValue.text) {
        if (splitMode && textValue.text.length >= 2) {
            val safe = splitIndex.coerceIn(1, textValue.text.length - 1)
            val parts = splitTranslation(
                value = translationValue.text,
                sourceIndex = safe,
                sourceLength = textValue.text.length,
            )
            leftTranslation = parts.first
            rightTranslation = parts.second
        }
    }

    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(14.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Row(
                    verticalAlignment = Alignment.CenterVertically,
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    Surface(
                        shape = RoundedCornerShape(10.dp),
                        color = MaterialTheme.colorScheme.primaryContainer,
                        contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                    ) {
                        Text(
                            text = formatMediaTime((segment.startSeconds * 1_000).toLong()),
                            style = MaterialTheme.typography.labelMedium,
                            fontWeight = FontWeight.Bold,
                            modifier = Modifier.padding(horizontal = 10.dp, vertical = 4.dp),
                        )
                    }
                    Text(
                        text = stringResource(R.string.transcript_edit),
                        style = MaterialTheme.typography.titleMedium,
                        fontWeight = FontWeight.SemiBold,
                    )
                }
                IconButton(onClick = onDismiss) {
                    Icon(Icons.Rounded.Close, stringResource(R.string.action_close))
                }
            }

            if (splitMode && textValue.text.length >= 2) {
                val safeIndex = splitIndex.coerceIn(1, textValue.text.length - 1)
                val leftText = textValue.text.substring(0, safeIndex).trim()
                val rightText = textValue.text.substring(safeIndex).trim()

                Text(
                    text = stringResource(R.string.transcript_split_hint),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )

                Slider(
                    value = safeIndex.toFloat(),
                    onValueChange = { splitIndex = it.toInt().coerceIn(1, textValue.text.length - 1) },
                    valueRange = 1f..(textValue.text.length - 1).toFloat(),
                    modifier = Modifier.fillMaxWidth(),
                )

                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(
                        modifier = Modifier.padding(14.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.transcript_left_original),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.primary,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            text = leftText.ifBlank { "—" },
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }

                Surface(
                    shape = RoundedCornerShape(16.dp),
                    color = MaterialTheme.colorScheme.surfaceContainerHigh,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Column(
                        modifier = Modifier.padding(14.dp),
                        verticalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        Text(
                            text = stringResource(R.string.transcript_right_original),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.primary,
                            fontWeight = FontWeight.Bold,
                        )
                        Text(
                            text = rightText.ifBlank { "—" },
                            style = MaterialTheme.typography.bodyMedium,
                        )
                    }
                }

                if (segment.translation != null) {
                    OutlinedTextField(
                        value = leftTranslation,
                        onValueChange = { leftTranslation = it },
                        label = { Text(stringResource(R.string.transcript_left_translation)) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                    OutlinedTextField(
                        value = rightTranslation,
                        onValueChange = { rightTranslation = it },
                        label = { Text(stringResource(R.string.transcript_right_translation)) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    OutlinedButton(
                        onClick = { splitMode = false },
                        modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(16.dp),
                    ) {
                        Text(stringResource(R.string.action_cancel))
                    }
                    Button(
                        enabled = leftText.isNotBlank() && rightText.isNotBlank() &&
                            (segment.translation == null || (leftTranslation.isNotBlank() && rightTranslation.isNotBlank())),
                        onClick = {
                            onSplit(
                                leftText,
                                rightText,
                                leftTranslation.takeIf { segment.translation != null },
                                rightTranslation.takeIf { segment.translation != null },
                            )
                            onDismiss()
                        },
                        modifier = Modifier.weight(1f),
                        shape = RoundedCornerShape(16.dp),
                    ) {
                        Text(stringResource(R.string.transcript_split))
                    }
                }
            } else {
                OutlinedTextField(
                    value = textValue,
                    onValueChange = {
                        textValue = it
                        onUpdateText(it.text)
                    },
                    label = { Text(stringResource(R.string.transcript_original)) },
                    shape = RoundedCornerShape(16.dp),
                    modifier = Modifier.fillMaxWidth(),
                )

                OutlinedTextField(
                    value = translationValue,
                    onValueChange = {
                        translationValue = it
                        onUpdateTranslation(it.text)
                    },
                    label = { Text(stringResource(R.string.transcript_translation)) },
                    shape = RoundedCornerShape(16.dp),
                    modifier = Modifier.fillMaxWidth(),
                )

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    FilledTonalButton(
                        onClick = {
                            val cursor = textValue.selection.start
                            splitIndex = if (cursor in 1 until textValue.text.length) {
                                cursor
                            } else {
                                (textValue.text.length / 2).coerceIn(1, maxSplitIndex)
                            }
                            splitMode = true
                        },
                        enabled = textValue.text.length >= 2,
                        shape = RoundedCornerShape(16.dp),
                    ) {
                        Icon(Icons.AutoMirrored.Rounded.CallSplit, null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.transcript_split))
                    }

                    FilledTonalButton(
                        onClick = {
                            onMerge()
                            onDismiss()
                        },
                        enabled = hasNext,
                        shape = RoundedCornerShape(16.dp),
                    ) {
                        Icon(Icons.AutoMirrored.Rounded.CallMerge, null, modifier = Modifier.size(18.dp))
                        Spacer(Modifier.width(6.dp))
                        Text(stringResource(R.string.transcript_merge_next))
                    }

                    Spacer(Modifier.weight(1f))

                    TextButton(
                        onClick = {
                            onDelete()
                            onDismiss()
                        },
                    ) {
                        Icon(
                            Icons.Rounded.Delete,
                            contentDescription = stringResource(R.string.action_delete),
                            tint = MaterialTheme.colorScheme.error,
                            modifier = Modifier.size(20.dp),
                        )
                    }
                }
            }
        }
    }
}

private fun splitTranslation(
    value: String,
    sourceIndex: Int,
    sourceLength: Int,
): Pair<String, String> {
    if (value.isBlank()) return "" to ""
    val target = ((sourceIndex.toDouble() / sourceLength.coerceAtLeast(1)) * value.length).toInt()
    val boundaries = value.indices.filter { index -> value[index].isWhitespace() }
    val boundary = boundaries.minByOrNull { abs(it - target) } ?: target
    return value.substring(0, boundary).trim() to value.substring(boundary).trim()
}
