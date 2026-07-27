package com.sona.android.application.recording

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test

class TranscribeRecordingWithCloudTest {
    @Test
    fun `a saved recording keeps its duration and only replaces the transcript`() = runTest {
        val calls = mutableListOf<String>()
        val history = RecordingFakes.FakeRecordingHistory(calls)
        val segments = listOf(segment("cloud-1"))
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { result(segments) },
            history = history,
        )

        val outcome = transcribe(request())

        assertEquals(
            CloudTranscriptionOutcome.Completed(
                historyId = "history-1",
                provider = OnlineBatchProvider.GROQ_WHISPER,
                segments = segments,
            ),
            outcome,
        )
        assertEquals(listOf("history.checkpoint"), calls)
        assertEquals(listOf(segments), history.checkpointRequests)
    }

    @Test
    fun `a draft is completed with the reported audio duration`() = runTest {
        val calls = mutableListOf<String>()
        val history = RecordingFakes.FakeRecordingHistory(calls)
        val segments = listOf(segment("cloud-1"))
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { result(segments, audioDurationMillis = 4_200.6) },
            history = history,
        )

        transcribe(request(isDraft = true))

        assertEquals(listOf("history.complete"), calls)
        assertEquals(
            CompleteLiveDraftRequest(
                historyId = "history-1",
                segments = segments,
                durationMillis = 4_201,
            ),
            history.completedRequest,
        )
    }

    @Test
    fun `a non-finite duration completes a draft at zero instead of failing`() = runTest {
        val history = RecordingFakes.FakeRecordingHistory(mutableListOf())
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { result(listOf(segment("cloud-1")), Double.NaN) },
            history = history,
        )

        transcribe(request(isDraft = true))

        assertEquals(0L, history.completedRequest?.durationMillis)
    }

    @Test
    fun `the request is forwarded with the active provider and language`() = runTest {
        var forwarded: OnlineBatchTranscriptionRequest? = null
        val transcribe = TranscribeRecordingWithCloud(
            credentials = {
                ActiveBatchCredential(
                    provider = OnlineBatchProvider.MISTRAL_VOXTRAL,
                    credential = OnlineBatchCredential("temporary-secret"),
                )
            },
            transcription = { request ->
                forwarded = request
                result(listOf(segment("cloud-1")))
            },
            history = RecordingFakes.FakeRecordingHistory(mutableListOf()),
        )

        transcribe(request())

        assertEquals(
            OnlineBatchTranscriptionRequest(
                audioPath = "/recordings/history-1.wav",
                provider = OnlineBatchProvider.MISTRAL_VOXTRAL,
                credential = OnlineBatchCredential("temporary-secret"),
                language = "auto",
            ),
            forwarded,
        )
    }

    @Test
    fun `missing audio is rejected before any credential is resolved`() = runTest {
        val calls = mutableListOf<String>()
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { throw AssertionError("must not resolve a credential") },
            transcription = { throw AssertionError("must not transcribe") },
            history = RecordingFakes.FakeRecordingHistory(calls),
        )

        val missingFlag = transcribe(request(audioAvailable = false))
        val blankPath = transcribe(request(audioPath = "  "))

        assertEquals(failed(CloudTranscriptionFailure.MISSING_AUDIO), missingFlag)
        assertEquals(failed(CloudTranscriptionFailure.MISSING_AUDIO), blankPath)
        assertEquals(emptyList<String>(), calls)
    }

    @Test
    fun `an absent blank or failing credential never reaches the provider`() = runTest {
        val calls = mutableListOf<String>()
        val resolvers = listOf<BatchCredentialResolverPort>(
            BatchCredentialResolverPort { null },
            BatchCredentialResolverPort {
                ActiveBatchCredential(
                    provider = OnlineBatchProvider.GROQ_WHISPER,
                    credential = OnlineBatchCredential("   "),
                )
            },
            BatchCredentialResolverPort { throw IllegalStateException("keystore detail") },
        )

        resolvers.forEach { credentials ->
            val transcribe = TranscribeRecordingWithCloud(
                credentials = credentials,
                transcription = { throw AssertionError("must not transcribe") },
                history = RecordingFakes.FakeRecordingHistory(calls),
            )

            assertEquals(failed(CloudTranscriptionFailure.MISSING_CREDENTIAL), transcribe(request()))
        }
        assertEquals(emptyList<String>(), calls)
    }

    @Test
    fun `a provider failure is categorized and leaves history untouched`() = runTest {
        val calls = mutableListOf<String>()
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { throw IllegalStateException("Bearer sk-live-secret") },
            history = RecordingFakes.FakeRecordingHistory(calls),
        )

        val outcome = transcribe(request())

        assertEquals(failed(CloudTranscriptionFailure.TRANSCRIPTION_FAILED), outcome)
        assertEquals(emptyList<String>(), calls)
        assertFalse(outcome.toString().contains("sk-live-secret"))
    }

    @Test
    fun `an empty cloud result never overwrites a stored transcript`() = runTest {
        val calls = mutableListOf<String>()
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { result(emptyList()) },
            history = RecordingFakes.FakeRecordingHistory(calls),
        )

        val saved = transcribe(request())
        val draft = transcribe(request(isDraft = true))

        assertEquals(failed(CloudTranscriptionFailure.EMPTY_TRANSCRIPT), saved)
        assertEquals(failed(CloudTranscriptionFailure.EMPTY_TRANSCRIPT), draft)
        assertEquals(emptyList<String>(), calls)
    }

    @Test
    fun `a persistence failure is reported without a completed outcome`() = runTest {
        val history = RecordingFakes.FakeRecordingHistory(mutableListOf()).apply {
            checkpointFailuresRemaining = 1
        }
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { result(listOf(segment("cloud-1"))) },
            history = history,
        )

        assertEquals(failed(CloudTranscriptionFailure.PERSISTENCE_FAILED), transcribe(request()))
    }

    @Test
    fun `cancellation propagates instead of becoming a failure outcome`() {
        val cancellation = CancellationException("cancelled")
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { throw cancellation },
            history = RecordingFakes.FakeRecordingHistory(mutableListOf()),
        )

        val thrown = assertThrows(CancellationException::class.java) {
            runBlocking { transcribe(request()) }
        }

        assertEquals(cancellation, thrown)
        assertNull(thrown.cause)
    }

    @Test
    fun `a blank history id is a programming error`() {
        val transcribe = TranscribeRecordingWithCloud(
            credentials = { activeCredential() },
            transcription = { result(listOf(segment("cloud-1"))) },
            history = RecordingFakes.FakeRecordingHistory(mutableListOf()),
        )

        assertThrows(IllegalArgumentException::class.java) {
            runBlocking { transcribe(request(historyId = " ")) }
        }
    }

    private fun request(
        historyId: String = "history-1",
        audioPath: String = "/recordings/history-1.wav",
        audioAvailable: Boolean = true,
        isDraft: Boolean = false,
    ) = CloudTranscriptionRequest(
        historyId = historyId,
        audioPath = audioPath,
        audioAvailable = audioAvailable,
        isDraft = isDraft,
    )

    private fun failed(reason: CloudTranscriptionFailure) =
        CloudTranscriptionOutcome.Failed(historyId = "history-1", reason = reason)

    private fun activeCredential() = ActiveBatchCredential(
        provider = OnlineBatchProvider.GROQ_WHISPER,
        credential = OnlineBatchCredential("temporary-secret"),
    )

    private fun result(
        segments: List<TranscriptSegment>,
        audioDurationMillis: Double = 1_000.0,
    ) = OnlineBatchTranscriptionResult(
        segments = segments,
        audioDurationMillis = audioDurationMillis,
        bufferedSamples = 16_000uL,
        stage = "batch_complete",
    )

    private fun segment(id: String) = TranscriptSegment(
        id = id,
        text = "Hello",
        startSeconds = 0.0,
        endSeconds = 1.0,
        isFinal = true,
    )
}
