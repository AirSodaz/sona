package com.sona.android.app.feature.library

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
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
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowBack
import androidx.compose.material.icons.rounded.AutoAwesome
import androidx.compose.material.icons.rounded.CallMerge
import androidx.compose.material.icons.rounded.CallSplit
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.ContentCopy
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.Forward5
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.Label
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Redo
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material.icons.rounded.Replay5
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material.icons.rounded.Undo
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalClipboardManager
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.TextFieldValue
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.sona.android.app.R
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.TagRecord
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.TranscriptSnapshotDetail
import com.sona.android.application.llm.LlmFailureCategory
import com.sona.android.application.llm.LlmTaskState
import com.sona.android.application.media.AudioPlaybackState
import com.sona.android.application.media.AudioPlaybackStatus
import com.sona.android.application.recording.CloudTranscriptionFailure
import com.sona.android.application.recording.TranscriptSegment
import java.util.Locale

@OptIn(ExperimentalMaterial3Api::class)
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
    playback: AudioPlaybackState,
    editor: TranscriptEditorUiState,
    llm: LibraryLlmUiState = LibraryLlmUiState(),
    onSummarize: () -> Unit = {},
    onTranslate: (String, String?) -> Unit = { _, _ -> },
    onPolish: () -> Unit = {},
    onRetryLlm: () -> Unit = {},
    onConfigureLlm: () -> Unit = {},
    onClearLlmConfigurationPrompt: () -> Unit = {},
    appLanguage: AppLanguage = AppLanguage.SYSTEM,
    exitRequestToken: Int,
    onNavigateBack: () -> Unit,
    onTogglePlayback: () -> Unit,
    onSeekPlayback: (Long) -> Unit,
    onSkipPlayback: (Long) -> Unit,
    onSetPlaybackSpeed: (Float) -> Unit,
    onPausePlayback: () -> Unit,
    onReleasePlayback: () -> Unit,
    onStartEditing: (String?) -> Unit,
    onEditSegment: (String?) -> Unit,
    onUpdateText: (String, String) -> Unit,
    onUpdateTranslation: (String, String) -> Unit,
    onDeleteSegment: (String) -> Unit,
    onMergeSegment: (String) -> Unit,
    onSplitSegment: (String, String, String, String?, String?) -> Unit,
    onUndoEdit: () -> Unit,
    onRedoEdit: () -> Unit,
    onSaveEdit: () -> Unit,
    onDiscardEdit: () -> Unit,
    onFlushEdit: () -> Unit,
) {
    val resolvedDetail = detail.forHistory(historyId)
    val fallbackTitle = stringResource(R.string.library_detail_heading)

    var moreMenuExpanded by remember { mutableStateOf(false) }
    var aiSheetVisible by remember { mutableStateOf(false) }
    var tagsDialogVisible by remember { mutableStateOf(false) }
    var snapshotsDialogVisible by remember { mutableStateOf(false) }
    var titleEditorVisible by remember { mutableStateOf(false) }
    var titleInput by remember(item?.historyId, item?.title) { mutableStateOf(item?.title.orEmpty()) }
    var tagCreatorVisible by remember { mutableStateOf(false) }
    var tagNameInput by remember { mutableStateOf("") }
    var exportDialogVisible by remember { mutableStateOf(false) }
    var exportFormat by remember { mutableStateOf(TranscriptExportFormat.TXT) }
    var exportMode by remember { mutableStateOf(TranscriptExportMode.ORIGINAL) }
    var pendingExport by remember { mutableStateOf<Pair<TranscriptExportFormat, TranscriptExportMode>?>(null) }
    var translateDialogVisible by remember { mutableStateOf(false) }
    var targetLanguage by remember(appLanguage) {
        mutableStateOf(appLanguage.takeUnless { it == AppLanguage.SYSTEM } ?: AppLanguage.ENGLISH)
    }
    var exitPending by remember { mutableStateOf(false) }

    val requestExit = {
        if (editor.dirty) exitPending = true else onNavigateBack()
    }
    BackHandler(onBack = requestExit)

    LaunchedEffect(exitRequestToken) {
        if (exitRequestToken > 0) requestExit()
    }
    LaunchedEffect(editor.active, exitPending) {
        if (exitPending && !editor.active) {
            exitPending = false
            onNavigateBack()
        }
    }

    val lifecycleOwner = LocalLifecycleOwner.current
    DisposableEffect(lifecycleOwner) {
        val observer = LifecycleEventObserver { _, event ->
            if (event == Lifecycle.Event.ON_STOP) {
                onPausePlayback()
                onFlushEdit()
            }
        }
        lifecycleOwner.lifecycle.addObserver(observer)
        onDispose {
            lifecycleOwner.lifecycle.removeObserver(observer)
            onReleasePlayback()
            onFlushEdit()
        }
    }

    // Dialogs
    if (exitPending) {
        AlertDialog(
            onDismissRequest = { exitPending = false },
            title = { Text(stringResource(R.string.transcript_unsaved_title)) },
            text = { Text(stringResource(R.string.transcript_unsaved_body)) },
            confirmButton = {
                TextButton(enabled = !editor.saving, onClick = onSaveEdit) {
                    Text(stringResource(R.string.action_save))
                }
            },
            dismissButton = {
                Row {
                    TextButton(onClick = {
                        onDiscardEdit()
                        exitPending = false
                        onNavigateBack()
                    }) { Text(stringResource(R.string.action_discard)) }
                    TextButton(onClick = { exitPending = false }) {
                        Text(stringResource(R.string.action_cancel))
                    }
                }
            },
        )
    }

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
                    modifier = Modifier.fillMaxWidth(),
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
                    modifier = Modifier.fillMaxWidth(),
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

    if (tagsDialogVisible && item != null) {
        AlertDialog(
            onDismissRequest = { tagsDialogVisible = false },
            title = { Text(stringResource(R.string.history_tags)) },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    if (tags.isEmpty()) {
                        Text(
                            text = stringResource(R.string.history_scope_untagged),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .horizontalScroll(rememberScrollState()),
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
                        }
                    }
                    FilledTonalButton(
                        onClick = { tagCreatorVisible = true },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(stringResource(R.string.history_create_tag))
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { tagsDialogVisible = false }) {
                    Text(stringResource(R.string.action_done))
                }
            },
        )
    }

    if (snapshotsDialogVisible) {
        AlertDialog(
            onDismissRequest = { snapshotsDialogVisible = false },
            title = { Text(stringResource(R.string.history_snapshots)) },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    if (snapshots.isEmpty()) {
                        Text(
                            text = stringResource(R.string.history_snapshot_empty),
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    } else {
                        snapshots.forEach { snapshot ->
                            Card(
                                onClick = {
                                    snapshotsDialogVisible = false
                                    onLoadSnapshot(snapshot.id)
                                },
                                colors = CardDefaults.cardColors(
                                    containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                                ),
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = 14.dp, vertical = 10.dp),
                                    horizontalArrangement = Arrangement.SpaceBetween,
                                    verticalAlignment = Alignment.CenterVertically,
                                ) {
                                    Text(
                                        text = stringResource(snapshot.reason.labelRes()),
                                        style = MaterialTheme.typography.bodyMedium,
                                        fontWeight = FontWeight.Medium,
                                    )
                                    Text(
                                        text = formatLibraryTimestamp(snapshot.createdAtEpochMillis),
                                        style = MaterialTheme.typography.labelSmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                            }
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { snapshotsDialogVisible = false }) {
                    Text(stringResource(R.string.action_close))
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

    if (translateDialogVisible) {
        AlertDialog(
            onDismissRequest = { translateDialogVisible = false },
            title = { Text(stringResource(R.string.llm_translate_title)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    AppLanguage.entries.filter { it != AppLanguage.SYSTEM }.forEach { language ->
                        val languageName = stringResource(language.labelRes)
                        TextButton(
                            onClick = {
                                targetLanguage = language
                                translateDialogVisible = false
                                onTranslate(language.languageTag, languageName)
                            },
                        ) {
                            Text(languageName)
                        }
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { translateDialogVisible = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    if (llm.needsConfiguration) {
        AlertDialog(
            onDismissRequest = onClearLlmConfigurationPrompt,
            title = { Text(stringResource(R.string.llm_not_configured)) },
            text = { Text(stringResource(R.string.llm_configure_prompt)) },
            confirmButton = {
                TextButton(onClick = {
                    onClearLlmConfigurationPrompt()
                    onConfigureLlm()
                }) {
                    Text(stringResource(R.string.action_configure))
                }
            },
            dismissButton = {
                TextButton(onClick = onClearLlmConfigurationPrompt) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    snapshotDetail?.takeIf { it.metadata.historyId == historyId }?.let { snapshot ->
        AlertDialog(
            onDismissRequest = onCloseSnapshot,
            title = {
                Text(stringResource(R.string.history_snapshot_title, stringResource(snapshot.metadata.reason.labelRes())))
            },
            text = {
                Box(modifier = Modifier.height(320.dp)) {
                    TranscriptDetail(snapshot.segments)
                }
            },
            confirmButton = {
                TextButton(onClick = onCloseSnapshot) {
                    Text(stringResource(R.string.action_close))
                }
            },
        )
    }

    // AI Assistant Bottom Sheet
    if (aiSheetVisible) {
        ModalBottomSheet(
            onDismissRequest = { aiSheetVisible = false },
            sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
        ) {
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp, vertical = 12.dp)
                    .verticalScroll(rememberScrollState()),
                verticalArrangement = Arrangement.spacedBy(16.dp),
            ) {
                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.SpaceBetween,
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Row(verticalAlignment = Alignment.CenterVertically) {
                        Icon(
                            imageVector = Icons.Rounded.AutoAwesome,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            text = stringResource(R.string.action_ai_actions),
                            style = MaterialTheme.typography.titleLarge,
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                    IconButton(onClick = { aiSheetVisible = false }) {
                        Icon(Icons.Rounded.Close, stringResource(R.string.action_close))
                    }
                }

                Row(
                    modifier = Modifier.fillMaxWidth(),
                    horizontalArrangement = Arrangement.spacedBy(10.dp),
                ) {
                    FilledTonalButton(
                        onClick = onSummarize,
                        enabled = !editor.dirty && llm.task !is LlmTaskState.Running,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(stringResource(R.string.llm_summarize))
                    }
                    FilledTonalButton(
                        onClick = { translateDialogVisible = true },
                        enabled = !editor.dirty && llm.task !is LlmTaskState.Running,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(stringResource(R.string.llm_translate))
                    }
                    FilledTonalButton(
                        onClick = onPolish,
                        enabled = !editor.dirty && llm.task !is LlmTaskState.Running,
                        modifier = Modifier.weight(1f),
                    ) {
                        Text(stringResource(R.string.llm_polish))
                    }
                }

                when (val task = llm.task) {
                    is LlmTaskState.Running -> {
                        Card(
                            modifier = Modifier.fillMaxWidth(),
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.surfaceContainerHigh,
                            ),
                        ) {
                            Column(
                                modifier = Modifier.padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Text(
                                    text = stringResource(R.string.llm_progress, task.progress.percent),
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                                LinearProgressIndicator(
                                    progress = { task.progress.percent / 100f },
                                    modifier = Modifier.fillMaxWidth(),
                                )
                            }
                        }
                    }
                    is LlmTaskState.Failed -> {
                        Card(
                            modifier = Modifier.fillMaxWidth(),
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.errorContainer,
                                contentColor = MaterialTheme.colorScheme.onErrorContainer,
                            ),
                        ) {
                            Column(
                                modifier = Modifier.padding(16.dp),
                                verticalArrangement = Arrangement.spacedBy(8.dp),
                            ) {
                                Text(
                                    text = stringResource(task.category.toStringResource()),
                                    style = MaterialTheme.typography.bodyMedium,
                                )
                                FilledTonalButton(onClick = onRetryLlm) {
                                    Text(stringResource(R.string.llm_retry))
                                }
                            }
                        }
                    }
                    else -> Unit
                }

                llm.summary?.content?.takeIf(String::isNotBlank)?.let { summary ->
                    Card(
                        modifier = Modifier.fillMaxWidth(),
                        shape = MaterialTheme.shapes.medium,
                        colors = CardDefaults.cardColors(
                            containerColor = MaterialTheme.colorScheme.surfaceContainerLow,
                        ),
                    ) {
                        Column(
                            modifier = Modifier.padding(16.dp),
                            verticalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Row(
                                modifier = Modifier.fillMaxWidth(),
                                horizontalArrangement = Arrangement.SpaceBetween,
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Text(
                                    text = stringResource(R.string.llm_summarize),
                                    style = MaterialTheme.typography.titleMedium,
                                    fontWeight = FontWeight.SemiBold,
                                    color = MaterialTheme.colorScheme.primary,
                                )
                                val clipboardManager = LocalClipboardManager.current
                                IconButton(onClick = {
                                    clipboardManager.setText(AnnotatedString(summary))
                                }) {
                                    Icon(
                                        imageVector = Icons.Rounded.ContentCopy,
                                        contentDescription = stringResource(R.string.action_copy),
                                        modifier = Modifier.size(20.dp),
                                    )
                                }
                            }
                            Text(
                                text = summary,
                                style = MaterialTheme.typography.bodyMedium,
                            )
                        }
                    }
                }

                Spacer(Modifier.height(24.dp))
            }
        }
    }

    // Main Layout: Text-Centric
    Box(
        modifier = Modifier.fillMaxSize(),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .widthIn(max = 840.dp),
        ) {
            // 1. Top Bar
            Row(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 8.dp, vertical = 6.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                IconButton(onClick = requestExit) {
                    Icon(
                        imageVector = Icons.AutoMirrored.Rounded.ArrowBack,
                        contentDescription = stringResource(R.string.action_back),
                    )
                }

                Column(
                    modifier = Modifier
                        .weight(1f)
                        .padding(horizontal = 8.dp),
                ) {
                    Text(
                        text = item?.title?.ifBlank { fallbackTitle } ?: fallbackTitle,
                        style = MaterialTheme.typography.titleLarge,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    item?.let {
                        Text(
                            text = "${formatLibraryTimestamp(it.timestampEpochMillis)} · ${formatMediaTime(it.durationMillis)}",
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // AI Action Button
                val isAiRunning = llm.task is LlmTaskState.Running
                IconButton(
                    onClick = { aiSheetVisible = true },
                    enabled = !editor.dirty,
                ) {
                    if (isAiRunning) {
                        CircularProgressIndicator(
                            modifier = Modifier.size(20.dp),
                            strokeWidth = 2.dp,
                        )
                    } else {
                        Icon(
                            imageVector = Icons.Rounded.AutoAwesome,
                            contentDescription = stringResource(R.string.action_ai_actions),
                            tint = if (llm.summary != null) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // Edit Transcript Button
                if (resolvedDetail is LibraryDetailUiState.Ready && item != null &&
                    item.status == HistoryItemStatus.COMPLETE && item.deletedAtEpochMillis == null
                ) {
                    IconButton(
                        onClick = {
                            if (!editor.active) onStartEditing(null)
                        },
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.Edit,
                            contentDescription = stringResource(R.string.transcript_edit),
                            tint = if (editor.active) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                        )
                    }
                }

                // Overflow More Options
                Box {
                    IconButton(onClick = { moreMenuExpanded = true }) {
                        Icon(
                            imageVector = Icons.Rounded.MoreVert,
                            contentDescription = stringResource(R.string.action_more_options),
                        )
                    }
                    DropdownMenu(
                        expanded = moreMenuExpanded,
                        onDismissRequest = { moreMenuExpanded = false },
                    ) {
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.history_edit_title)) },
                            onClick = {
                                moreMenuExpanded = false
                                titleEditorVisible = true
                            },
                            leadingIcon = { Icon(Icons.Rounded.Edit, null) },
                            enabled = item != null && !operationInProgress,
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.history_export_transcript)) },
                            onClick = {
                                moreMenuExpanded = false
                                exportDialogVisible = true
                            },
                            leadingIcon = { Icon(Icons.Rounded.FileDownload, null) },
                            enabled = resolvedDetail is LibraryDetailUiState.Ready && !operationInProgress,
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.history_tags)) },
                            onClick = {
                                moreMenuExpanded = false
                                tagsDialogVisible = true
                            },
                            leadingIcon = { Icon(Icons.Rounded.Label, null) },
                            enabled = item != null && !operationInProgress,
                        )
                        DropdownMenuItem(
                            text = { Text(stringResource(R.string.history_snapshots)) },
                            onClick = {
                                moreMenuExpanded = false
                                snapshotsDialogVisible = true
                            },
                            leadingIcon = { Icon(Icons.Rounded.History, null) },
                        )
                        if (item?.audioAvailable == true) {
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.action_transcribe_current_engine)) },
                                onClick = {
                                    moreMenuExpanded = false
                                    onTranscribeWithCurrentEngine(item)
                                },
                                leadingIcon = { Icon(Icons.Rounded.Replay, null) },
                                enabled = !editor.dirty,
                            )
                            DropdownMenuItem(
                                text = { Text(stringResource(R.string.action_cloud_transcribe)) },
                                onClick = {
                                    moreMenuExpanded = false
                                    onTranscribeWithCloud(item)
                                },
                                leadingIcon = { Icon(Icons.Rounded.CloudSync, null) },
                                enabled = !editor.dirty && cloudTranscription !is CloudTranscriptionUiState.Running,
                            )
                        }
                    }
                }
            }

            // Sub-header compact items
            if (item != null && item.tagIds.isNotEmpty()) {
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState())
                        .padding(horizontal = 16.dp, vertical = 2.dp),
                    horizontalArrangement = Arrangement.spacedBy(6.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    tags.filter { it.id in item.tagIds }.forEach { tag ->
                        FilterChip(
                            selected = true,
                            onClick = { tagsDialogVisible = true },
                            label = { Text(tag.name, style = MaterialTheme.typography.labelSmall) },
                            modifier = Modifier.height(28.dp),
                        )
                    }
                }
            }

            if (item?.status == HistoryItemStatus.DRAFT) {
                Card(
                    shape = MaterialTheme.shapes.small,
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.tertiaryContainer,
                        contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
                    ),
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 4.dp),
                ) {
                    Text(
                        text = stringResource(R.string.library_draft_notice),
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 6.dp),
                    )
                }
            }

            CloudTranscriptionStatus(historyId = historyId, state = cloudTranscription)

            llm.summary?.content?.takeIf(String::isNotBlank)?.let { summary ->
                Card(
                    shape = MaterialTheme.shapes.small,
                    colors = CardDefaults.cardColors(
                        containerColor = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.4f),
                        contentColor = MaterialTheme.colorScheme.onPrimaryContainer,
                    ),
                    onClick = { aiSheetVisible = true },
                    modifier = Modifier
                        .fillMaxWidth()
                        .padding(horizontal = 16.dp, vertical = 4.dp),
                ) {
                    Row(
                        modifier = Modifier.padding(horizontal = 12.dp, vertical = 8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        Icon(
                            imageVector = Icons.Rounded.AutoAwesome,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.primary,
                            modifier = Modifier.size(16.dp),
                        )
                        Spacer(Modifier.width(8.dp))
                        Text(
                            text = summary,
                            style = MaterialTheme.typography.bodySmall,
                            maxLines = 1,
                            overflow = TextOverflow.Ellipsis,
                            modifier = Modifier.weight(1f),
                        )
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = stringResource(R.string.home_view_all),
                            style = MaterialTheme.typography.labelSmall,
                            color = MaterialTheme.colorScheme.primary,
                            fontWeight = FontWeight.SemiBold,
                        )
                    }
                }
            }

            if (operationError) {
                Text(
                    text = stringResource(R.string.history_operation_failed),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.bodySmall,
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 2.dp),
                )
            }

            // 2. Main Content: Transcript (TEXT AS PRIMARY FOCUS)
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .padding(horizontal = 16.dp),
            ) {
                when (resolvedDetail) {
                    is LibraryDetailUiState.Loading -> LibraryLoading(modifier = Modifier.fillMaxSize())
                    is LibraryDetailUiState.Failed -> LibraryTranscriptError(
                        onRetry = onRetry,
                        modifier = Modifier.fillMaxSize(),
                    )
                    is LibraryDetailUiState.Ready -> TranscriptDetail(
                        segments = if (editor.historyId == historyId) editor.draftSegments else resolvedDetail.segments,
                        editor = editor.takeIf { it.historyId == historyId },
                        playbackPositionMillis = playback.takeIf { it.historyId == historyId }?.positionMillis,
                        onSeek = onSeekPlayback,
                        onEditSegment = onEditSegment,
                        onUpdateText = onUpdateText,
                        onUpdateTranslation = onUpdateTranslation,
                        onDeleteSegment = onDeleteSegment,
                        onMergeSegment = onMergeSegment,
                        onSplitSegment = onSplitSegment,
                        modifier = Modifier.fillMaxSize(),
                    )
                    LibraryDetailUiState.None -> Unit
                }
            }

            // 3. Bottom Docked Bar (Player or Edit Bar)
            if (editor.active && editor.historyId == historyId) {
                DockedEditToolbar(
                    editor = editor,
                    onUndo = onUndoEdit,
                    onRedo = onRedoEdit,
                    onSave = onSaveEdit,
                    onDiscard = onDiscardEdit,
                )
            } else if (playback.historyId == historyId && item?.audioAvailable == true) {
                DockedAudioPlayer(
                    state = playback,
                    onToggle = onTogglePlayback,
                    onSeek = onSeekPlayback,
                    onSkip = onSkipPlayback,
                    onSetSpeed = onSetPlaybackSpeed,
                )
            }
        }
    }
}

