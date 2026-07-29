package com.sona.android.app.feature.library

import androidx.compose.foundation.combinedClickable
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.background
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
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.rounded.CheckCircle
import androidx.compose.material.icons.rounded.ChevronRight
import androidx.compose.material.icons.rounded.FolderOpen
import androidx.compose.material.icons.rounded.Refresh
import androidx.compose.material.icons.rounded.Schedule
import androidx.compose.material.icons.rounded.Delete
import androidx.compose.material.icons.rounded.DeleteForever
import androidx.compose.material.icons.rounded.Close
import androidx.compose.material.icons.rounded.Label
import androidx.compose.material.icons.rounded.Restore
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.FilterChip
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.snapshotFlow
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.unit.dp
import com.sona.android.app.R
import com.sona.android.app.feature.recording.formatRecordingTimer
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.HistorySortOrder
import java.text.DateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone
import kotlinx.coroutines.flow.distinctUntilChanged

@Composable
internal fun LibraryScreen(
    state: LibraryUiState,
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
    onAddTagToSelected: (String) -> Unit,
    onRemoveTagFromSelected: (String) -> Unit,
    recoveryPendingCount: Int,
    onOpenRecovery: () -> Unit,
) {
    val listState = rememberLazyListState()
    var purgeConfirmationVisible by remember { mutableStateOf(false) }
    var tagMenuVisible by remember { mutableStateOf(false) }

    if (purgeConfirmationVisible) {
        AlertDialog(
            onDismissRequest = { purgeConfirmationVisible = false },
            title = { Text(stringResource(R.string.history_delete_confirm_title)) },
            text = {
                Text(pluralStringResource(R.plurals.history_delete_confirm_body, state.selectedIds.size, state.selectedIds.size))
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
            Row(
                modifier = Modifier.fillMaxWidth(),
                verticalAlignment = Alignment.CenterVertically,
            ) {
                Text(
                    text = stringResource(R.string.library_heading),
                    style = MaterialTheme.typography.headlineMedium,
                    color = MaterialTheme.colorScheme.primary,
                    fontWeight = FontWeight.SemiBold,
                    modifier = Modifier.weight(1f),
                )
                if (state.isRefreshing) {
                    CircularProgressIndicator(modifier = Modifier.size(20.dp), strokeWidth = 2.dp)
                    Spacer(Modifier.width(12.dp))
                }
                IconButton(
                    onClick = onRefresh,
                    enabled = !state.isInitialLoading && !state.isRefreshing,
                ) {
                    Icon(
                        imageVector = Icons.Rounded.Refresh,
                        contentDescription = stringResource(R.string.library_refresh_description),
                    )
                }
            }
            Spacer(Modifier.height(16.dp))
            if (recoveryPendingCount > 0) {
                FilledTonalButton(
                    onClick = onOpenRecovery,
                    modifier = Modifier.fillMaxWidth(),
                ) {
                    Text(pluralStringResource(R.plurals.recovery_pending_notice, recoveryPendingCount, recoveryPendingCount))
                }
                Spacer(Modifier.height(8.dp))
            }
            OutlinedTextField(
                value = state.query.query,
                onValueChange = onSearchChanged,
                label = { Text(stringResource(R.string.history_search)) },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            Row(
                modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                ScopeChip(stringResource(R.string.history_scope_all), state.query.scope == HistoryScope.All) { onScopeChanged(HistoryScope.All) }
                ScopeChip(stringResource(R.string.history_scope_untagged), state.query.scope == HistoryScope.Untagged) { onScopeChanged(HistoryScope.Untagged) }
                state.tags.forEach { tag ->
                    ScopeChip(tag.name, state.query.scope == HistoryScope.Tag(tag.id)) { onScopeChanged(HistoryScope.Tag(tag.id)) }
                }
                ScopeChip(stringResource(R.string.history_scope_trash), state.query.scope == HistoryScope.Trash) { onScopeChanged(HistoryScope.Trash) }
            }
            Row(
                modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
                horizontalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                HistoryFilterType.entries.forEach { value ->
                    FilterChip(selected = state.query.filterType == value, onClick = { onFilterChanged(value) }, label = { Text(value.name.lowercase()) })
                }
                HistoryDateFilter.entries.forEach { value ->
                    FilterChip(selected = state.query.dateFilter == value, onClick = { onDateChanged(value) }, label = { Text(value.name.lowercase()) })
                }
                HistorySortOrder.entries.forEach { value ->
                    FilterChip(selected = state.query.sortOrder == value, onClick = { onSortChanged(value) }, label = { Text(value.name.lowercase().replace('_', ' ')) })
                }
            }
            if (state.selectedIds.isNotEmpty()) {
                Row(
                    modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                    horizontalArrangement = Arrangement.spacedBy(8.dp),
                    verticalAlignment = Alignment.CenterVertically,
                ) {
                    Text(
                        pluralStringResource(R.plurals.history_selected_count, state.selectedIds.size, state.selectedIds.size),
                        modifier = Modifier.weight(1f),
                    )
                    if (state.query.scope == HistoryScope.Trash) {
                        IconButton(onClick = onRestoreSelected) { Icon(Icons.Rounded.Restore, stringResource(R.string.history_restore)) }
                        IconButton(onClick = { purgeConfirmationVisible = true }) { Icon(Icons.Rounded.DeleteForever, stringResource(R.string.history_delete_forever)) }
                    } else {
                        Box {
                            IconButton(onClick = { tagMenuVisible = true }) {
                                Icon(Icons.Rounded.Label, stringResource(R.string.history_manage_selected_tags))
                            }
                            DropdownMenu(expanded = tagMenuVisible, onDismissRequest = { tagMenuVisible = false }) {
                                state.tags.forEach { tag ->
                                    DropdownMenuItem(
                                        text = { Text(stringResource(R.string.history_add_tag, tag.name)) },
                                        onClick = {
                                            tagMenuVisible = false
                                            onAddTagToSelected(tag.id)
                                        },
                                    )
                                    DropdownMenuItem(
                                        text = { Text(stringResource(R.string.history_remove_tag, tag.name)) },
                                        onClick = {
                                            tagMenuVisible = false
                                            onRemoveTagFromSelected(tag.id)
                                        },
                                    )
                                }
                            }
                        }
                        IconButton(onClick = onTrashSelected) { Icon(Icons.Rounded.Delete, stringResource(R.string.history_move_to_trash)) }
                    }
                    IconButton(onClick = onClearSelection) {
                        Icon(Icons.Rounded.Close, stringResource(R.string.history_clear_selection))
                    }
                }
            }

            when {
                state.isInitialLoading -> LibraryLoading(modifier = Modifier.weight(1f))
                state.items.isEmpty() && state.listError != null -> LibraryEmptyError(
                    onRetry = onRetry,
                    modifier = Modifier.weight(1f),
                )
                state.items.isEmpty() -> LibraryEmpty(modifier = Modifier.weight(1f))
                else -> {
                    if (state.listError != null) {
                        LibraryInlineError(onRetry)
                    }
                    LazyColumn(
                        state = listState,
                        modifier = Modifier
                            .fillMaxWidth()
                            .weight(1f),
                        verticalArrangement = Arrangement.spacedBy(10.dp),
                        contentPadding = PaddingValues(vertical = 8.dp)
                    ) {
                        items(state.items, key = HistoryItem::historyId) { item ->
                            LibraryItemRow(
                                item = item,
                                selected = item.historyId in state.selectedIds,
                                onClick = {
                                    if (state.selectedIds.isEmpty()) onOpenItem(item.historyId)
                                    else onToggleSelection(item.historyId)
                                },
                                onLongClick = { onToggleSelection(item.historyId) },
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

@Composable
internal fun LibraryItemRow(
    item: HistoryItem,
    selected: Boolean = false,
    onClick: () -> Unit,
    onLongClick: () -> Unit = {},
) {
    val fallbackTitle = stringResource(R.string.library_detail_heading)
    val containerColor = if (selected) {
        MaterialTheme.colorScheme.secondaryContainer
    } else if (item.status == HistoryItemStatus.DRAFT) {
        MaterialTheme.colorScheme.surfaceContainerLow
    } else {
        MaterialTheme.colorScheme.surfaceContainer
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
        status
    )

    Card(
        shape = MaterialTheme.shapes.medium,
        colors = CardDefaults.cardColors(containerColor = containerColor),
        modifier = Modifier
            .fillMaxWidth()
            .combinedClickable(onClick = onClick, onLongClick = onLongClick)
            .semantics {
                contentDescription = accessibilityDescription
            }
    ) {
        Row(
            modifier = Modifier
                .fillMaxWidth()
                .padding(horizontal = 16.dp, vertical = 16.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Icon(
                imageVector = if (item.status == HistoryItemStatus.DRAFT) {
                    Icons.Rounded.Schedule
                } else {
                    Icons.Rounded.CheckCircle
                },
                contentDescription = null,
                tint = if (item.status == HistoryItemStatus.DRAFT) {
                    MaterialTheme.colorScheme.tertiary
                } else {
                    MaterialTheme.colorScheme.primary
                },
                modifier = Modifier.size(24.dp),
            )
            Spacer(Modifier.width(16.dp))
            Column(modifier = Modifier.weight(1f)) {
                Text(
                    text = item.title.ifBlank { fallbackTitle },
                    style = MaterialTheme.typography.titleMedium,
                    fontWeight = FontWeight.SemiBold,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Spacer(Modifier.height(6.dp))
                LibraryItemMetadata(item)
                val preview = item.searchMatch?.snippet ?: item.previewText
                if (preview.isNotBlank()) {
                    Spacer(Modifier.height(8.dp))
                    Text(
                        text = highlightedPreview(item, preview),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 2,
                        overflow = TextOverflow.Ellipsis,
                    )
                }
            }
            Spacer(Modifier.width(12.dp))
            Icon(
                imageVector = Icons.Rounded.ChevronRight,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
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
private fun ScopeChip(label: String, selected: Boolean, onClick: () -> Unit) {
    FilterChip(selected = selected, onClick = onClick, label = { Text(label) })
}

@Composable
internal fun LibraryItemMetadata(item: HistoryItem) {
    val status = if (item.status == HistoryItemStatus.DRAFT) {
        stringResource(R.string.library_status_draft)
    } else {
        stringResource(R.string.library_status_complete)
    }

    val statusColor = if (item.status == HistoryItemStatus.DRAFT) {
        MaterialTheme.colorScheme.tertiary
    } else {
        MaterialTheme.colorScheme.primary
    }

    Row(
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        Text(
            text = formatLibraryTimestamp(item.timestampEpochMillis),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            text = "•",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
        )
        Text(
            text = formatRecordingTimer(item.durationMillis),
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Text(
            text = "•",
            style = MaterialTheme.typography.labelMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f)
        )
        Card(
            shape = MaterialTheme.shapes.extraSmall,
            colors = CardDefaults.cardColors(
                containerColor = statusColor.copy(alpha = 0.12f),
                contentColor = statusColor
            )
        ) {
            Text(
                text = status,
                style = MaterialTheme.typography.labelSmall,
                fontWeight = FontWeight.Bold,
                modifier = Modifier.padding(horizontal = 6.dp, vertical = 2.dp)
            )
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
private fun LibraryEmpty(modifier: Modifier = Modifier) {
    Box(
        modifier = modifier
            .fillMaxWidth()
            .padding(vertical = 32.dp)
            .background(
                color = MaterialTheme.colorScheme.surfaceContainerLow,
                shape = MaterialTheme.shapes.medium
            ),
        contentAlignment = Alignment.Center
    ) {
        Column(
            modifier = Modifier.padding(32.dp),
            horizontalAlignment = Alignment.CenterHorizontally,
            verticalArrangement = Arrangement.spacedBy(8.dp)
        ) {
            Icon(
                imageVector = Icons.Rounded.FolderOpen,
                contentDescription = null,
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                modifier = Modifier.size(48.dp)
            )
            Text(
                text = stringResource(R.string.library_empty),
                style = MaterialTheme.typography.titleMedium,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
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
            .background(
                color = MaterialTheme.colorScheme.errorContainer.copy(alpha = 0.3f),
                shape = MaterialTheme.shapes.medium
            )
            .padding(32.dp),
        contentAlignment = Alignment.Center
    ) {
        Column(horizontalAlignment = Alignment.CenterHorizontally) {
            Text(
                text = stringResource(R.string.library_load_failed),
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

@Composable
private fun LibraryInlineError(onRetry: () -> Unit) {
    Card(
        shape = MaterialTheme.shapes.small,
        colors = CardDefaults.cardColors(
            containerColor = MaterialTheme.colorScheme.errorContainer,
            contentColor = MaterialTheme.colorScheme.onErrorContainer
        ),
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp)
    ) {
        Row(
            modifier = Modifier.padding(horizontal = 16.dp, vertical = 8.dp),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                text = stringResource(R.string.library_load_failed),
                style = MaterialTheme.typography.bodyMedium,
                fontWeight = FontWeight.Medium,
                modifier = Modifier.weight(1f),
            )
            FilledTonalButton(
                onClick = onRetry,
                contentPadding = PaddingValues(horizontal = 12.dp, vertical = 2.dp),
                modifier = Modifier.height(30.dp)
            ) {
                Text(
                    text = stringResource(R.string.action_retry),
                    style = MaterialTheme.typography.labelMedium
                )
            }
        }
    }
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
