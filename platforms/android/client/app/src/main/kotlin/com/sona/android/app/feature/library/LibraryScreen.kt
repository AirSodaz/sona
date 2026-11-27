package com.sona.android.app.feature.library

import androidx.compose.animation.AnimatedContent
import androidx.compose.animation.animateColorAsState
import androidx.compose.foundation.ExperimentalFoundationApi
import androidx.compose.foundation.background
import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.WindowInsets
import androidx.compose.foundation.layout.asPaddingValues
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBars
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.statusBarsPadding
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyRow
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.rounded.ArrowForward
import androidx.compose.material.icons.rounded.Add
import androidx.compose.material.icons.rounded.AudioFile
import androidx.compose.material.icons.rounded.Check
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.DeleteForever
import androidx.compose.material.icons.rounded.Edit
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Mic
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material.icons.rounded.Search
import androidx.compose.material.icons.rounded.Tune
import androidx.compose.material.icons.rounded.WarningAmber
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Badge
import androidx.compose.material3.BadgedBox
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.SwipeToDismissBox
import androidx.compose.material3.SwipeToDismissBoxValue
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.material3.pulltorefresh.PullToRefreshBox
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.material3.rememberSwipeToDismissBoxState
import androidx.compose.material3.ripple
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.input.nestedscroll.nestedScroll
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.recording.formatRecordingTimer
import com.sona.android.app.feature.settings.RecognitionSettingsUiState
import com.sona.android.app.ui.component.SonaCollapsibleTopAppBar
import com.sona.android.app.ui.component.springOverscroll
import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemKind
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.HistorySortOrder
import com.sona.android.application.library.ProjectRecord
import com.sona.android.application.recording.AudioImportFailure
import com.sona.android.application.recording.AudioImportJobState
import com.sona.android.application.recording.AudioImportStage
import com.sona.android.application.recording.AudioImportTask
import com.sona.android.application.recording.AudioImportTaskStatus
import com.sona.android.application.recording.OnlineAsrProvider
import java.text.DateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlinx.coroutines.flow.distinctUntilChanged

