package com.sona.android.application.recording

data class CreateLiveDraftRequest(
    val recordingId: String,
    val audioExtension: String,
)

data class RecordingDraft(
    val historyId: String,
    val destination: RecordingDestination,
)

data class CompleteLiveDraftRequest(
    val historyId: String,
    val segments: List<TranscriptSegment>,
    val durationMillis: Long,
)

data class HistoryRecordingSummary(
    val historyId: String,
)

interface RecordingHistoryPort {
    suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft

    suspend fun checkpointTranscript(
        historyId: String,
        segments: List<TranscriptSegment>,
    )

    suspend fun completeLiveDraft(request: CompleteLiveDraftRequest): HistoryRecordingSummary
    suspend fun deleteDraft(historyId: String)
}
