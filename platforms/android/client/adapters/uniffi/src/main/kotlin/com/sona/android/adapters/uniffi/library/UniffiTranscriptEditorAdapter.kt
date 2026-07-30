package com.sona.android.adapters.uniffi.library

import com.sona.android.adapters.uniffi.recording.toApplication
import com.sona.android.adapters.uniffi.recording.toFfi
import com.sona.android.application.library.CommitTranscriptEditRequest
import com.sona.android.application.library.CommitTranscriptEditResult
import com.sona.android.application.library.HistoryMediaSource
import com.sona.android.application.library.HistoryMediaSourcePort
import com.sona.android.application.library.TranscriptEditException
import com.sona.android.application.library.TranscriptEditFailure
import com.sona.android.application.library.TranscriptEditOperation
import com.sona.android.application.library.TranscriptEditorPort
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.TranscriptSnapshotReason
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.CancellationException
import uniffi.sona_uniffi_bind.FfiHistoryCommitTranscriptEditRequestV1
import uniffi.sona_uniffi_bind.FfiHistoryCommitTranscriptEditResultV1
import uniffi.sona_uniffi_bind.FfiTranscriptEditOperationV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotMetadataV1
import uniffi.sona_uniffi_bind.applyTranscriptEditV1
import uniffi.sona_uniffi_bind.commitHistoryTranscriptEditV1
import uniffi.sona_uniffi_bind.resolveHistoryAudioSourceV1

class UniffiTranscriptEditorAdapter(
    private val appDataDir: String,
    private val onLocalChange: () -> Unit = {},
) : TranscriptEditorPort, HistoryMediaSourcePort {
    init { require(appDataDir.isNotBlank()) { "History app data directory must not be blank." } }

    override suspend fun apply(
        segments: List<TranscriptSegment>,
        operation: TranscriptEditOperation,
    ): List<TranscriptSegment> = mapFailure(TranscriptEditFailure.INVALID_EDIT) {
        applyTranscriptEditV1(segments.map(TranscriptSegment::toFfi), operation.toFfi())
            .map { it.toApplication() }
    }

    override suspend fun commit(request: CommitTranscriptEditRequest): CommitTranscriptEditResult =
        mapFailure(TranscriptEditFailure.PERSISTENCE) {
            when (val result = commitHistoryTranscriptEditV1(
                appDataDir,
                FfiHistoryCommitTranscriptEditRequestV1(
                    historyId = request.historyId,
                    editSessionId = request.editSessionId,
                    baseSegments = request.baseSegments.map(TranscriptSegment::toFfi),
                    editedSegments = request.editedSegments.map(TranscriptSegment::toFfi),
                ),
            )) {
                FfiHistoryCommitTranscriptEditResultV1.Unchanged -> CommitTranscriptEditResult.Unchanged
                is FfiHistoryCommitTranscriptEditResultV1.Committed -> {
                    onLocalChange()
                    CommitTranscriptEditResult.Committed(result.snapshot.toApplication())
                }
                is FfiHistoryCommitTranscriptEditResultV1.Conflict -> CommitTranscriptEditResult.Conflict(
                    result.currentSegments.map { it.toApplication() },
                )
            }
        }

    override suspend fun resolve(historyId: String): HistoryMediaSource? =
        mapFailure(TranscriptEditFailure.NOT_FOUND) {
            require(historyId.isNotBlank()) { "History ID must not be blank." }
            resolveHistoryAudioSourceV1(appDataDir, historyId)?.let(::HistoryMediaSource)
        }
}

internal fun TranscriptEditOperation.toFfi(): FfiTranscriptEditOperationV1 = when (this) {
    is TranscriptEditOperation.UpdateText -> FfiTranscriptEditOperationV1.UpdateText(segmentId, text)
    is TranscriptEditOperation.UpdateTranslation ->
        FfiTranscriptEditOperationV1.UpdateTranslation(segmentId, translation)
    is TranscriptEditOperation.Delete -> FfiTranscriptEditOperationV1.Delete(segmentId)
    is TranscriptEditOperation.MergeNext -> FfiTranscriptEditOperationV1.MergeNext(segmentId)
    is TranscriptEditOperation.Split -> FfiTranscriptEditOperationV1.Split(
        segmentId,
        newSegmentId,
        leftText,
        rightText,
        leftTranslation,
        rightTranslation,
    )
}

internal fun FfiTranscriptSnapshotMetadataV1.toApplication() = TranscriptSnapshot(
    id = id,
    historyId = historyId,
    reason = when (reason) {
        uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1.POLISH -> TranscriptSnapshotReason.POLISH
        uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1.TRANSLATE -> TranscriptSnapshotReason.TRANSLATE
        uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1.RETRANSCRIBE -> TranscriptSnapshotReason.RETRANSCRIBE
        uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1.RESTORE -> TranscriptSnapshotReason.RESTORE
        uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1.MANUAL_EDIT -> TranscriptSnapshotReason.MANUAL_EDIT
    },
    createdAtEpochMillis = createdAt.toLong(),
    segmentCount = segmentCount.toLong(),
)

private suspend inline fun <T> mapFailure(
    failure: TranscriptEditFailure,
    crossinline operation: suspend () -> T,
): T = try {
    operation()
} catch (error: CancellationException) {
    throw error
} catch (error: Exception) {
    throw TranscriptEditException(failure, error)
}
