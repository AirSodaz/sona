package com.sona.android.app.feature.library

import androidx.activity.compose.BackHandler
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.annotation.StringRes
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.Redo
import androidx.compose.material.icons.automirrored.rounded.Undo
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.CloudSync
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.FileDownload
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Forward5
import androidx.compose.material.icons.rounded.History
import androidx.compose.material.icons.rounded.MoreVert
import androidx.compose.material.icons.rounded.Pause
import androidx.compose.material.icons.rounded.PlayArrow
import androidx.compose.material.icons.rounded.Replay
import androidx.compose.material.icons.rounded.Replay5
import androidx.compose.material.icons.rounded.Save
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FloatingActionButton
import androidx.compose.material3.FloatingActionButtonDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.SegmentedButton
import androidx.compose.material3.SegmentedButtonDefaults
import androidx.compose.material3.SingleChoiceSegmentedButtonRow
import androidx.compose.material3.Slider
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.LifecycleEventObserver
import androidx.lifecycle.compose.LocalLifecycleOwner
import com.sona.android.app.R
import com.sona.android.app.feature.settings.AppLanguage
import com.sona.android.app.ui.component.SonaBackButton
import com.sona.android.app.ui.component.SonaTopAppBar
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.ProjectRecord
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.TranscriptSnapshotDetail
import com.sona.android.application.media.AudioPlaybackState
import com.sona.android.application.media.AudioPlaybackStatus
import com.sona.android.application.recording.CloudTranscriptionFailure

