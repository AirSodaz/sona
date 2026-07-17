package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineBatchProvider
import com.sona.android.application.recording.OnlineBatchTranscriptionRequest
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiSpeakerAttribution
import uniffi.sona_uniffi_bind.FfiSpeakerCandidate
import uniffi.sona_uniffi_bind.FfiSpeakerTag
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.FfiTranscriptTiming
import uniffi.sona_uniffi_bind.FfiTranscriptTimingLevel
import uniffi.sona_uniffi_bind.FfiTranscriptTimingSource
import uniffi.sona_uniffi_bind.FfiTranscriptTimingUnit

class UniffiOnlineBatchTranscriptionAdapterTest {
    @Test
    fun `adapter maps all supported providers and keeps temporary credentials redacted`() = runTest {
        val bindings = FakeOnlineBatchBindings()
        val adapter = UniffiOnlineBatchTranscriptionAdapter(bindings)
        val credential = OnlineBatchCredential("temporary-secret")

        OnlineBatchProvider.entries.forEach { provider ->
            adapter.transcribe(request(provider, credential))
        }

        assertEquals(
            listOf(
                UniffiOnlineBatchProvider.VOLCENGINE_DOUBAO,
                UniffiOnlineBatchProvider.GROQ_WHISPER,
                UniffiOnlineBatchProvider.MISTRAL_VOXTRAL,
            ),
            bindings.requests.map(UniffiOnlineBatchRequest::provider),
        )
        assertFalse(bindings.requests.toString().contains("temporary-secret"))
        assertEquals(List(3) { credential }, bindings.requests.map(UniffiOnlineBatchRequest::credential))
    }

    @Test
    fun `adapter maps every transcript field and batch metric without loss`() = runTest {
        val bindings = FakeOnlineBatchBindings()

        val result = UniffiOnlineBatchTranscriptionAdapter(bindings).transcribe(
            request(OnlineBatchProvider.GROQ_WHISPER, OnlineBatchCredential("secret")),
        )

        val segment = result.segments.single()
        assertEquals(
            listOf(
                "segment-1", "hello", 1.25, 2.5, true,
                TranscriptTimingLevel.TOKEN, TranscriptTimingSource.MODEL, "he",
                listOf("he", "llo"), listOf(1.25f, 1.75f), listOf(0.5f, 0.75f),
                "\u4f60\u597d", "speaker-1", 0.8f,
                listOf("group-1", "Speaker 1", "matched", "embedding", "high"),
                listOf("profile-1", "Alice", 0.9f, 1uL),
                1_500.0, 24_000uL, "groq_batch_complete",
            ),
            listOf(
                segment.id,
                segment.text,
                segment.startSeconds,
                segment.endSeconds,
                segment.isFinal,
                segment.timing?.level,
                segment.timing?.source,
                segment.timing?.units?.single()?.text,
                segment.tokens,
                segment.timestamps,
                segment.durations,
                segment.translation,
                segment.speaker?.id,
                segment.speaker?.score,
                segment.speakerAttribution?.let {
                    listOf(it.groupId, it.anonymousLabel, it.state, it.source, it.confidence)
                },
                segment.speakerAttribution?.candidates?.single()?.let {
                    listOf(it.profileId, it.profileName, it.score, it.rank)
                },
                result.audioDurationMillis,
                result.bufferedSamples,
                result.stage,
            ),
        )
    }

    private fun request(
        provider: OnlineBatchProvider,
        credential: OnlineBatchCredential,
    ) = OnlineBatchTranscriptionRequest(
        audioPath = "recording.wav",
        provider = provider,
        credential = credential,
        language = "auto",
    )

    private class FakeOnlineBatchBindings : UniffiOnlineBatchBindings {
        val requests = mutableListOf<UniffiOnlineBatchRequest>()

        override suspend fun transcribe(request: UniffiOnlineBatchRequest): UniffiOnlineBatchResult {
            requests += request
            return UniffiOnlineBatchResult(
                segments = listOf(fullSegment()),
                audioDurationMillis = 1_500.0,
                bufferedSamples = 24_000u,
                stage = "groq_batch_complete",
            )
        }
    }

    companion object {
        private fun fullSegment() = FfiTranscriptSegment(
            id = "segment-1",
            text = "hello",
            start = 1.25,
            end = 2.5,
            isFinal = true,
            timing = FfiTranscriptTiming(
                level = FfiTranscriptTimingLevel.TOKEN,
                source = FfiTranscriptTimingSource.MODEL,
                units = listOf(FfiTranscriptTimingUnit("he", 1.25, 1.5)),
            ),
            tokens = listOf("he", "llo"),
            timestamps = listOf(1.25f, 1.75f),
            durations = listOf(0.5f, 0.75f),
            translation = "\u4f60\u597d",
            speaker = FfiSpeakerTag(
                id = "speaker-1",
                label = "Speaker 1",
                kind = "known",
                score = 0.8f,
            ),
            speakerAttribution = FfiSpeakerAttribution(
                groupId = "group-1",
                anonymousLabel = "Speaker 1",
                state = "matched",
                source = "embedding",
                confidence = "high",
                candidates = listOf(
                    FfiSpeakerCandidate(
                        profileId = "profile-1",
                        profileName = "Alice",
                        score = 0.9f,
                        rank = 1u,
                    ),
                ),
            ),
        )
    }
}
