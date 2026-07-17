package com.sona.android.application.recording

import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.emptyFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test

class RecordingPortsContractTest {
    @Test
    fun `recording adapters can satisfy every port without platform types`() = runTest {
        val credentialSettings = MemoryCredentialSettings()
        val credential = StreamingCredential(apiKey = "secret")
        assertFalse(credential.toString().contains("secret"))
        credentialSettings.save(credential)
        assertEquals(CredentialStatus.CONFIGURED, credentialSettings.status.first())
        assertEquals(
            setOf("clear", "getStatus", "save"),
            StreamingCredentialSettingsPort::class.java.declaredMethods.map { it.name }.toSet(),
        )

        val credentialResolver = StreamingCredentialResolverPort { credential }
        assertEquals(credential, credentialResolver.loadForStart())

        val profile = StreamingProviderProfile(
            providerId = "volcengine-doubao",
            profileId = "volcengine-doubao-default",
            streamingEndpoint = "wss://example.test/stream",
            streamingResourceId = "resource-id",
        )
        val catalog = StreamingProviderCatalogPort { profile }
        assertEquals(profile, catalog.loadVolcengineStreamingProfile())

        val destination = RecordingDestination("managed-recording.wav")
        val history = MemoryHistoryPort(destination)
        val draft = history.createLiveDraft(
            CreateLiveDraftRequest(recordingId = "live-1", audioExtension = "wav"),
        )
        assertEquals("live-1", draft.historyId)
        assertEquals(destination, draft.destination)

        val microphone = RecordingMicrophonePort()
        val microphoneSession = microphone.open(
            MicrophoneCaptureRequest(
                recordingId = "live-1",
                destination = destination,
                sampleRateHz = 16_000,
                channelCount = 1,
                bitsPerSample = 16,
            ),
        )
        microphoneSession.start()
        assertArrayEquals(byteArrayOf(1, 2), microphoneSession.frames.first().bytes)
        assertNull(microphoneSession.events.firstOrNull())
        microphoneSession.stop()
        assertEquals(CapturedAudio(durationMillis = 125, bytesWritten = 2), microphoneSession.finish())
        microphoneSession.close()

        val transcription = RecordingTranscriptionPort()
        val transcriptionSession = transcription.open(
            StreamingTranscriptionRequest(
                recordingId = "live-1",
                engine = StreamingEngineConfig.Online(
                    credential = credential,
                    profile = profile,
                ),
                language = "auto",
                enableItn = true,
            ),
        )
        transcriptionSession.start()
        transcriptionSession.feed(microphone.frame)
        transcriptionSession.flush()
        transcriptionSession.stop()
        transcriptionSession.close()
        assertEquals(listOf("start", "feed", "flush", "stop", "close"), transcription.calls)

        history.checkpointTranscript("live-1", emptyList())
        assertEquals(
            HistoryRecordingSummary(historyId = "live-1"),
            history.completeLiveDraft(
                CompleteLiveDraftRequest(
                    historyId = "live-1",
                    segments = emptyList(),
                    durationMillis = 125,
                ),
            ),
        )
        history.deleteDraft("live-2")

        assertEquals(1_725_000_000_000, WallClockPort { 1_725_000_000_000 }.nowEpochMillis())
        assertEquals(125, MonotonicClockPort { 125 }.elapsedRealtimeMillis())
        assertEquals("live-2", RecordingIdPort { "live-2" }.nextRecordingId())

        credentialSettings.clear()
        assertEquals(CredentialStatus.NOT_CONFIGURED, credentialSettings.status.first())
    }

    private class MemoryCredentialSettings : StreamingCredentialSettingsPort {
        private val mutableStatus = MutableStateFlow(CredentialStatus.NOT_CONFIGURED)
        override val status: Flow<CredentialStatus> = mutableStatus

        override suspend fun save(credential: StreamingCredential) {
            mutableStatus.value = CredentialStatus.CONFIGURED
        }

        override suspend fun clear() {
            mutableStatus.value = CredentialStatus.NOT_CONFIGURED
        }
    }

    private class MemoryHistoryPort(
        private val destination: RecordingDestination,
    ) : RecordingHistoryPort {
        override suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft =
            RecordingDraft(historyId = request.recordingId, destination = destination)

        override suspend fun checkpointTranscript(
            historyId: String,
            segments: List<TranscriptSegment>,
        ) = Unit

        override suspend fun completeLiveDraft(
            request: CompleteLiveDraftRequest,
        ): HistoryRecordingSummary = HistoryRecordingSummary(request.historyId)

        override suspend fun deleteDraft(historyId: String) = Unit
    }

    private class RecordingMicrophonePort : MicrophoneCapturePort {
        val frame = Pcm16Frame(byteArrayOf(1, 2))

        override suspend fun open(request: MicrophoneCaptureRequest): MicrophoneCaptureSession =
            object : MicrophoneCaptureSession {
                override val frames: Flow<Pcm16Frame> = flowOf(frame)
                override val events: Flow<MicrophoneCaptureEvent> = emptyFlow()

                override suspend fun start() = Unit

                override suspend fun stop() = Unit

                override suspend fun finish(): CapturedAudio = CapturedAudio(
                    durationMillis = 125,
                    bytesWritten = frame.bytes.size.toLong(),
                )

                override fun close() = Unit
            }
    }

    private class RecordingTranscriptionPort : StreamingTranscriptionPort {
        val calls = mutableListOf<String>()

        override suspend fun open(
            request: StreamingTranscriptionRequest,
        ): StreamingTranscriptionSession = object : StreamingTranscriptionSession {
            override val events: Flow<StreamingTranscriptionEvent> = emptyFlow()

            override suspend fun start() {
                calls += "start"
            }

            override suspend fun feed(frame: Pcm16Frame) {
                calls += "feed"
            }

            override suspend fun flush() {
                calls += "flush"
            }

            override suspend fun stop() {
                calls += "stop"
            }

            override fun close() {
                calls += "close"
            }
        }
    }
}
