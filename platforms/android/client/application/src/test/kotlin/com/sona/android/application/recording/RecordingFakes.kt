package com.sona.android.application.recording

import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.onCompletion
import kotlinx.coroutines.flow.onEach
import kotlinx.coroutines.flow.receiveAsFlow

class RecordingFakes {
    val calls = mutableListOf<String>()
    val credentialResolver = FakeCredentialResolver(calls)
    val providerCatalog = FakeProviderCatalog(calls)
    val history = FakeRecordingHistory(calls)
    val microphone = FakeMicrophoneCapture(calls)
    val transcription = FakeStreamingTranscription(calls)
    val monotonicClock = FakeMonotonicClock()
    val recordingIds = FakeRecordingIds(calls)

    class FakeCredentialResolver(
        private val calls: MutableList<String>,
    ) : StreamingCredentialResolverPort {
        var credential: StreamingCredential? = StreamingCredential("secret")
        var loadFailure: Throwable? = null
        var loadBarrier: Pair<CompletableDeferred<Unit>, CompletableDeferred<Unit>>? = null

        override suspend fun loadForStart(): StreamingCredential? {
            calls += "credential.loadForStart"
            loadBarrier?.let { (started, release) ->
                started.complete(Unit)
                release.await()
            }
            loadFailure?.let { throw it }
            return credential
        }
    }

    class FakeProviderCatalog(
        private val calls: MutableList<String>,
    ) : StreamingProviderCatalogPort {
        var failure: Throwable? = null
        var profile = StreamingProviderProfile(
            providerId = "volcengine-doubao",
            profileId = "volcengine-doubao-default",
            streamingEndpoint = "wss://example.test/stream",
            streamingResourceId = "resource-id",
        )

        override suspend fun loadVolcengineStreamingProfile(): StreamingProviderProfile {
            calls += "provider.load"
            failure?.let { throw it }
            return profile
        }
    }

    class FakeRecordingHistory(
        private val calls: MutableList<String>,
    ) : RecordingHistoryPort {
        var completedRequest: CompleteLiveDraftRequest? = null
        val checkpointRequests = mutableListOf<List<TranscriptSegment>>()
        var checkpointFailuresRemaining: Int = 0
        var completeFailure: Throwable? = null
        var createFailure: Throwable? = null
        var createBarrier: Pair<CompletableDeferred<Unit>, CompletableDeferred<Unit>>? = null
        var checkpointBarrier: Pair<CompletableDeferred<Unit>, CompletableDeferred<Unit>>? = null

        override suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft {
            calls += "history.create"
            createBarrier?.let { (started, release) ->
                started.complete(Unit)
                release.await()
            }
            createFailure?.let { throw it }
            return RecordingDraft(
                historyId = request.recordingId,
                destination = RecordingDestination("${request.recordingId}.wav"),
            )
        }

        override suspend fun checkpointTranscript(
            historyId: String,
            segments: List<TranscriptSegment>,
        ) {
            calls += "history.checkpoint"
            checkpointBarrier?.let { (started, release) ->
                started.complete(Unit)
                release.await()
            }
            checkpointRequests += segments
            if (checkpointFailuresRemaining > 0) {
                checkpointFailuresRemaining -= 1
                throw IllegalStateException("checkpoint detail")
            }
        }

        override suspend fun completeLiveDraft(
            request: CompleteLiveDraftRequest,
        ): HistoryRecordingSummary {
            calls += "history.complete"
            completedRequest = request
            completeFailure?.let { throw it }
            return HistoryRecordingSummary(request.historyId)
        }

        override suspend fun deleteDraft(historyId: String) {
            calls += "history.delete"
        }
    }

