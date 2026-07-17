package com.sona.android.application.recording

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class OnlineBatchPortsContractTest {
    @Test
    fun `online batch transcription is provider neutral and keeps credentials redacted`() = runTest {
        val credential = OnlineBatchCredential("temporary-secret")
        val port = RecordingOnlineBatchTranscriptionPort()

        val results = OnlineBatchProvider.entries.map { provider ->
            port.transcribe(
                OnlineBatchTranscriptionRequest(
                    audioPath = "recording.wav",
                    provider = provider,
                    credential = credential,
                    language = "auto",
                ),
            )
        }

        assertFalse(credential.toString().contains("temporary-secret"))
        assertEquals(
            listOf(
                OnlineBatchProvider.VOLCENGINE_DOUBAO,
                OnlineBatchProvider.GROQ_WHISPER,
                OnlineBatchProvider.MISTRAL_VOXTRAL,
            ),
            port.requests.map(OnlineBatchTranscriptionRequest::provider),
        )
        assertEquals(
            List(3) {
                OnlineBatchTranscriptionResult(
                    segments = listOf(segment()),
                    audioDurationMillis = 1_500.0,
                    bufferedSamples = 24_000u,
                    stage = "batch-complete",
                )
            },
            results,
        )
    }

    private class RecordingOnlineBatchTranscriptionPort : OnlineBatchTranscriptionPort {
        val requests = mutableListOf<OnlineBatchTranscriptionRequest>()

        override suspend fun transcribe(
            request: OnlineBatchTranscriptionRequest,
        ): OnlineBatchTranscriptionResult {
            requests += request
            return OnlineBatchTranscriptionResult(
                segments = listOf(segment()),
                audioDurationMillis = 1_500.0,
                bufferedSamples = 24_000u,
                stage = "batch-complete",
            )
        }
    }

    companion object {
        private fun segment() = TranscriptSegment(
            id = "segment-1",
            text = "hello",
            startSeconds = 0.25,
            endSeconds = 1.5,
            isFinal = true,
        )
    }
}
