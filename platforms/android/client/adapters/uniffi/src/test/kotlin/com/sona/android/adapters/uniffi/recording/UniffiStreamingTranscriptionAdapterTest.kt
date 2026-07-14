package com.sona.android.adapters.uniffi.recording

import com.sona.android.application.recording.Pcm16Frame
import com.sona.android.application.recording.StreamingCredential
import com.sona.android.application.recording.StreamingProviderProfile
import com.sona.android.application.recording.StreamingTranscriptionEvent
import com.sona.android.application.recording.StreamingTranscriptionRequest
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import kotlinx.coroutines.async
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiAsrStreamingErrorEvent
import uniffi.sona_uniffi_bind.FfiAsrStreamingObserver
import uniffi.sona_uniffi_bind.FfiAsrTranscriptUpdateEvent
import uniffi.sona_uniffi_bind.FfiSpeakerAttribution
import uniffi.sona_uniffi_bind.FfiSpeakerCandidate
import uniffi.sona_uniffi_bind.FfiSpeakerTag
import uniffi.sona_uniffi_bind.FfiTranscriptSegment
import uniffi.sona_uniffi_bind.FfiTranscriptTiming
import uniffi.sona_uniffi_bind.FfiTranscriptTimingLevel
import uniffi.sona_uniffi_bind.FfiTranscriptTimingSource
import uniffi.sona_uniffi_bind.FfiTranscriptTimingUnit
import uniffi.sona_uniffi_bind.FfiTranscriptUpdate

class UniffiStreamingTranscriptionAdapterTest {
    @Test
    fun `open builds the online request and the session delegates its lifecycle`() = runTest {
        val bindings = FakeStreamingBindings()
        val session = UniffiStreamingTranscriptionAdapter(bindings).open(request())
        val events = async { session.events.toList() }

        session.start()
        session.feed(Pcm16Frame(byteArrayOf(1, 2, 3)))
        session.flush()
        session.stop()
        session.close()
        session.close()

        val config = parseJsonObject(checkNotNull(bindings.resolvedConfigJson), "config")
        val root = parseJsonObject(checkNotNull(bindings.requestJson), "request")
        val provider = root.getValue("onlineProvider").jsonObject
        assertEquals(
            mapOf(
                "provider" to "volcengine-doubao",
                "profile" to "volcengine-doubao-default",
                "credential" to "secret-key",
                "instance" to "recording-1",
                "mode" to "streaming",
                "engine" to "online",
                "canonicalCredential" to "canonical-key",
            ),
            mapOf(
                "provider" to bindings.resolvedProviderId,
                "profile" to bindings.resolvedProfileId,
                "credential" to config.string("apiKey"),
                "instance" to bindings.instanceId,
                "mode" to root.string("mode"),
                "engine" to root.string("engine"),
                "canonicalCredential" to provider.getValue("config").jsonObject.string("apiKey"),
            ),
        )
        events.await()
        assertEquals(listOf("start", "feed", "flush", "stop", "close"), bindings.handle.calls)
        assertArrayEquals(byteArrayOf(1, 2, 3), bindings.handle.fedBytes.single())
    }

    @Test
    fun `observer maps every transcript field without loss`() = runTest {
        val bindings = FakeStreamingBindings()
        val session = UniffiStreamingTranscriptionAdapter(bindings).open(request())
        val event = async { session.events.first() }

        checkNotNull(bindings.observer).onTranscriptUpdate(fullTranscriptEvent())

        val transcript = event.await() as StreamingTranscriptionEvent.Transcript
        val segment = transcript.update.upsertSegments.single()
        assertEquals(
            listOf(
                listOf("old"), "segment-1", "hello", 1.25, 2.5, true,
                TranscriptTimingLevel.TOKEN, TranscriptTimingSource.MODEL, "he",
                listOf("he", "llo"), listOf(1.25f, 1.75f), listOf(0.5f, 0.75f),
                "\u4f60\u597d", "speaker-1", 0.8f,
                listOf("group-1", "Speaker 1", "matched", "embedding", "high"),
                listOf("profile-1", "Alice", 0.9f, 1uL),
            ),
            listOf(
                transcript.update.removeIds,
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
            ),
        )
        session.close()
    }