private enum class DetailWorkspaceTab {
    TRANSCRIPT,
    AI_SUMMARY,
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
internal fun LibraryDetailScreen(
    historyId: String,
    item: HistoryItem?,
    detail: LibraryDetailUiState,
    cloudTranscription: CloudTranscriptionUiState,
    projects: List<ProjectRecord>,
    snapshots: List<TranscriptSnapshot>,
    snapshotDetail: TranscriptSnapshotDetail?,
    operationInProgress: Boolean,
    operationError: Boolean,
    onRetry: () -> Unit,
    onTranscribeWithCloud: (HistoryItem) -> Unit,
    onTranscribeWithCurrentEngine: (HistoryItem) -> Unit,
    onUpdateTitle: (String) -> Unit,
    onUpdateProject: (String?) -> Unit,
    onCreateProject: (String) -> Unit,
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

    var selectedTab by remember { mutableStateOf(DetailWorkspaceTab.TRANSCRIPT) }
    var moreMenuExpanded by remember { mutableStateOf(false) }
    var projectDialogVisible by remember { mutableStateOf(false) }
    var snapshotsDialogVisible by remember { mutableStateOf(false) }
    var titleEditorVisible by remember { mutableStateOf(false) }
    var titleInput by remember(item?.historyId, item?.title) { mutableStateOf(item?.title.orEmpty()) }
    var projectCreatorVisible by remember { mutableStateOf(false) }
    var projectNameInput by remember { mutableStateOf("") }
    var exportDialogVisible by remember { mutableStateOf(false) }
    var exportFormat by remember { mutableStateOf(TranscriptExportFormat.TXT) }
    var exportMode by remember { mutableStateOf(TranscriptExportMode.ORIGINAL) }
    var pendingExport by remember { mutableStateOf<Pair<TranscriptExportFormat, TranscriptExportMode>?>(null) }
    var exitPending by remember { mutableStateOf(false) }

    val isEditingThis = editor.active && editor.historyId == historyId

    val requestExit = {
        if (editor.dirty) exitPending = true else onNavigateBack()
    }
    BackHandler(enabled = editor.dirty, onBack = requestExit)

    LaunchedEffect(exitRequestToken) {
        if (exitRequestToken > 0) requestExit()
    }
    LaunchedEffect(editor.active, exitPending) {
        if (exitPending && !editor.active) {
            exitPending = false
            onNavigateBack()
        }
    }
    LaunchedEffect(isEditingThis) {
        if (isEditingThis) {
            selectedTab = DetailWorkspaceTab.TRANSCRIPT
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

    // Segment Editor Bottom Sheet
    if (isEditingThis && editor.editingSegmentId != null && resolvedDetail is LibraryDetailUiState.Ready) {
        val segments = editor.draftSegments
        val targetSegment = segments.firstOrNull { it.id == editor.editingSegmentId }
        if (targetSegment != null) {
            TranscriptEditorSheet(
                segment = targetSegment,
                hasNext = segments.lastOrNull()?.id != targetSegment.id,
                onUpdateText = { onUpdateText(targetSegment.id, it) },
                onUpdateTranslation = { onUpdateTranslation(targetSegment.id, it) },
                onDelete = { onDeleteSegment(targetSegment.id) },
                onMerge = { onMergeSegment(targetSegment.id) },
                onSplit = { left, right, leftTranslation, rightTranslation ->
                    onSplitSegment(targetSegment.id, left, right, leftTranslation, rightTranslation)
                },
                onDismiss = { onEditSegment(null) },
            )
        }
    }

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

    if (projectCreatorVisible) {
        AlertDialog(
            onDismissRequest = { projectCreatorVisible = false },
            title = { Text(stringResource(R.string.history_create_project)) },
            text = {
                OutlinedTextField(
                    value = projectNameInput,
                    onValueChange = { projectNameInput = it },
                    label = { Text(stringResource(R.string.history_project_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                TextButton(
                    enabled = projectNameInput.isNotBlank() && !operationInProgress,
                    onClick = {
                        onCreateProject(projectNameInput.trim())
                        projectNameInput = ""
                        projectCreatorVisible = false
                    },
                ) { Text(stringResource(R.string.history_create_project)) }
            },
            dismissButton = {
                TextButton(onClick = { projectCreatorVisible = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    if (projectDialogVisible && item != null) {
        AlertDialog(
            onDismissRequest = { projectDialogVisible = false },
            title = { Text(stringResource(R.string.history_projects)) },
            text = {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .verticalScroll(rememberScrollState()),
                    verticalArrangement = Arrangement.spacedBy(12.dp),
                ) {
                    Row(
                        modifier = Modifier
                            .fillMaxWidth()
                            .horizontalScroll(rememberScrollState()),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                    ) {
                        FilterChip(
                            selected = item.projectId == null,
                            enabled = !operationInProgress,
                            onClick = { onUpdateProject(null) },
                            label = { Text(stringResource(R.string.history_scope_inbox)) },
                        )
                        projects.forEach { project ->
                            val selected = project.id == item.projectId
                            FilterChip(
                                selected = selected,
                                enabled = !operationInProgress,
                                onClick = {
                                    onUpdateProject(if (selected) null else project.id)
                                },
                                label = { Text(project.name) },
                            )
                        }
                    }
                    FilledTonalButton(
                        onClick = { projectCreatorVisible = true },
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(stringResource(R.string.history_create_project))
                    }
                }
            },
            confirmButton = {
                TextButton(onClick = { projectDialogVisible = false }) {
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
                            Surface(
                                onClick = {
                                    snapshotsDialogVisible = false
                                    onLoadSnapshot(snapshot.id)
                                },
                                shape = RoundedCornerShape(14.dp),
                                color = MaterialTheme.colorScheme.surfaceContainerHigh,
                                modifier = Modifier.fillMaxWidth(),
                            ) {
                                Row(
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .padding(horizontal = 14.dp, vertical = 12.dp),
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
                Text(
                    stringResource(
                        R.string.history_snapshot_title,
                        stringResource(snapshot.metadata.reason.labelRes()),
                    ),
                )
            },
            text = {
                Box(modifier = Modifier.height(320.dp)) {
                    TranscriptStream(segments = snapshot.segments)
                }
            },
            confirmButton = {
                TextButton(onClick = onCloseSnapshot) {
                    Text(stringResource(R.string.action_close))
                }
            },
        )
    }

    Box(
        modifier = Modifier
            .fillMaxSize()
            .background(MaterialTheme.colorScheme.background),
        contentAlignment = Alignment.TopCenter,
    ) {
        Column(
            modifier = Modifier
                .fillMaxSize()
                .widthIn(max = 840.dp),
        ) {
            // 1. Top App Bar
            SonaTopAppBar(
                navigationIcon = {
                    SonaBackButton(onClick = requestExit)
                },
                title = {
                    Column(
                        modifier = Modifier
                            .clickable(enabled = item != null && !operationInProgress) {
                                titleEditorVisible = true
                            }
                            .padding(horizontal = 4.dp),
                    ) {
                        Text(
                            text = item?.title?.ifBlank { fallbackTitle } ?: fallbackTitle,
                            style = MaterialTheme.typography.titleMedium,
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
                },
                actions = {
                    if (resolvedDetail is LibraryDetailUiState.Ready && item != null &&
                        item.status == HistoryItemStatus.COMPLETE && item.deletedAtEpochMillis == null
                    ) {
                        IconButton(
                            onClick = {
                                if (!isEditingThis) {
                                    selectedTab = DetailWorkspaceTab.TRANSCRIPT
                                    onStartEditing(null)
                                } else if (!editor.dirty) {
                                    onDiscardEdit()
                                } else {
                                    onSaveEdit()
                                }
                            },
                        ) {
                            Icon(
                                imageVector = if (isEditingThis) Icons.Rounded.Check else Icons.Rounded.Edit,
                                contentDescription = stringResource(R.string.transcript_edit),
                                tint = if (isEditingThis) {
                                    MaterialTheme.colorScheme.primary
                                } else {
                                    MaterialTheme.colorScheme.onSurfaceVariant
                                },
                            )
                        }
                    }

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
                                text = { Text(stringResource(R.string.history_projects)) },
                                onClick = {
                                    moreMenuExpanded = false
                                    projectDialogVisible = true
                                },
                                leadingIcon = { Icon(Icons.Rounded.FolderOpen, null) },
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
                            if (item != null) {
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.action_transcribe_current_engine)) },
                                    onClick = {
                                        moreMenuExpanded = false
                                        onTranscribeWithCurrentEngine(item)
                                    },
                                    leadingIcon = { Icon(Icons.Rounded.Replay, null) },
                                    enabled = item.audioAvailable && !editor.dirty,
                                )
                                DropdownMenuItem(
                                    text = { Text(stringResource(R.string.action_cloud_transcribe)) },
                                    onClick = {
                                        moreMenuExpanded = false
                                        onTranscribeWithCloud(item)
                                    },
                                    leadingIcon = { Icon(Icons.Rounded.CloudSync, null) },
                                    enabled = item.audioAvailable && !editor.dirty && cloudTranscription !is CloudTranscriptionUiState.Running,
                                )
                            }
                        }
                    }
                },
            )

            // 2. Segmented Workspace Switcher (Transcript | AI Summary) + Metadata Chips
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 16.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (!isEditingThis) {
                    SingleChoiceSegmentedButtonRow(
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        DetailWorkspaceTab.entries.forEachIndexed { index, tab ->
                            SegmentedButton(
                                selected = selectedTab == tab,
                                onClick = { selectedTab = tab },
                                shape = SegmentedButtonDefaults.itemShape(
                                    index = index,
                                    count = DetailWorkspaceTab.entries.size,
                                ),
                                label = {
                                    Text(
                                        text = stringResource(
                                            when (tab) {
                                                DetailWorkspaceTab.TRANSCRIPT -> R.string.detail_tab_transcript
                                                DetailWorkspaceTab.AI_SUMMARY -> R.string.detail_tab_ai
                                            },
                                        ),
                                    )
                                },
                            )
                        }
                    }
                }

                if (item != null && item.projectId != null) {
                    val assignedProject = projects.firstOrNull { it.id == item.projectId }
                    if (assignedProject != null) {
                        FilterChip(
                            selected = true,
                            onClick = { projectDialogVisible = true },
                            label = { Text(assignedProject.name, style = MaterialTheme.typography.labelSmall) },
                            modifier = Modifier.height(28.dp),
                        )
                    }
                }

                if (item?.status == HistoryItemStatus.DRAFT) {
                    Surface(
                        shape = RoundedCornerShape(14.dp),
                        color = MaterialTheme.colorScheme.tertiaryContainer,
                        contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        Text(
                            text = stringResource(R.string.library_draft_notice),
                            style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
                        )
                    }
                }

                CloudTranscriptionStatus(historyId = historyId, state = cloudTranscription)

                if (operationError) {
                    Text(
                        text = stringResource(R.string.history_operation_failed),
                        color = MaterialTheme.colorScheme.error,
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
            }

            // 3. Main Content Area
            Box(
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f)
                    .padding(horizontal = 12.dp),
            ) {
                if (selectedTab == DetailWorkspaceTab.AI_SUMMARY && !isEditingThis) {
                    AiWorkspacePane(
                        llm = llm,
                        editorDirty = editor.dirty,
                        appLanguage = appLanguage,
                        onSummarize = onSummarize,
                        onTranslate = onTranslate,
                        onPolish = onPolish,
                        onRetryLlm = onRetryLlm,
                        modifier = Modifier.padding(horizontal = 4.dp),
                    )
                } else {
                    when (resolvedDetail) {
                        is LibraryDetailUiState.Loading -> LibraryLoading(modifier = Modifier.fillMaxSize())
                        is LibraryDetailUiState.Failed -> LibraryTranscriptError(
                            onRetry = onRetry,
                            modifier = Modifier.fillMaxSize(),
                        )
                        is LibraryDetailUiState.Ready -> TranscriptStream(
                            segments = if (editor.historyId == historyId) editor.draftSegments else resolvedDetail.segments,
                            editor = editor.takeIf { it.historyId == historyId },
                            playbackPositionMillis = playback.takeIf { it.historyId == historyId }?.positionMillis,
                            onSeek = onSeekPlayback,
                            onEditSegment = onEditSegment,
                            modifier = Modifier.fillMaxSize(),
                        )
                        LibraryDetailUiState.None -> Unit
                    }
                }
            }

            // 4. Bottom Docked Workspace Bar (Simultaneous Proofread Audio Mini-Bar + Edit Bar OR Full Audio Player)
            val hasAudioPlayer = playback.historyId == historyId && item?.audioAvailable == true
            if (isEditingThis) {
                Column(
                    modifier = Modifier
                        .fillMaxWidth()
                        .navigationBarsPadding(),
                ) {
                    if (hasAudioPlayer) {
                        CompactProofreadAudioBar(
                            state = playback,
                            onToggle = onTogglePlayback,
                            onSkip = onSkipPlayback,
                        )
                    }
                    DockedEditToolbar(
                        editor = editor,
                        onUndo = onUndoEdit,
                        onRedo = onRedoEdit,
                        onSave = onSaveEdit,
                        onDiscard = onDiscardEdit,
                    )
                }
            } else if (hasAudioPlayer) {
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
private fun CompactProofreadAudioBar(
    state: AudioPlaybackState,
    onToggle: () -> Unit,
    onSkip: (Long) -> Unit,
) {
    val duration = state.durationMillis.coerceAtLeast(0L)
    val position = state.positionMillis.coerceIn(0L, duration.coerceAtLeast(1L))
    Surface(
        modifier = Modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp),
        shape = RoundedCornerShape(20.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        tonalElevation = 2.dp,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            FilledTonalIconButton(
                onClick = onToggle,
                modifier = Modifier.size(36.dp),
            ) {
                Icon(
                    imageVector = if (state.status == AudioPlaybackStatus.Playing) {
                        Icons.Rounded.Pause
                    } else {
                        Icons.Rounded.PlayArrow
                    },
                    contentDescription = stringResource(
                        if (state.status == AudioPlaybackStatus.Playing) R.string.playback_pause else R.string.playback_play,
                    ),
                    modifier = Modifier.size(18.dp),
                )
            }
            IconButton(onClick = { onSkip(-5_000) }, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Rounded.Replay5, stringResource(R.string.playback_back_five), modifier = Modifier.size(18.dp))
            }
            IconButton(onClick = { onSkip(5_000) }, modifier = Modifier.size(34.dp)) {
                Icon(Icons.Rounded.Forward5, stringResource(R.string.playback_forward_five), modifier = Modifier.size(18.dp))
            }
            Spacer(Modifier.weight(1f))
            Text(
                text = "${formatMediaTime(position)} / ${formatMediaTime(duration)}",
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
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
        modifier = modifier
            .fillMaxWidth()
            .navigationBarsPadding()
            .padding(horizontal = 16.dp, vertical = 10.dp),
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        tonalElevation = 4.dp,
        shadowElevation = 6.dp,
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 18.dp, vertical = 10.dp),
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
                        modifier = Modifier.size(44.dp),
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
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 16.dp, vertical = 6.dp),
        shape = RoundedCornerShape(24.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHighest,
        tonalElevation = 4.dp,
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 12.dp, vertical = 8.dp),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                IconButton(onClick = onUndo, enabled = editor.undoAvailable && !editor.saving) {
                    Icon(Icons.AutoMirrored.Rounded.Undo, stringResource(R.string.action_undo))
                }
                IconButton(onClick = onRedo, enabled = editor.redoAvailable && !editor.saving) {
                    Icon(Icons.AutoMirrored.Rounded.Redo, stringResource(R.string.action_redo))
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
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
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

@OptIn(ExperimentalLayoutApi::class)
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
            Column(verticalArrangement = Arrangement.spacedBy(10.dp)) {
                Text(stringResource(R.string.history_export_format), fontWeight = FontWeight.SemiBold)
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
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
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
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

    Surface(
        shape = RoundedCornerShape(14.dp),
        color = if (message.isError) {
            MaterialTheme.colorScheme.errorContainer
        } else {
            MaterialTheme.colorScheme.secondaryContainer
        },
        contentColor = if (message.isError) {
            MaterialTheme.colorScheme.onErrorContainer
        } else {
            MaterialTheme.colorScheme.onSecondaryContainer
        },
        modifier = Modifier.fillMaxWidth(),
    ) {
        Text(
            text = stringResource(message.textRes),
            style = MaterialTheme.typography.bodyMedium,
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 8.dp),
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
private fun LibraryTranscriptError(
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(
                text = stringResource(R.string.library_transcript_load_failed),
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodyLarge,
                fontWeight = FontWeight.Medium,
            )
            FilledTonalButton(onClick = onRetry) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}