@Composable
private fun DockedAudioPlayer(
    state: AudioPlaybackState,
    onToggle: () -> Unit,
    onSeek: (Long) -> Unit,
    onSkip: (Long) -> Unit,
    onSetSpeed: (Float) -> Unit,
    modifier: Modifier = Modifier,
) {
    val duration = state.durationMillis.coerceAtLeast(0L)
    val position = state.positionMillis.coerceIn(0L, duration.coerceAtLeast(1L))
    var speedMenuExpanded by remember { mutableStateOf(false) }

    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceContainer,
        tonalElevation = 3.dp,
        shadowElevation = 8.dp,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 6.dp),
        ) {
            Slider(
                value = position.toFloat(),
                onValueChange = { onSeek(it.toLong()) },
                valueRange = 0f..duration.coerceAtLeast(1L).toFloat(),
                enabled = duration > 0 && state.status !is AudioPlaybackStatus.Failed,
                modifier = Modifier
                    .fillMaxWidth()
                    .height(24.dp)
                    .semantics {
                        contentDescription =
                            "Playback position ${formatMediaTime(position)} of ${formatMediaTime(duration)}"
                    },
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = "${formatMediaTime(position)} / ${formatMediaTime(duration)}",
                    style = MaterialTheme.typography.labelMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )

                Row(verticalAlignment = Alignment.CenterVertically) {
                    IconButton(onClick = { onSkip(-5_000) }) {
                        Icon(
                            imageVector = Icons.Rounded.Replay5,
                            contentDescription = stringResource(R.string.playback_back_five),
                            modifier = Modifier.size(22.dp),
                        )
                    }
                    FloatingActionButton(
                        onClick = onToggle,
                        shape = CircleShape,
                        containerColor = MaterialTheme.colorScheme.primary,
                        contentColor = MaterialTheme.colorScheme.onPrimary,
                        modifier = Modifier.size(42.dp),
                        elevation = FloatingActionButtonDefaults.elevation(defaultElevation = 2.dp),
                    ) {
                        Icon(
                            imageVector = if (state.status == AudioPlaybackStatus.Playing) {
                                Icons.Rounded.Pause
                            } else {
                                Icons.Rounded.PlayArrow
                            },
                            contentDescription = stringResource(
                                if (state.status == AudioPlaybackStatus.Playing) {
                                    R.string.playback_pause
                                } else {
                                    R.string.playback_play
                                },
                            ),
                            modifier = Modifier.size(24.dp),
                        )
                    }
                    IconButton(onClick = { onSkip(5_000) }) {
                        Icon(
                            imageVector = Icons.Rounded.Forward5,
                            contentDescription = stringResource(R.string.playback_forward_five),
                            modifier = Modifier.size(22.dp),
                        )
                    }
                }

                Box {
                    TextButton(onClick = { speedMenuExpanded = true }) {
                        Text(
                            text = "${state.speed}x",
                            style = MaterialTheme.typography.labelLarge,
                            fontWeight = FontWeight.Bold,
                        )
                    }
                    DropdownMenu(
                        expanded = speedMenuExpanded,
                        onDismissRequest = { speedMenuExpanded = false },
                    ) {
                        listOf(0.5f, 0.8f, 1f, 1.25f, 1.5f, 2f, 3f).forEach { speed ->
                            DropdownMenuItem(
                                text = { Text("${speed}x") },
                                onClick = {
                                    onSetSpeed(speed)
                                    speedMenuExpanded = false
                                },
                            )
                        }
                    }
                }
            }
            (state.status as? AudioPlaybackStatus.Failed)?.let {
                Text(
                    text = stringResource(R.string.playback_failed),
                    color = MaterialTheme.colorScheme.error,
                    style = MaterialTheme.typography.labelSmall,
                )
            }
        }
    }
}