    @Test
    fun `observer maps a structured stream error and then completes events`() = runTest {
        val bindings = FakeStreamingBindings()
        val session = UniffiStreamingTranscriptionAdapter(bindings).open(request())
        val events = async { session.events.toList() }

        checkNotNull(bindings.observer).onStreamingError(
            FfiAsrStreamingErrorEvent(
                instanceId = "recording-1",
                code = "VOLCENGINE_WEB_SOCKET_CLOSED",
                message = "remote closed",
            ),
        )

        assertEquals(
            listOf(
                StreamingTranscriptionEvent.Failure(
                    code = "VOLCENGINE_WEB_SOCKET_CLOSED",
                    message = "remote closed",
                ),
            ),
            events.await(),
        )
        session.close()
    }

    @Test
    fun `a callback for another instance becomes a terminal adapter failure`() = runTest {
        val bindings = FakeStreamingBindings()
        val session = UniffiStreamingTranscriptionAdapter(bindings).open(request())
        val events = async { session.events.toList() }

        checkNotNull(bindings.observer).onTranscriptUpdate(
            fullTranscriptEvent().copy(instanceId = "recording-2"),
        )

        assertEquals(
            "UNEXPECTED_STREAM_INSTANCE",
            (events.await().single() as StreamingTranscriptionEvent.Failure).code,
        )
        session.close()
    }

    @Test
    fun `stop failure still completes events and close remains idempotent`() = runTest {
        val bindings = FakeStreamingBindings()
        bindings.handle.stopFailure = IllegalStateException("stop detail")
        val session = UniffiStreamingTranscriptionAdapter(bindings).open(request())
        val events = async { session.events.toList() }

        val failure = runCatching { session.stop() }.exceptionOrNull()
        session.close()
        session.close()
        session.stop()

        events.await()
        assertEquals(
            "stop detail" to listOf("stop", "close"),
            failure?.message to bindings.handle.calls,
        )
    }

    private fun request() = StreamingTranscriptionRequest(
        recordingId = "recording-1",
        credential = StreamingCredential("secret-key"),
        profile = StreamingProviderProfile(
            providerId = "volcengine-doubao",
            profileId = "volcengine-doubao-default",
            streamingEndpoint = "wss://stream.example",
            streamingResourceId = "stream-resource",
        ),
        language = "auto",
        enableItn = true,
    )

    private fun fullTranscriptEvent() = FfiAsrTranscriptUpdateEvent(
        instanceId = "recording-1",
        stage = "volcengine_streaming",
        update = FfiTranscriptUpdate(
            removeIds = listOf("old"),
            upsertSegments = listOf(
                FfiTranscriptSegment(
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
                ),
            ),
        ),
    )

    private class FakeStreamingBindings : UniffiStreamingBindings {
        var resolvedProviderId: String? = null
        var resolvedProfileId: String? = null
        var resolvedConfigJson: String? = null
        var instanceId: String? = null
        var requestJson: String? = null
        var observer: FfiAsrStreamingObserver? = null
        val handle = FakeSessionHandle()

        override fun resolveProviderRequest(
            providerId: String,
            profileId: String,
            configJson: String,
        ): UniffiOnlineProviderRequest {
            resolvedProviderId = providerId
            resolvedProfileId = profileId
            resolvedConfigJson = configJson
            return UniffiOnlineProviderRequest(
                providerId = providerId,
                profileId = profileId,
                configJson = "{\"apiKey\":\"canonical-key\"}",
            )
        }

        override fun createSession(
            instanceId: String,
            requestJson: String,
            observer: FfiAsrStreamingObserver,
        ): UniffiStreamingSessionHandle {
            this.instanceId = instanceId
            this.requestJson = requestJson
            this.observer = observer
            return handle
        }
    }

    private class FakeSessionHandle : UniffiStreamingSessionHandle {
        val calls = mutableListOf<String>()
        val fedBytes = mutableListOf<ByteArray>()
        var stopFailure: Throwable? = null

        override suspend fun start() {
            calls += "start"
        }

        override suspend fun feedAudioChunk(bytes: ByteArray) {
            calls += "feed"
            fedBytes += bytes.copyOf()
        }

        override suspend fun flush() {
            calls += "flush"
        }

        override suspend fun stop() {
            calls += "stop"
            stopFailure?.let { throw it }
        }

        override fun close() {
            calls += "close"
        }
    }
}

private fun JsonObject.string(key: String): String = getValue(key).jsonPrimitive.content
