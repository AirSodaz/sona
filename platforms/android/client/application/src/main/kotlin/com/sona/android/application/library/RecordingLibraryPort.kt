package com.sona.android.application.library

import com.sona.android.application.recording.TranscriptSegment

enum class RecordingLibraryItemStatus {
    DRAFT,
    COMPLETE,
}

data class RecordingLibraryItem(
    val historyId: String,
    val title: String,
    val timestampEpochMillis: Long,
    val durationMillis: Long,
    val previewText: String,
    val status: RecordingLibraryItemStatus,
    val tagIds: List<String> = emptyList(),
    val deletedAtEpochMillis: Long? = null,
)

data class RecordingLibraryPage(
    val items: List<RecordingLibraryItem>,
    val hasMore: Boolean,
)

interface RecordingLibraryPort {
    suspend fun loadPage(offset: Int, limit: Int): RecordingLibraryPage

    suspend fun loadTranscript(historyId: String): List<TranscriptSegment>
}