@Composable
private fun DockedEditToolbar(
    editor: TranscriptEditorUiState,
    onUndo: () -> Unit,
    onRedo: () -> Unit,
    onSave: () -> Unit,
    onDiscard: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        modifier = modifier.fillMaxWidth(),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        tonalElevation = 3.dp,
        shadowElevation = 8.dp,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onUndo, enabled = editor.undoAvailable && !editor.saving) {
                    Icon(Icons.Rounded.Undo, stringResource(R.string.action_undo))
                }
                IconButton(onClick = onRedo, enabled = editor.redoAvailable && !editor.saving) {
                    Icon(Icons.Rounded.Redo, stringResource(R.string.action_redo))
                }
            }

            val statusText = when {
                editor.error != null -> when (editor.error) {
                    TranscriptEditorError.INVALID_EDIT -> stringResource(R.string.transcript_edit_invalid)
                    TranscriptEditorError.SAVE_FAILED -> stringResource(R.string.transcript_edit_save_failed)
                    TranscriptEditorError.STALE_TRANSCRIPT -> stringResource(R.string.transcript_edit_conflict)
                }
                editor.saving -> stringResource(R.string.transcript_auto_save_saving)
                editor.dirty -> stringResource(R.string.transcript_auto_save_unsaved)
                else -> stringResource(R.string.transcript_auto_save_saved)
            }
            val statusColor = if (editor.error != null || editor.dirty) {
                MaterialTheme.colorScheme.error
            } else {
                MaterialTheme.colorScheme.onSurfaceVariant
            }

            Text(
                text = statusText,
                style = MaterialTheme.typography.labelMedium,
                color = statusColor,
            )

            Row(verticalAlignment = Alignment.CenterVertically) {
                TextButton(onClick = onDiscard, enabled = !editor.saving) {
                    Text(stringResource(if (editor.dirty) R.string.action_discard else R.string.action_done))
                }
                FilledTonalButton(onClick = onSave, enabled = editor.dirty && !editor.saving) {
                    Icon(Icons.Rounded.Save, null, modifier = Modifier.size(18.dp))
                    Spacer(Modifier.width(6.dp))
                    Text(stringResource(R.string.action_save))
                }
            }
        }
    }
}