@OptIn(ExperimentalMaterial3Api::class, ExperimentalFoundationApi::class)
@Composable
internal fun LibraryScreen(
    state: LibraryUiState,
    recognitionSettings: RecognitionSettingsUiState,
    configuredProviders: Set<OnlineAsrProvider>,
    onRefresh: () -> Unit,
    onLoadMore: () -> Unit,
    onRetry: () -> Unit,
    onOpenItem: (String) -> Unit,
    onSearchChanged: (String) -> Unit,
    onScopeChanged: (HistoryScope) -> Unit,
    onFilterChanged: (HistoryFilterType) -> Unit,
    onDateChanged: (HistoryDateFilter) -> Unit,
    onSortChanged: (HistorySortOrder) -> Unit,
    onToggleSelection: (String) -> Unit,
    onClearSelection: () -> Unit,
    onTrashSelected: () -> Unit,
    onRestoreSelected: () -> Unit,
    onPurgeSelected: () -> Unit,
    onTrashItem: (String) -> Unit,
    onRestoreItem: (String) -> Unit,
    onMoveSelectedToProject: (String?) -> Unit,
    onCreateProject: (String) -> Unit,
    onRenameProject: (String, String) -> Unit,
    onDeleteProject: (String) -> Unit,
    onCancelAudioImportJob: (String) -> Unit,
    onDismissFailedImport: (String?) -> Unit = {},
    onConfigureRecognition: () -> Unit,
    recoveryPendingCount: Int,
    onOpenRecovery: () -> Unit,
    extraBottomPadding: Dp = 0.dp,
) {
    val listState = rememberLazyListState()
    var purgeConfirmationVisible by remember { mutableStateOf(false) }
    var projectMenuVisible by remember { mutableStateOf(false) }
    var filterSheetVisible by remember { mutableStateOf(false) }
    var createProjectDialogVisible by remember { mutableStateOf(false) }
    var newProjectName by remember { mutableStateOf("") }
    var managedProject by remember { mutableStateOf<ProjectRecord?>(null) }
    var renameProjectName by remember { mutableStateOf("") }
    var deleteProjectConfirmation by remember { mutableStateOf<ProjectRecord?>(null) }

    val activeFilterCount = listOf(
        state.query.filterType != HistoryFilterType.ALL,
        state.query.dateFilter != HistoryDateFilter.ALL,
        state.query.sortOrder != HistorySortOrder.NEWEST,
    ).count { it }

    if (purgeConfirmationVisible) {
        AlertDialog(
            onDismissRequest = { purgeConfirmationVisible = false },
            title = { Text(stringResource(R.string.history_delete_confirm_title)) },
            text = {
                Text(
                    pluralStringResource(
                        R.plurals.history_delete_confirm_body,
                        state.selectedIds.size,
                        state.selectedIds.size,
                    ),
                )
            },
            confirmButton = {
                TextButton(onClick = {
                    purgeConfirmationVisible = false
                    onPurgeSelected()
                }) { Text(stringResource(R.string.history_delete_forever)) }
            },
            dismissButton = {
                TextButton(onClick = { purgeConfirmationVisible = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    if (createProjectDialogVisible) {
        AlertDialog(
            onDismissRequest = { createProjectDialogVisible = false },
            title = { Text(stringResource(R.string.history_create_project)) },
            text = {
                OutlinedTextField(
                    value = newProjectName,
                    onValueChange = { newProjectName = it },
                    label = { Text(stringResource(R.string.history_project_name)) },
                    singleLine = true,
                    modifier = Modifier.fillMaxWidth(),
                )
            },
            confirmButton = {
                TextButton(
                    enabled = newProjectName.isNotBlank() && !state.operationInProgress,
                    onClick = {
                        onCreateProject(newProjectName.trim())
                        newProjectName = ""
                        createProjectDialogVisible = false
                    },
                ) { Text(stringResource(R.string.history_create_project)) }
            },
            dismissButton = {
                TextButton(onClick = { createProjectDialogVisible = false }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    managedProject?.let { project ->
        AlertDialog(
            onDismissRequest = { managedProject = null },
            title = { Text(project.name) },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    OutlinedTextField(
                        value = renameProjectName,
                        onValueChange = { renameProjectName = it },
                        label = { Text(stringResource(R.string.history_project_name)) },
                        singleLine = true,
                        modifier = Modifier.fillMaxWidth(),
                    )
                    TextButton(
                        onClick = {
                            managedProject = null
                            deleteProjectConfirmation = project
                        },
                    ) {
                        Icon(
                            Icons.Rounded.Delete,
                            contentDescription = null,
                            tint = MaterialTheme.colorScheme.error,
                            modifier = Modifier.size(18.dp),
                        )
                        Spacer(Modifier.width(6.dp))
                        Text(
                            text = stringResource(R.string.action_delete),
                            color = MaterialTheme.colorScheme.error,
                        )
                    }
                }
            },
            confirmButton = {
                TextButton(
                    enabled = renameProjectName.isNotBlank() && !state.operationInProgress,
                    onClick = {
                        onRenameProject(project.id, renameProjectName.trim())
                        managedProject = null
                    },
                ) { Text(stringResource(R.string.action_save)) }
            },
            dismissButton = {
                TextButton(onClick = { managedProject = null }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    deleteProjectConfirmation?.let { project ->
        AlertDialog(
            onDismissRequest = { deleteProjectConfirmation = null },
            title = { Text(stringResource(R.string.project_delete_confirm_title, project.name)) },
            text = { Text(stringResource(R.string.project_delete_confirm_body)) },
            confirmButton = {
                TextButton(
                    onClick = {
                        if (state.query.scope == HistoryScope.Project(project.id)) {
                            onScopeChanged(HistoryScope.All)
                        }
                        onDeleteProject(project.id)
                        deleteProjectConfirmation = null
                    },
                ) {
                    Text(
                        text = stringResource(R.string.action_delete),
                        color = MaterialTheme.colorScheme.error,
                    )
                }
            },
            dismissButton = {
                TextButton(onClick = { deleteProjectConfirmation = null }) {
                    Text(stringResource(R.string.action_cancel))
                }
            },
        )
    }

    if (filterSheetVisible) {
        LibraryFilterSheet(
            filterType = state.query.filterType,
            dateFilter = state.query.dateFilter,
            sortOrder = state.query.sortOrder,
            onFilterChanged = onFilterChanged,
            onDateChanged = onDateChanged,
            onSortChanged = onSortChanged,
            onReset = {
                onFilterChanged(HistoryFilterType.ALL)
                onDateChanged(HistoryDateFilter.ALL)
                onSortChanged(HistorySortOrder.NEWEST)
            },
            onDismiss = { filterSheetVisible = false },
        )
    }

    LaunchedEffect(listState, state.items.size, state.hasMore) {
        snapshotFlow { listState.layoutInfo.visibleItemsInfo.lastOrNull()?.index }
            .distinctUntilChanged()
            .collect { lastVisibleIndex ->
                if (
                    lastVisibleIndex != null &&
                    state.hasMore &&
                    lastVisibleIndex >= (state.items.lastIndex - 3).coerceAtLeast(0)
                ) {
                    onLoadMore()
                }
            }
    }

    val scrollBehavior = TopAppBarDefaults.exitUntilCollapsedScrollBehavior()
    val isSelecting = state.selectedIds.isNotEmpty()
    val hasActiveFilter = state.query.query.isNotBlank() || activeFilterCount > 0

    LaunchedEffect(state.items.isEmpty()) {
        if (state.items.isEmpty()) {
            scrollBehavior.state.heightOffset = 0f
        }
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
            // 1. Contextual Selection Bar OR Collapsible Top Bar + Search Pill Header
            AnimatedContent(
                targetState = isSelecting,
                label = "libraryHeaderMode",
            ) { selecting ->
                if (selecting) {
                    Surface(
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                        shape = RoundedCornerShape(24.dp),
                        modifier = Modifier
                            .fillMaxWidth()
                            .statusBarsPadding()
                            .padding(horizontal = 16.dp, vertical = 10.dp),
                    ) {
                        Row(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 12.dp, vertical = 6.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            IconButton(onClick = onClearSelection) {
                                Icon(Icons.Rounded.Close, stringResource(R.string.history_clear_selection))
                            }
                            Text(
                                text = pluralStringResource(
                                    R.plurals.history_selected_count,
                                    state.selectedIds.size,
                                    state.selectedIds.size,
                                ),
                                style = MaterialTheme.typography.titleMedium,
                                fontWeight = FontWeight.Bold,
                                modifier = Modifier.weight(1f),
                            )
                            if (state.query.scope == HistoryScope.Trash) {
                                IconButton(onClick = onRestoreSelected) {
                                    Icon(Icons.Rounded.Restore, stringResource(R.string.history_restore))
                                }
                                IconButton(onClick = { purgeConfirmationVisible = true }) {
                                    Icon(
                                        Icons.Rounded.DeleteForever,
                                        stringResource(R.string.history_delete_forever),
                                        tint = MaterialTheme.colorScheme.error,
                                    )
                                }
                            } else {
                                Box {
                                    IconButton(onClick = { projectMenuVisible = true }) {
                                        Icon(
                                            Icons.Rounded.FolderOpen,
                                            stringResource(R.string.history_manage_selected_project),
                                        )
                                    }
                                    DropdownMenu(
                                        expanded = projectMenuVisible,
                                        onDismissRequest = { projectMenuVisible = false },
                                    ) {
                                        DropdownMenuItem(
                                            text = { Text(stringResource(R.string.history_move_to_inbox)) },
                                            onClick = {
                                                projectMenuVisible = false
                                                onMoveSelectedToProject(null)
                                            },
                                        )
                                        state.projects.forEach { project ->
                                            DropdownMenuItem(
                                                text = {
                                                    Text(stringResource(R.string.history_move_to_project, project.name))
                                                },
                                                onClick = {
                                                    projectMenuVisible = false
                                                    onMoveSelectedToProject(project.id)
                                                },
                                            )
                                        }
                                    }
                                }
                                IconButton(onClick = onTrashSelected) {
                                    Icon(Icons.Rounded.Delete, stringResource(R.string.history_move_to_trash))
                                }
                            }
                        }
                    }
                } else {
                    Column(
                        modifier = Modifier.fillMaxWidth(),
                    ) {
                        SonaCollapsibleTopAppBar(
                            title = stringResource(R.string.destination_library),
                            scrollBehavior = scrollBehavior,
                        )
                        Box(
                            modifier = Modifier
                                .fillMaxWidth()
                                .padding(horizontal = 20.dp)
                                .padding(top = 8.dp, bottom = 8.dp),
                        ) {
                            LibrarySearchPill(
                                query = state.query.query,
                                onQueryChange = onSearchChanged,
                                activeFilterCount = activeFilterCount,
                                onOpenFilters = { filterSheetVisible = true },
                            )
                        }
                    }
                }
            }

            // 2. Single Clean Scope & Project Strip
            LazyRow(
                modifier = Modifier.fillMaxWidth(),
                contentPadding = PaddingValues(horizontal = 20.dp, vertical = 4.dp),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                item(key = "scope-all") {
                    val selected = state.query.scope == HistoryScope.All
                    ScopePillChip(
                        label = stringResource(R.string.history_scope_all),
                        count = if (selected && hasActiveFilter) state.workspaceCount else state.allCount,
                        selected = selected,
                        onClick = { onScopeChanged(HistoryScope.All) },
                    )
                }
                item(key = "scope-inbox") {
                    val selected = state.query.scope == HistoryScope.Inbox
                    ScopePillChip(
                        label = stringResource(R.string.history_scope_inbox),
                        count = if (selected && hasActiveFilter) state.workspaceCount else state.inboxCount,
                        selected = selected,
                        onClick = { onScopeChanged(HistoryScope.Inbox) },
                    )
                }
                items(state.projects, key = ProjectRecord::id) { project ->
                    val selected = state.query.scope == HistoryScope.Project(project.id)
                    ScopePillChip(
                        label = project.name,
                        count = if (selected && hasActiveFilter) {
                            state.workspaceCount
                        } else {
                            state.projectCounts[project.id]
                        },
                        selected = selected,
                        onClick = { onScopeChanged(HistoryScope.Project(project.id)) },
                        onLongClick = {
                            renameProjectName = project.name
                            managedProject = project
                        },
                    )
                }
                item(key = "scope-trash") {
                    val selected = state.query.scope == HistoryScope.Trash
                    ScopePillChip(
                        label = stringResource(R.string.history_scope_trash),
                        count = if (selected && hasActiveFilter) state.workspaceCount else state.trashCount,
                        selected = selected,
                        onClick = { onScopeChanged(HistoryScope.Trash) },
                    )
                }
                item(key = "scope-add-project") {
                    Surface(
                        onClick = { createProjectDialogVisible = true },
                        shape = RoundedCornerShape(18.dp),
                        color = MaterialTheme.colorScheme.surfaceContainerLow,
                        contentColor = MaterialTheme.colorScheme.primary,
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 12.dp, vertical = 7.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(4.dp),
                        ) {
                            Icon(Icons.Rounded.Add, contentDescription = null, modifier = Modifier.size(16.dp))
                            Text(
                                text = stringResource(R.string.history_create_project),
                                style = MaterialTheme.typography.labelLarge,
                                fontWeight = FontWeight.Medium,
                            )
                        }
                    }
                }
            }

            // 3. Inline Notices (Recovery + Audio Import Queue)
            Column(
                modifier = Modifier
                    .fillMaxWidth()
                    .padding(horizontal = 20.dp),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                if (recoveryPendingCount > 0) {
                    Surface(
                        onClick = onOpenRecovery,
                        shape = RoundedCornerShape(18.dp),
                        color = MaterialTheme.colorScheme.tertiaryContainer,
                        contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 6.dp),
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(Icons.Rounded.WarningAmber, contentDescription = null, modifier = Modifier.size(20.dp))
                            Text(
                                text = pluralStringResource(
                                    R.plurals.recovery_pending_notice,
                                    recoveryPendingCount,
                                    recoveryPendingCount,
                                ),
                                style = MaterialTheme.typography.bodyMedium,
                                fontWeight = FontWeight.Medium,
                                modifier = Modifier.weight(1f),
                            )
                            Icon(Icons.AutoMirrored.Rounded.ArrowForward, contentDescription = null, modifier = Modifier.size(18.dp))
                        }
                    }
                }

                val visibleTasks = state.audioImportQueue.tasks.filter {
                    it.status == AudioImportTaskStatus.QUEUED ||
                        it.status == AudioImportTaskStatus.RUNNING ||
                        it.status == AudioImportTaskStatus.FAILED
                }
                visibleTasks.forEach { task ->
                    AudioImportTaskCard(
                        task = task,
                        onCancel = { onCancelAudioImportJob(task.jobId) },
                        onDismiss = { onDismissFailedImport(task.jobId) },
                        onConfigure = onConfigureRecognition,
                    )
                }

                val legacyFailed = state.audioImport as? AudioImportJobState.Failed
                if (legacyFailed != null && visibleTasks.none { it.status == AudioImportTaskStatus.FAILED }) {
                    Surface(
                        shape = RoundedCornerShape(18.dp),
                        color = MaterialTheme.colorScheme.errorContainer,
                        contentColor = MaterialTheme.colorScheme.onErrorContainer,
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(top = 6.dp),
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 14.dp, vertical = 10.dp),
                            verticalAlignment = Alignment.CenterVertically,
                            horizontalArrangement = Arrangement.spacedBy(10.dp),
                        ) {
                            Icon(Icons.Rounded.WarningAmber, contentDescription = null, modifier = Modifier.size(20.dp))
                            Text(
                                text = stringResource(legacyFailed.reason.messageRes()),
                                style = MaterialTheme.typography.bodySmall,
                                modifier = Modifier.weight(1f),
                            )
                            if (legacyFailed.reason == AudioImportFailure.CONFIGURATION) {
                                TextButton(onClick = onConfigureRecognition) {
                                    Text(stringResource(R.string.action_configure))
                                }
                            }
                            IconButton(
                                onClick = { onDismissFailedImport(legacyFailed.jobId) },
                                modifier = Modifier.size(32.dp),
                            ) {
                                Icon(
                                    imageVector = Icons.Rounded.Close,
                                    contentDescription = stringResource(R.string.action_close),
                                    modifier = Modifier.size(18.dp),
                                )
                            }
                        }
                    }
                }
            }

            // 4. Main List with Native Pull-to-Refresh
            PullToRefreshBox(
                isRefreshing = state.isRefreshing,
                onRefresh = onRefresh,
                modifier = Modifier
                    .fillMaxWidth()
                    .weight(1f),
            ) {
                Box(
                    modifier = Modifier
                        .fillMaxSize()
                        .then(
                            if (isSelecting) {
                                Modifier
                            } else {
                                Modifier.nestedScroll(scrollBehavior.nestedScrollConnection)
                            },
                        ),
                ) {
                    when {
                        state.isInitialLoading -> LibraryLoading(modifier = Modifier.fillMaxSize())
                        state.items.isEmpty() && state.listError != null -> LibraryEmptyError(
                            onRetry = onRetry,
                            modifier = Modifier.fillMaxSize(),
                        )
                        state.items.isEmpty() -> LibraryEmpty(
                            modifier = Modifier.fillMaxSize(),
                        )
                        else -> {
                            Column(modifier = Modifier.fillMaxSize()) {
                                if (state.listError != null) {
                                    LibraryInlineError(
                                        onRetry = onRetry,
                                        modifier = Modifier.padding(horizontal = 20.dp),
                                    )
                                }
                                val navBarBottom = WindowInsets.navigationBars.asPaddingValues().calculateBottomPadding()
                                LazyColumn(
                                    state = listState,
                                    modifier = Modifier
                                        .fillMaxWidth()
                                        .weight(1f)
                                        .springOverscroll(
                                            scrollBehavior = scrollBehavior,
                                            enableTopOverscroll = false,
                                        ),
                                    verticalArrangement = Arrangement.spacedBy(10.dp),
                                    contentPadding = PaddingValues(
                                        start = 20.dp,
                                        end = 20.dp,
                                        top = 10.dp,
                                        bottom = maxOf(navBarBottom + 16.dp, extraBottomPadding + 12.dp),
                                    ),
                                ) {
                                    items(state.items, key = HistoryItem::historyId) { item ->
                                        val projectName = item.projectId?.let { pid ->
                                            state.projects.firstOrNull { it.id == pid }?.name
                                        }
                                        SwipeableLibraryItemRow(
                                            item = item,
                                            projectName = projectName,
                                            inTrash = state.query.scope == HistoryScope.Trash,
                                            selected = item.historyId in state.selectedIds,
                                            selectionActive = state.selectedIds.isNotEmpty(),
                                            operationInProgress = state.operationInProgress,
                                            onClick = {
                                                if (state.selectedIds.isEmpty()) {
                                                    onOpenItem(item.historyId)
                                                } else {
                                                    onToggleSelection(item.historyId)
                                                }
                                            },
                                            onLongClick = { onToggleSelection(item.historyId) },
                                            onSwipeAction = {
                                                if (state.query.scope == HistoryScope.Trash) {
                                                    onRestoreItem(item.historyId)
                                                } else {
                                                    onTrashItem(item.historyId)
                                                }
                                            },
                                        )
                                    }
                                    if (state.isLoadingMore) {
                                        item(key = "library-loading-more") {
                                            Box(
                                                modifier = Modifier
                                                    .fillMaxWidth()
                                                    .padding(20.dp),
                                                contentAlignment = Alignment.Center,
                                            ) {
                                                CircularProgressIndicator(modifier = Modifier.size(24.dp))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun LibrarySearchPill(
    query: String,
    onQueryChange: (String) -> Unit,
    activeFilterCount: Int,
    onOpenFilters: () -> Unit,
) {
    Surface(
        shape = RoundedCornerShape(28.dp),
        color = MaterialTheme.colorScheme.surfaceContainerHigh,
        modifier = Modifier
            .fillMaxWidth()
            .height(52.dp),
    ) {
        Row(
            modifier = Modifier
                .fillMaxSize()
                .padding(start = 16.dp, end = 6.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(10.dp),
        ) {
            Icon(
                imageVector = Icons.Rounded.Search,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant,
                modifier = Modifier.size(20.dp),
            )
            Box(
                modifier = Modifier.weight(1f),
                contentAlignment = Alignment.CenterStart,
            ) {
                if (query.isEmpty()) {
                    Text(
                        text = stringResource(R.string.history_search),
                        style = MaterialTheme.typography.bodyLarge,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
                BasicTextField(
                    value = query,
                    onValueChange = onQueryChange,
                    singleLine = true,
                    textStyle = MaterialTheme.typography.bodyLarge.copy(
                        color = MaterialTheme.colorScheme.onSurface,
                    ),
                    cursorBrush = SolidColor(MaterialTheme.colorScheme.primary),
                    modifier = Modifier.fillMaxWidth(),
                )
            }
            if (query.isNotEmpty()) {
                IconButton(onClick = { onQueryChange("") }) {
                    Icon(
                        imageVector = Icons.Rounded.Close,
                        contentDescription = stringResource(R.string.action_clear_search),
                        modifier = Modifier.size(20.dp),
                    )
                }
            }
            IconButton(onClick = onOpenFilters) {
                BadgedBox(
                    badge = {
                        if (activeFilterCount > 0) {
                            Badge(
                                containerColor = MaterialTheme.colorScheme.primary,
                                contentColor = MaterialTheme.colorScheme.onPrimary,
                            ) {
                                Text("$activeFilterCount")
                            }
                        }
                    },
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Tune,
                        contentDescription = stringResource(R.string.library_filter_and_sort),
                        tint = if (activeFilterCount > 0) {
                            MaterialTheme.colorScheme.primary
                        } else {
                            MaterialTheme.colorScheme.onSurfaceVariant
                        },
                    )
                }
            }
        }
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
private fun ScopePillChip(
    label: String,
    count: Long? = null,
    selected: Boolean,
    onClick: () -> Unit,
    onLongClick: (() -> Unit)? = null,
) {
    val containerColor by animateColorAsState(
        targetValue = if (selected) {
            MaterialTheme.colorScheme.primaryContainer
        } else {
            MaterialTheme.colorScheme.surfaceContainerHigh
        },
        label = "scopeChipBg",
    )
    val contentColor by animateColorAsState(
        targetValue = if (selected) {
            MaterialTheme.colorScheme.onPrimaryContainer
        } else {
            MaterialTheme.colorScheme.onSurfaceVariant
        },
        label = "scopeChipFg",
    )

    Surface(
        shape = RoundedCornerShape(18.dp),
        color = containerColor,
        contentColor = contentColor,
        modifier = Modifier
            .clip(RoundedCornerShape(18.dp))
            .combinedClickable(
                role = Role.Tab,
                onClick = onClick,
                onLongClick = onLongClick,
            ),
    ) {
        val showCount = count != null && count > 0
        Row(
            modifier = Modifier.padding(
                start = 14.dp,
                end = if (showCount) 10.dp else 14.dp,
                top = 7.dp,
                bottom = 7.dp,
            ),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(6.dp),
        ) {
            Text(
                text = label,
                style = MaterialTheme.typography.labelLarge,
                fontWeight = if (selected) FontWeight.Bold else FontWeight.Medium,
            )
            if (showCount) {
                Surface(
                    shape = RoundedCornerShape(10.dp),
                    color = if (selected) {
                        MaterialTheme.colorScheme.primary.copy(alpha = 0.14f)
                    } else {
                        MaterialTheme.colorScheme.surfaceContainerHighest
                    },
                    contentColor = contentColor,
                ) {
                    Text(
                        text = "$count",
                        style = MaterialTheme.typography.labelSmall,
                        fontWeight = FontWeight.SemiBold,
                        modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
                    )
                }
            }
        }
    }
}

@Composable
private fun AudioImportTaskCard(
    task: AudioImportTask,
    onCancel: () -> Unit,
    onDismiss: () -> Unit,
    onConfigure: () -> Unit,
) {
    val isFailed = task.status == AudioImportTaskStatus.FAILED
    Surface(
        shape = RoundedCornerShape(18.dp),
        color = if (isFailed) {
            MaterialTheme.colorScheme.errorContainer
        } else {
            MaterialTheme.colorScheme.secondaryContainer
        },
        contentColor = if (isFailed) {
            MaterialTheme.colorScheme.onErrorContainer
        } else {
            MaterialTheme.colorScheme.onSecondaryContainer
        },
        modifier = Modifier
            .fillMaxWidth()
            .padding(top = 6.dp),
    ) {
        Column(
            modifier = Modifier.padding(horizontal = 14.dp, vertical = 12.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Icon(
                    imageVector = if (isFailed) Icons.Rounded.WarningAmber else Icons.Rounded.AudioFile,
                    contentDescription = null,
                    modifier = Modifier.size(20.dp),
                )
                Spacer(Modifier.width(10.dp))
                Column(modifier = Modifier.weight(1f)) {
                    Text(
                        text = task.displayName ?: stringResource(R.string.home_status_processing),
                        style = MaterialTheme.typography.titleSmall,
                        fontWeight = FontWeight.SemiBold,
                        maxLines = 1,
                        overflow = TextOverflow.Ellipsis,
                    )
                    Text(
                        text = if (isFailed) {
                            stringResource(task.failure?.messageRes() ?: R.string.audio_import_notification_failed)
                        } else {
                            stringResource(task.stage.labelRes())
                        },
                        style = MaterialTheme.typography.bodySmall,
                    )
                }
                if (isFailed) {
                    if (task.failure == AudioImportFailure.CONFIGURATION) {
                        TextButton(onClick = onConfigure) {
                            Text(stringResource(R.string.action_configure))
                        }
                    }
                    IconButton(onClick = onDismiss, modifier = Modifier.size(32.dp)) {
                        Icon(
                            imageVector = Icons.Rounded.Close,
                            contentDescription = stringResource(R.string.action_close),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                } else {
                    IconButton(onClick = onCancel, modifier = Modifier.size(32.dp)) {
                        Icon(
                            imageVector = Icons.Rounded.Close,
                            contentDescription = stringResource(R.string.library_import_cancel),
                            modifier = Modifier.size(18.dp),
                        )
                    }
                }
            }
            if (!isFailed) {
                val progress = task.progressPercent
                if (progress != null) {
                    LinearProgressIndicator(
                        progress = { (progress / 100f).coerceIn(0f, 1f) },
                        modifier = Modifier.fillMaxWidth(),
                    )
                } else {
                    LinearProgressIndicator(modifier = Modifier.fillMaxWidth())
                }
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SwipeableLibraryItemRow(
    item: HistoryItem,
    projectName: String?,
    inTrash: Boolean,
    selected: Boolean,
    selectionActive: Boolean,
    operationInProgress: Boolean,
    onClick: () -> Unit,
    onLongClick: () -> Unit,
    onSwipeAction: () -> Unit,
) {
    val currentOnSwipeAction by rememberUpdatedState(onSwipeAction)
    val currentOperationInProgress by rememberUpdatedState(operationInProgress)
    val dismissState = rememberSwipeToDismissBoxState()

    LaunchedEffect(dismissState.currentValue) {
        if (dismissState.currentValue == SwipeToDismissBoxValue.EndToStart) {
            if (!currentOperationInProgress) {
                currentOnSwipeAction()
            }
            dismissState.snapTo(SwipeToDismissBoxValue.Settled)
        }
    }

    SwipeToDismissBox(
        state = dismissState,
        enableDismissFromStartToEnd = false,
        enableDismissFromEndToStart = !selectionActive,
        backgroundContent = {
            val bgColor by animateColorAsState(
                targetValue = if (inTrash) {
                    MaterialTheme.colorScheme.primaryContainer
                } else {
                    MaterialTheme.colorScheme.errorContainer
                },
                label = "swipeBgColor",
            )
            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .background(bgColor, RoundedCornerShape(22.dp))
                    .padding(horizontal = 24.dp),
                contentAlignment = Alignment.CenterEnd,
            ) {
                Icon(
                    imageVector = if (inTrash) Icons.Rounded.Restore else Icons.Rounded.Delete,
                    contentDescription = stringResource(
                        if (inTrash) R.string.history_restore else R.string.history_move_to_trash,
                    ),
                    tint = if (inTrash) {
                        MaterialTheme.colorScheme.onPrimaryContainer
                    } else {
                        MaterialTheme.colorScheme.onErrorContainer
                    },
                )
            }
        },
    ) {
        LibraryItemRow(
            item = item,
            projectName = projectName,
            selected = selected,
            onClick = onClick,
            onLongClick = onLongClick,
        )
    }
}

@OptIn(ExperimentalFoundationApi::class)
@Composable
internal fun LibraryItemRow(
    item: HistoryItem,
    projectName: String? = null,
    selected: Boolean = false,
    onClick: () -> Unit,
    onLongClick: () -> Unit = {},
) {
    val fallbackTitle = stringResource(R.string.library_detail_heading)
    val containerColor = when {
        selected -> MaterialTheme.colorScheme.secondaryContainer
        item.status == HistoryItemStatus.DRAFT -> MaterialTheme.colorScheme.surfaceContainerLow
        else -> MaterialTheme.colorScheme.surfaceContainer
    }

    val status = if (item.status == HistoryItemStatus.DRAFT) {
        stringResource(R.string.library_status_draft)
    } else {
        stringResource(R.string.library_status_complete)
    }

    val accessibilityDescription = stringResource(
        R.string.library_item_metadata,
        formatLibraryTimestamp(item.timestampEpochMillis),
        formatRecordingTimer(item.durationMillis),
        status,
    )

    val interactionSource = remember { MutableInteractionSource() }
    Surface(
        shape = RoundedCornerShape(22.dp),
        color = containerColor,
        modifier = Modifier
            .fillMaxWidth()
            .clip(RoundedCornerShape(22.dp))
            .combinedClickable(
                interactionSource = interactionSource,
                indication = ripple(bounded = true),
                onClick = onClick,
                onLongClick = onLongClick,
            )
            .semantics {
                contentDescription = accessibilityDescription
            },
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 14.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Surface(
                shape = CircleShape,
                color = when {
                    selected -> MaterialTheme.colorScheme.primary
                    item.status == HistoryItemStatus.DRAFT -> MaterialTheme.colorScheme.tertiaryContainer
                    else -> MaterialTheme.colorScheme.primaryContainer
                },
                contentColor = when {
                    selected -> MaterialTheme.colorScheme.onPrimary
                    item.status == HistoryItemStatus.DRAFT -> MaterialTheme.colorScheme.onTertiaryContainer
                    else -> MaterialTheme.colorScheme.onPrimaryContainer
                },
                modifier = Modifier.size(42.dp),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        imageVector = when {
                            selected -> Icons.Rounded.Check
                            item.kind == HistoryItemKind.BATCH -> Icons.Rounded.AudioFile
                            else -> Icons.Rounded.Mic
                        },
                        contentDescription = null,
                        modifier = Modifier.size(20.dp),
                    )
                }
            }

            Spacer(Modifier.width(14.dp))

            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = item.title.ifBlank { fallbackTitle },
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(4.dp))
                LibraryItemMetadata(item = item, projectName = projectName)
                val preview = item.searchMatch?.snippet ?: item.previewText
                if (preview.isNotBlank()) {
                    Spacer(Modifier.height(6.dp))
                    Text(
                        text = highlightedPreview(item, preview),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }

            Spacer(Modifier.width(8.dp))
            Icon(
                imageVector = Icons.Rounded.ChevronRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.55f),
            )
        }
    }
}

@Composable
private fun highlightedPreview(item: HistoryItem, preview: String) = buildAnnotatedString {
    append(preview)
    val match = item.searchMatch ?: return@buildAnnotatedString
    val start = match.highlightStart.coerceIn(0, preview.length)
    val end = match.highlightEnd.coerceIn(start, preview.length)
    if (start < end) {
        addStyle(
            SpanStyle(
                color = MaterialTheme.colorScheme.primary,
                fontWeight = FontWeight.Bold,
            ),
            start,
            end,
        )
    }
}

@Composable
internal fun LibraryItemMetadata(
    item: HistoryItem,
    projectName: String? = null,
) {
    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(6.dp),
    ) {
        Text(
            text = formatLibraryTimestamp(item.timestampEpochMillis),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            maxLines = 1,
            overflow = TextOverflow.Ellipsis,
        )
        Text(
            text = "·",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
        )
        Text(
            text = formatRecordingTimer(item.durationMillis),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.primary,
            fontWeight = FontWeight.SemiBold,
        )
        if (item.status == HistoryItemStatus.DRAFT) {
            Surface(
                shape = RoundedCornerShape(6.dp),
                color = MaterialTheme.colorScheme.tertiaryContainer,
                contentColor = MaterialTheme.colorScheme.onTertiaryContainer,
            ) {
                Text(
                    text = stringResource(R.string.library_status_draft),
                    style = MaterialTheme.typography.labelSmall,
                    fontWeight = FontWeight.Bold,
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
                )
            }
        } else if (!projectName.isNullOrBlank()) {
            Surface(
                shape = RoundedCornerShape(6.dp),
                color = MaterialTheme.colorScheme.surfaceContainerHighest,
                contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
            ) {
                Text(
                    text = projectName,
                    style = MaterialTheme.typography.labelSmall,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                    modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp),
                )
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
private fun LibraryFilterSheet(
    filterType: HistoryFilterType,
    dateFilter: HistoryDateFilter,
    sortOrder: HistorySortOrder,
    onFilterChanged: (HistoryFilterType) -> Unit,
    onDateChanged: (HistoryDateFilter) -> Unit,
    onSortChanged: (HistorySortOrder) -> Unit,
    onReset: () -> Unit,
    onDismiss: () -> Unit,
) {
    ModalBottomSheet(
        onDismissRequest = onDismiss,
        sheetState = rememberModalBottomSheetState(skipPartiallyExpanded = true),
    ) {
        Column(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 24.dp)
                .padding(bottom = 32.dp)
                .verticalScroll(rememberScrollState()),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
                horizontalArrangement = Arrangement.SpaceBetween,
            ) {
                Text(
                    text = stringResource(R.string.library_filter_and_sort),
                    style = MaterialTheme.typography.titleLarge,
                    fontWeight = FontWeight.Bold,
                )
                TextButton(onClick = onReset) {
                    Text(stringResource(R.string.action_reset))
                }
            }

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = stringResource(R.string.library_filter_source),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                )
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    HistoryFilterType.entries.forEach { value ->
                        FilterChip(
                            selected = filterType == value,
                            onClick = { onFilterChanged(value) },
                            label = { Text(stringResource(value.labelRes())) },
                        )
                    }
                }
            }

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = stringResource(R.string.library_filter_date),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                )
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    HistoryDateFilter.entries.forEach { value ->
                        FilterChip(
                            selected = dateFilter == value,
                            onClick = { onDateChanged(value) },
                            label = { Text(stringResource(value.labelRes())) },
                        )
                    }
                }
            }

            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(
                    text = stringResource(R.string.library_filter_sort),
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.primary,
                )
                FlowRow(
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalArrangement = Arrangement.spacedBy(6.dp),
                ) {
                    HistorySortOrder.entries.forEach { value ->
                        FilterChip(
                            selected = sortOrder == value,
                            onClick = { onSortChanged(value) },
                            label = { Text(stringResource(value.labelRes())) },
                        )
                    }
                }
            }
        }
    }
}

@Composable
internal fun LibraryLoading(modifier: Modifier = Modifier) {
    Box(modifier = modifier.fillMaxWidth(), contentAlignment = Alignment.Center) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            CircularProgressIndicator()
            Spacer(Modifier.height(12.dp))
            Text(
                text = stringResource(R.string.library_loading),
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
    }
}

@Composable
private fun LibraryEmpty(
    modifier: Modifier = Modifier,
) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(horizontal = 24.dp, vertical = 32.dp),
        contentAlignment = Alignment.Center,
    ) {
        Column(
            modifier = Modifier.padding(24.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Surface(
                shape = CircleShape,
                color = MaterialTheme.colorScheme.primaryContainer.copy(alpha = 0.6f),
                modifier = Modifier.size(72.dp),
            ) {
                Box(contentAlignment = Alignment.Center) {
                    Icon(
                        imageVector = Icons.Rounded.FolderOpen,
                        contentDescription = null,
                        tint = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.size(34.dp),
                    )
                }
            }
            Text(
                text = stringResource(R.string.library_empty),
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.SemiBold,
                color = MaterialTheme.colorScheme.onSurface,
            )
        }
    }
}

@Composable
private fun LibraryEmptyError(
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
                text = stringResource(R.string.library_load_failed),
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

@Composable
private fun LibraryInlineError(
    onRetry: () -> Unit,
    modifier: Modifier = Modifier,
) {
    Surface(
        shape = RoundedCornerShape(16.dp),
        color = MaterialTheme.colorScheme.errorContainer,
        contentColor = MaterialTheme.colorScheme.onErrorContainer,
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.library_load_failed),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.weight(1f),
            )
            TextButton(onClick = onRetry) {
                Text(stringResource(R.string.action_retry))
            }
        }
    }
}

private fun AudioImportStage.labelRes(): Int = when (this) {
    AudioImportStage.QUEUED -> R.string.library_import_queued
    AudioImportStage.STAGING -> R.string.library_import_staging
    AudioImportStage.TRANSCODING -> R.string.library_import_transcoding
    AudioImportStage.TRANSCRIBING -> R.string.library_import_transcribing
    AudioImportStage.SAVING -> R.string.library_import_saving
}

internal fun formatLibraryTimestamp(
    timestampEpochMillis: Long,
    locale: Locale = Locale.getDefault(),
    timeZone: TimeZone = TimeZone.getDefault(),
): String = DateFormat.getDateTimeInstance(
    DateFormat.MEDIUM,
    DateFormat.SHORT,
    locale,
).apply {
    this.timeZone = timeZone
}.format(Date(timestampEpochMillis.coerceAtLeast(0)))