    class FakeMicrophoneCapture(
        private val calls: MutableList<String>,
    ) : MicrophoneCapturePort {
        private val frameChannel = Channel<Pcm16Frame>(Channel.UNLIMITED)
        private val eventChannel = Channel<MicrophoneCaptureEvent>(Channel.UNLIMITED)
        var capturedAudio = CapturedAudio(durationMillis = 125, bytesWritten = 2)
        var startFailure: Throwable? = null
        var onStart: (() -> Unit)? = null
        var stopBarrier: Pair<CompletableDeferred<Unit>, CompletableDeferred<Unit>>? = null
        var captureEventBarrier: Pair<CompletableDeferred<Unit>, CompletableDeferred<Unit>>? = null
        var stopAttemptFinished: CompletableDeferred<Unit>? = null
        var stopFailure: Throwable? = null
        var frameFailureOnStop: Throwable? = null
        var finishFailure: Throwable? = null
        var closeFailure: Throwable? = null

        override suspend fun open(request: MicrophoneCaptureRequest): MicrophoneCaptureSession {
            calls += "microphone.open"
            return object : MicrophoneCaptureSession {
                override val frames: Flow<Pcm16Frame> = frameChannel.receiveAsFlow()
                override val events: Flow<MicrophoneCaptureEvent> =
                    eventChannel.receiveAsFlow().onEach {
                        captureEventBarrier?.let { (started, release) ->
                            started.complete(Unit)
                            release.await()
                        }
                    }

                override suspend fun start() {
                    calls += "microphone.start"
                    onStart?.invoke()
                    startFailure?.let { throw it }
                }

                override suspend fun stop() {
                    calls += "microphone.stop"
                    try {
                        stopBarrier?.let { (started, release) ->
                            started.complete(Unit)
                            release.await()
                        }
                        val frameFailure = frameFailureOnStop
                        if (frameFailure == null) {
                            frameChannel.close()
                        } else {
                            frameChannel.close(frameFailure)
                        }
                        eventChannel.close()
                        stopFailure?.let { throw it }
                    } finally {
                        stopAttemptFinished?.complete(Unit)
                    }
                }

                override suspend fun finish(): CapturedAudio {
                    calls += "microphone.finish"
                    finishFailure?.let { throw it }
                    return capturedAudio
                }

                override fun close() {
                    calls += "microphone.close"
                    closeFailure?.let { throw it }
                }
            }
        }

        suspend fun emitFrame(frame: Pcm16Frame) {
            frameChannel.send(frame)
        }

        suspend fun emitInput(event: AudioInputEvent) {
            emitCaptureEvent(MicrophoneCaptureEvent.Input(event))
        }

        suspend fun emitCaptureEvent(event: MicrophoneCaptureEvent) {
            eventChannel.send(event)
        }

        fun failFrames(error: Throwable) {
            frameChannel.close(error)
        }

        fun failEvents(error: Throwable) {
            eventChannel.close(error)
        }
    }

    class FakeStreamingTranscription(
        private val calls: MutableList<String>,
    ) : StreamingTranscriptionPort {
        private val eventChannel = Channel<StreamingTranscriptionEvent>(Channel.UNLIMITED)
        val fedFrames = mutableListOf<Pcm16Frame>()
        var startFailure: Throwable? = null
        var successfulFeedsBeforeFailure: Int? = null
        var flushFailure: Throwable? = null
        var stopFailure: Throwable? = null
        var closeFailure: Throwable? = null
        var eventCollectorCompletions: Int = 0
        var feedBarrier: Pair<CompletableDeferred<Unit>, CompletableDeferred<Unit>>? = null
        var closeObservedActiveFeed: Boolean = false
        var eventOnStop: StreamingTranscriptionEvent? = null
        private var activeFeeds: Int = 0

        override suspend fun open(
            request: StreamingTranscriptionRequest,
        ): StreamingTranscriptionSession {
            calls += "asr.open"
            return object : StreamingTranscriptionSession {
                override val events: Flow<StreamingTranscriptionEvent> =
                    eventChannel.receiveAsFlow().onCompletion {
                        eventCollectorCompletions += 1
                    }

                override suspend fun start() {
                    calls += "asr.start"
                    startFailure?.let { throw it }
                }

                override suspend fun feed(frame: Pcm16Frame) {
                    calls += "asr.feed"
                    activeFeeds += 1
                    try {
                        feedBarrier?.let { (started, release) ->
                            started.complete(Unit)
                            release.await()
                        }
                        if (
                            successfulFeedsBeforeFailure != null &&
                            fedFrames.size >= checkNotNull(successfulFeedsBeforeFailure)
                        ) {
                            throw IllegalStateException("provider feed detail")
                        }
                        fedFrames += frame
                    } finally {
                        activeFeeds -= 1
                    }
                }

                override suspend fun flush() {
                    calls += "asr.flush"
                    flushFailure?.let { throw it }
                }

                override suspend fun stop() {
                    calls += "asr.stop"
                    try {
                        eventOnStop?.let { eventChannel.send(it) }
                        stopFailure?.let { throw it }
                    } finally {
                        eventChannel.close()
                    }
                }

                override fun close() {
                    calls += "asr.close"
                    closeObservedActiveFeed = closeObservedActiveFeed || activeFeeds > 0
                    eventChannel.close()
                    closeFailure?.let { throw it }
                }
            }
        }

        suspend fun emit(event: StreamingTranscriptionEvent) {
            eventChannel.send(event)
        }

        fun failEvents(error: Throwable) {
            eventChannel.close(error)
        }
    }

    class FakeMonotonicClock : MonotonicClockPort {
        var nowMillis: Long = 1_000

        override fun elapsedRealtimeMillis(): Long = nowMillis
    }

    class FakeRecordingIds(
        private val calls: MutableList<String>,
    ) : RecordingIdPort {
        var failure: Throwable? = null

        override fun nextRecordingId(): String {
            calls += "id.next"
            failure?.let { throw it }
            return "live-1"
        }
    }
}