private fun LlmFailureCategory.toStringResource(): Int = when (this) {
    LlmFailureCategory.NOT_CONFIGURED -> R.string.llm_not_configured
    LlmFailureCategory.AUTHENTICATION -> R.string.llm_error_authentication
    LlmFailureCategory.NETWORK -> R.string.llm_error_network
    LlmFailureCategory.RATE_LIMITED -> R.string.llm_error_rate_limited
    LlmFailureCategory.INVALID_RESPONSE -> R.string.llm_error_invalid_response
    LlmFailureCategory.UNSUPPORTED -> R.string.llm_error_unsupported
    LlmFailureCategory.UNKNOWN -> R.string.llm_error_unknown
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
                Row(
                    modifier = Modifier
                        .fillMaxWidth()
                        .horizontalScroll(rememberScrollState()),
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
private fun CloudTranscriptionStatus(
    historyId: String,
    state: CloudTranscriptionUiState,
) {
    val message = when (state) {
        CloudTranscriptionUiState.Idle -> null
        is CloudTranscriptionUiState.Running ->
            state.takeIf { it.historyId == historyId }?.let {
                CloudTranscriptionMessage(R.string.cloud_transcription_running, isError = false)
            }
        is CloudTranscriptionUiState.Completed ->
            state.takeIf { it.historyId == historyId }?.let {
                CloudTranscriptionMessage(R.string.cloud_transcription_completed, isError = false)
            }
        is CloudTranscriptionUiState.Failed ->
            state.takeIf { it.historyId == historyId }?.let {
                CloudTranscriptionMessage(it.reason.messageRes, isError = true)
            }
    } ?: return

    Spacer(Modifier.height(4.dp))
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
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp),
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

internal fun LibraryDetailUiState.forHistory(historyId: String): LibraryDetailUiState = when (this) {
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
    editor: TranscriptEditorUiState? = null,
    playbackPositionMillis: Long? = null,
    onSeek: (Long) -> Unit = {},
    onEditSegment: (String?) -> Unit = {},
    onUpdateText: (String, String) -> Unit = { _, _ -> },
    onUpdateTranslation: (String, String) -> Unit = { _, _ -> },
    onDeleteSegment: (String) -> Unit = {},
    onMergeSegment: (String) -> Unit = {},
    onSplitSegment: (String, String, String, String?, String?) -> Unit = { _, _, _, _, _ -> },
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
        if (activeIndex != null && editor?.active != true) listState.animateScrollToItem(activeIndex)
    }
    LazyColumn(
        state = listState,
        modifier = modifier.fillMaxWidth(),
        verticalArrangement = Arrangement.spacedBy(10.dp),
        contentPadding = PaddingValues(vertical = 8.dp),
    ) {
        items(segments, key = TranscriptSegment::id) { segment ->
            val active = playbackPositionMillis?.let { position ->
                position >= (segment.startSeconds * 1_000).toLong() &&
                    position < (segment.endSeconds * 1_000).toLong()
            } == true
            val editing = editor?.editingSegmentId == segment.id
            Card(
                shape = MaterialTheme.shapes.medium,
                colors = CardDefaults.cardColors(
                    containerColor = if (active) {
                        MaterialTheme.colorScheme.secondaryContainer
                    } else {
                        MaterialTheme.colorScheme.surfaceContainerHigh
                    },
                ),
                onClick = {
                    if (editor?.active == true) {
                        onEditSegment(segment.id)
                    } else {
                        onSeek((segment.startSeconds * 1_000).toLong())
                    }
                },
                modifier = Modifier.fillMaxWidth(),
            ) {
                Column(
                    modifier = Modifier.padding(horizontal = 16.dp, vertical = 12.dp),
                ) {
                    segment.speaker?.label?.takeIf(String::isNotBlank)?.let { speaker ->
                        Card(
                            shape = MaterialTheme.shapes.extraSmall,
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.primary.copy(alpha = 0.12f),
                                contentColor = MaterialTheme.colorScheme.primary,
                            ),
                            modifier = Modifier.padding(bottom = 6.dp),
                        ) {
                            Text(
                                text = speaker,
                                style = MaterialTheme.typography.labelSmall,
                                fontWeight = FontWeight.Bold,
                                modifier = Modifier.padding(horizontal = 8.dp, vertical = 3.dp),
                            )
                        }
                    }
                    Text(
                        text = formatMediaTime((segment.startSeconds * 1_000).toLong()),
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.primary,
                    )
                    if (editing) {
                        TranscriptSegmentEditor(
                            segment = segment,
                            hasNext = segments.lastOrNull()?.id != segment.id,
                            onUpdateText = { onUpdateText(segment.id, it) },
                            onUpdateTranslation = { onUpdateTranslation(segment.id, it) },
                            onDelete = { onDeleteSegment(segment.id) },
                            onMerge = { onMergeSegment(segment.id) },
                            onSplit = { left, right, leftTranslation, rightTranslation ->
                                onSplitSegment(segment.id, left, right, leftTranslation, rightTranslation)
                            },
                        )
                    } else {
                        Text(
                            text = highlightedSegmentText(
                                segment,
                                playbackPositionMillis,
                                MaterialTheme.colorScheme.primaryContainer,
                            ),
                            style = MaterialTheme.typography.bodyLarge,
                            color = MaterialTheme.colorScheme.onSurface,
                        )
                        segment.translation?.takeIf(String::isNotBlank)?.let { translation ->
                            Spacer(Modifier.height(6.dp))
                            Text(
                                text = translation,
                                style = MaterialTheme.typography.bodyMedium,
                                color = MaterialTheme.colorScheme.onSurfaceVariant,
                            )
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun TranscriptSegmentEditor(
    segment: TranscriptSegment,
    hasNext: Boolean,
    onUpdateText: (String) -> Unit,
    onUpdateTranslation: (String) -> Unit,
    onDelete: () -> Unit,
    onMerge: () -> Unit,
    onSplit: (String, String, String?, String?) -> Unit,
) {
    var textValue by remember(segment.id) { mutableStateOf(TextFieldValue(segment.text)) }
    var translationValue by remember(segment.id) { mutableStateOf(TextFieldValue(segment.translation.orEmpty())) }
    var splitVisible by remember(segment.id) { mutableStateOf(false) }

    if (splitVisible) {
        val splitIndex = textValue.selection.start.coerceIn(1, (textValue.text.length - 1).coerceAtLeast(1))
        val leftText = textValue.text.substring(0, splitIndex).trim()
        val rightText = textValue.text.substring(splitIndex).trim()
        val translationParts = splitTranslation(translationValue.text, splitIndex, textValue.text.length)
        var leftTranslation by remember(splitIndex, translationValue.text) { mutableStateOf(translationParts.first) }
        var rightTranslation by remember(splitIndex, translationValue.text) { mutableStateOf(translationParts.second) }
        AlertDialog(
            onDismissRequest = { splitVisible = false },
            title = { Text(stringResource(R.string.transcript_split)) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(leftText, {}, readOnly = true, label = { Text(stringResource(R.string.transcript_left_original)) })
                    OutlinedTextField(rightText, {}, readOnly = true, label = { Text(stringResource(R.string.transcript_right_original)) })
                    if (segment.translation != null) {
                        OutlinedTextField(leftTranslation, { leftTranslation = it }, label = { Text(stringResource(R.string.transcript_left_translation)) })
                        OutlinedTextField(rightTranslation, { rightTranslation = it }, label = { Text(stringResource(R.string.transcript_right_translation)) })
                    }
                }
            },
            confirmButton = {
                TextButton(
                    enabled = leftText.isNotBlank() && rightText.isNotBlank() &&
                        (segment.translation == null || (leftTranslation.isNotBlank() && rightTranslation.isNotBlank())),
                    onClick = {
                        onSplit(leftText, rightText, leftTranslation.takeIf { segment.translation != null }, rightTranslation.takeIf { segment.translation != null })
                        splitVisible = false
                    },
                ) { Text(stringResource(R.string.transcript_split)) }
            },
            dismissButton = { TextButton(onClick = { splitVisible = false }) { Text(stringResource(R.string.action_cancel)) } },
        )
    }

    OutlinedTextField(
        value = textValue,
        onValueChange = {
            textValue = it
            onUpdateText(it.text)
        },
        label = { Text(stringResource(R.string.transcript_original)) },
        modifier = Modifier.fillMaxWidth(),
    )
    Spacer(Modifier.height(8.dp))
    OutlinedTextField(
        value = translationValue,
        onValueChange = {
            translationValue = it
            onUpdateTranslation(it.text)
        },
        label = { Text(stringResource(R.string.transcript_translation)) },
        modifier = Modifier.fillMaxWidth(),
    )
    Row(
        modifier = Modifier.fillMaxWidth(),
        horizontalArrangement = Arrangement.End,
    ) {
        IconButton(onClick = { splitVisible = true }, enabled = textValue.selection.start in 1 until textValue.text.length) {
            Icon(Icons.Rounded.CallSplit, stringResource(R.string.transcript_split))
        }
        IconButton(onClick = onMerge, enabled = hasNext) {
            Icon(Icons.Rounded.CallMerge, stringResource(R.string.transcript_merge_next))
        }
        IconButton(onClick = onDelete) {
            Icon(Icons.Rounded.Delete, stringResource(R.string.action_delete))
        }
    }
}

private fun splitTranslation(value: String, sourceIndex: Int, sourceLength: Int): Pair<String, String> {
    if (value.isBlank()) return "" to ""
    val target = ((sourceIndex.toDouble() / sourceLength.coerceAtLeast(1)) * value.length).toInt()
    val boundaries = value.indices.filter { index -> value[index].isWhitespace() }
    val boundary = boundaries.minByOrNull { kotlin.math.abs(it - target) } ?: target
    return value.substring(0, boundary).trim() to value.substring(boundary).trim()
}

private fun highlightedSegmentText(
    segment: TranscriptSegment,
    playbackPositionMillis: Long?,
    highlightColor: androidx.compose.ui.graphics.Color,
) = buildAnnotatedString {
    append(segment.text)
    val positionSeconds = playbackPositionMillis?.div(1_000.0) ?: return@buildAnnotatedString
    val unit = segment.timing?.units?.firstOrNull {
        positionSeconds >= it.startSeconds && positionSeconds < it.endSeconds
    } ?: return@buildAnnotatedString
    val start = segment.text.indexOf(unit.text).takeIf { it >= 0 } ?: return@buildAnnotatedString
    addStyle(SpanStyle(background = highlightColor), start, (start + unit.text.length).coerceAtMost(segment.text.length))
}

private fun formatMediaTime(millis: Long): String {
    val totalSeconds = millis.coerceAtLeast(0L) / 1_000
    val minutes = totalSeconds / 60
    val seconds = totalSeconds % 60
    return "%d:%02d".format(Locale.US, minutes, seconds)
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
                shape = MaterialTheme.shapes.medium,
            )
            .padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = stringResource(R.string.library_transcript_load_failed),
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
            )
            Spacer(Modifier.height(12.dp))
            FilledTonalButton(onClick = onRetry) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}
