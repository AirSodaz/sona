package com.sona.android.application.library

import com.sona.android.application.recording.TranscriptSegment

enum class HistoryItemStatus { DRAFT, COMPLETE }
enum class HistoryItemKind { RECORDING, BATCH }
enum class HistoryFilterType { ALL, RECORDING, BATCH }
enum class HistoryDateFilter { ALL, TODAY, WEEK, MONTH }
enum class HistorySortOrder { NEWEST, OLDEST, DURATION_DESC, DURATION_ASC, TITLE_ASC }
enum class TranscriptSnapshotReason { POLISH, TRANSLATE, RETRANSCRIBE, RESTORE }

sealed interface HistoryScope {
    data object All : HistoryScope
    data object Untagged : HistoryScope
    data object Trash : HistoryScope
    data class Tag(val tagId: String) : HistoryScope
}

data class HistoryWorkspaceQuery(
    val scope: HistoryScope = HistoryScope.All,
    val query: String = "",
    val filterType: HistoryFilterType = HistoryFilterType.ALL,
    val dateFilter: HistoryDateFilter = HistoryDateFilter.ALL,
    val sortOrder: HistorySortOrder = HistorySortOrder.NEWEST,
    val offset: Int = 0,
    val limit: Int = 30,
)

data class HistorySearchMatch(
    val field: String,
    val snippet: String,
    val highlightStart: Int,
    val highlightEnd: Int,
)

data class HistoryItem(
    val historyId: String,
    val title: String,
    val timestampEpochMillis: Long,
    val durationMillis: Long,
    val previewText: String,
    val status: HistoryItemStatus,
    val kind: HistoryItemKind,
    val tagIds: List<String> = emptyList(),
    val deletedAtEpochMillis: Long? = null,
    val audioPath: String = "",
    val audioAvailable: Boolean = false,
    val icon: String? = null,
    val searchMatch: HistorySearchMatch? = null,
)

data class HistoryWorkspaceSummary(
    val totalItems: Long,
    val totalDurationMillis: Long,
    val latestTimestampEpochMillis: Long?,
    val recordingCount: Long,
    val batchCount: Long,
)

data class HistoryWorkspaceCounts(
    val untagged: Long,
    val trash: Long,
    val byTagId: Map<String, Long>,
)

data class HistoryWorkspacePage(
    val items: List<HistoryItem>,
    val filteredItemCount: Long,
    val hasMore: Boolean,
    val summary: HistoryWorkspaceSummary,
    val counts: HistoryWorkspaceCounts,
)

data class TranscriptSnapshot(
    val id: String,
    val historyId: String,
    val reason: TranscriptSnapshotReason,
    val createdAtEpochMillis: Long,
    val segmentCount: Long,
)

data class TranscriptSnapshotDetail(
    val metadata: TranscriptSnapshot,
    val segments: List<TranscriptSegment>,
)

interface HistoryWorkspacePort {
    suspend fun query(request: HistoryWorkspaceQuery): HistoryWorkspacePage
    suspend fun loadTranscript(historyId: String): List<TranscriptSegment>
    suspend fun updateTitle(historyId: String, title: String)
    suspend fun updateTags(ids: List<String>, addTagIds: List<String>, removeTagIds: List<String>)
    suspend fun trash(ids: List<String>, deletedAtEpochMillis: Long)
    suspend fun restore(ids: List<String>)
    suspend fun purge(ids: List<String>)
    suspend fun listSnapshots(historyId: String): List<TranscriptSnapshot>
    suspend fun loadSnapshot(historyId: String, snapshotId: String): TranscriptSnapshotDetail?
}
