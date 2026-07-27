package com.sona.android.application.recording

import kotlin.math.roundToLong
import kotlinx.coroutines.CancellationException

enum class CloudTranscriptionFailure {
    MISSING_CREDENTIAL,
    MISSING_AUDIO,
    TRANSCRIPTION_FAILED,
    EMPTY_TRANSCRIPT,
    PERSISTENCE_FAILED,
}

data class CloudTranscriptionRequest(
    val historyId: String,
    val audioPath: String,
    val audioAvailable: Boolean,
    val isDraft: Boolean,
)

sealed interface CloudTranscriptionOutcome {
    val historyId: String

    data class Completed(
        override val historyId: String,
        val provider: OnlineBatchProvider,
        val segments: List<TranscriptSegment>,
    ) : CloudTranscriptionOutcome

    data class Failed(
        override val historyId: String,
        val reason: CloudTranscriptionFailure,
    ) : CloudTranscriptionOutcome
}

/**
 * Re-transcribes an already recorded audio file through a cloud batch provider
 * and persists the result. A draft is completed with the reported audio
 * duration; a saved recording keeps its duration and only replaces the
 * transcript.
 */
class TranscribeRecordingWithCloud(
    private val credentials: BatchCredentialResolverPort,
    private val transcription: OnlineBatchTranscriptionPort,
    private val history: RecordingHistoryPort,
    private val language: String = DEFAULT_LANGUAGE,
) {
    suspend operator fun invoke(request: CloudTranscriptionRequest): CloudTranscriptionOutcome {
        require(request.historyId.isNotBlank()) { "History ID must not be blank." }
        if (!request.audioAvailable || request.audioPath.isBlank()) {
            return request.failed(CloudTranscriptionFailure.MISSING_AUDIO)
        }

        val active = try {
            credentials.loadActive()
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            return request.failed(CloudTranscriptionFailure.MISSING_CREDENTIAL)
        }
        if (active == null || active.credential.apiKey.isBlank()) {
            return request.failed(CloudTranscriptionFailure.MISSING_CREDENTIAL)
        }

        val result = try {
            transcription.transcribe(
                OnlineBatchTranscriptionRequest(
                    audioPath = request.audioPath,
                    provider = active.provider,
                    credential = active.credential,
                    language = language,
                ),
            )
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            return request.failed(CloudTranscriptionFailure.TRANSCRIPTION_FAILED)
        }

        // An empty cloud result must never replace a transcript that already exists.
        if (result.segments.isEmpty()) {
            return request.failed(CloudTranscriptionFailure.EMPTY_TRANSCRIPT)
        }

        try {
            if (request.isDraft) {
                history.completeLiveDraft(
                    CompleteLiveDraftRequest(
                        historyId = request.historyId,
                        segments = result.segments,
                        durationMillis = result.audioDurationMillis.toDurationMillis(),
                    ),
                )
            } else {
                history.checkpointTranscript(request.historyId, result.segments)
            }
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            return request.failed(CloudTranscriptionFailure.PERSISTENCE_FAILED)
        }

        return CloudTranscriptionOutcome.Completed(
            historyId = request.historyId,
            provider = active.provider,
            segments = result.segments,
        )
    }

    private fun CloudTranscriptionRequest.failed(
        reason: CloudTranscriptionFailure,
    ): CloudTranscriptionOutcome = CloudTranscriptionOutcome.Failed(historyId, reason)

    private fun Double.toDurationMillis(): Long = if (isFinite() && this > 0.0) {
        coerceAtMost(MAX_DURATION_MILLIS).roundToLong()
    } else {
        0L
    }

    companion object {
        const val DEFAULT_LANGUAGE = "auto"
        private const val MAX_DURATION_MILLIS = 1_000_000_000_000.0
    }
}
