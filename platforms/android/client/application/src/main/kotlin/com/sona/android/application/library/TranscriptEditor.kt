package com.sona.android.application.library

import com.sona.android.application.recording.TranscriptSegment

sealed interface TranscriptEditOperation {
    data class UpdateText(val segmentId: String, val text: String) : TranscriptEditOperation
    data class UpdateTranslation(val segmentId: String, val translation: String?) : TranscriptEditOperation
    data class Delete(val segmentId: String) : TranscriptEditOperation
    data class MergeNext(val segmentId: String) : TranscriptEditOperation
    data class Split(
        val segmentId: String,
        val newSegmentId: String,
        val leftText: String,
        val rightText: String,
        val leftTranslation: String?,
        val rightTranslation: String?,
    ) : TranscriptEditOperation
}

data class CommitTranscriptEditRequest(
    val historyId: String,
    val editSessionId: String,
    val baseSegments: List<TranscriptSegment>,
    val editedSegments: List<TranscriptSegment>,
)

sealed interface CommitTranscriptEditResult {
    data object Unchanged : CommitTranscriptEditResult
    data class Committed(val snapshot: TranscriptSnapshot) : CommitTranscriptEditResult
    data class Conflict(val currentSegments: List<TranscriptSegment>) : CommitTranscriptEditResult
}

enum class TranscriptEditFailure { INVALID_EDIT, NOT_FOUND, PERSISTENCE }

class TranscriptEditException(
    val failure: TranscriptEditFailure,
    cause: Throwable? = null,
) : Exception(failure.name, cause)

interface TranscriptEditorPort {
    suspend fun apply(
        segments: List<TranscriptSegment>,
        operation: TranscriptEditOperation,
    ): List<TranscriptSegment>

    suspend fun commit(request: CommitTranscriptEditRequest): CommitTranscriptEditResult
}

data class HistoryMediaSource(val nativePath: String)

interface HistoryMediaSourcePort {
    suspend fun resolve(historyId: String): HistoryMediaSource?
}
