package com.sona.android.application.recording

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class LiveRecordingCoordinatorTest {
    @Test
    fun `transcript buffered before microphone start appears in initial recording state`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.emit(transcriptEvent(id = "early", text = "already here"))
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        val recording = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(listOf("already here"), recording.segments.map(TranscriptSegment::text))
        coordinator.stop()
    }

    @Test
    fun `input failure before microphone start appears in initial recording state`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.failInputEvents(IllegalStateException("early input detail"))
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        val recording = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(AudioInputStatus.MonitoringUnavailable, recording.inputStatus)
        coordinator.stop()
    }

    @Test
    fun `feed failure before microphone start appears as audio only in initial state`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.successfulFeedsBeforeFailure = 0
        fakes.microphone.emitFrame(Pcm16Frame(byteArrayOf(1, 2)))
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        val recording = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(
            StreamingStatus.AudioOnly(
                RecordingFailure(
                    category = RecordingFailureCategory.STREAMING,
                    message = "Live transcription stopped; audio recording continues.",
                ),
            ),
            recording.streamingStatus,
        )
        coordinator.stop()
    }

    @Test
    fun `owner scope cancellation saves and closes an active recording`() = runTest {
        val fakes = RecordingFakes()
        val ownerJob = SupervisorJob()
        val ownerScope = CoroutineScope(ownerJob + StandardTestDispatcher(testScheduler))
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = ownerScope,
        )
        coordinator.start()

        ownerJob.cancel()
        runCurrent()

        assertEquals(LiveRecordingState.Completed("live-1"), coordinator.state.value)
        assertEquals(1, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `microphone frame stream failure automatically saves with an audio warning`() = runTest {
        val fakes = RecordingFakes()
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        fakes.microphone.failFrames(IllegalStateException("audio read detail"))
        runCurrent()

        assertEquals(
            LiveRecordingState.Completed(
                historyId = "live-1",
                warning = RecordingFailure(
                    category = RecordingFailureCategory.AUDIO,
                    message = "Recording saved after microphone capture stopped unexpectedly.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `frame failure during shutdown is included in the completion warning`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.frameFailureOnStop = IllegalStateException("shutdown read detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            LiveRecordingState.Completed(
                historyId = "live-1",
                warning = RecordingFailure(
                    category = RecordingFailureCategory.AUDIO,
                    message = "Recording saved after microphone capture stopped unexpectedly.",
                ),
            ),
            coordinator.state.value,
        )
    }

    @Test
    fun `microphone stop failure does not replace an earlier frame warning`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.stopFailure = IllegalStateException("stop detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        fakes.microphone.failFrames(IllegalStateException("read detail"))
        runCurrent()

        assertEquals(
            RecordingFailure(
                category = RecordingFailureCategory.AUDIO,
                message = "Recording saved after microphone capture stopped unexpectedly.",
            ),
            (coordinator.state.value as LiveRecordingState.Completed).warning,
        )
    }

    @Test
    fun `frame warning wins when capture fails while microphone stop is suspended`() = runTest {
        val fakes = RecordingFakes()
        val stopStarted = CompletableDeferred<Unit>()
        val releaseStop = CompletableDeferred<Unit>()
        fakes.microphone.stopBarrier = stopStarted to releaseStop
        fakes.microphone.stopFailure = IllegalStateException("stop detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        val stopJob = launch { coordinator.stop() }
        stopStarted.await()
        fakes.microphone.failFrames(IllegalStateException("read detail"))
        runCurrent()
        releaseStop.complete(Unit)
        stopJob.join()

        assertEquals(
            RecordingFailure(
                category = RecordingFailureCategory.AUDIO,
                message = "Recording saved after microphone capture stopped unexpectedly.",
            ),
            (coordinator.state.value as LiveRecordingState.Completed).warning,
        )
    }

    @Test
    fun `microphone close failure does not replace an earlier streaming warning`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.flushFailure = IllegalStateException("flush detail")
        fakes.microphone.closeFailure = IllegalStateException("close detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            RecordingFailure(
                category = RecordingFailureCategory.STREAMING,
                message = "Recording saved, but the final transcript could not be confirmed.",
            ),
            (coordinator.state.value as LiveRecordingState.Completed).warning,
        )
    }

    @Test
    fun `microphone close failure clears the active session and reports a saved warning`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.closeFailure = IllegalStateException("audio close detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()
        coordinator.stop()

        assertEquals(
            LiveRecordingState.Completed(
                historyId = "live-1",
                warning = RecordingFailure(
                    category = RecordingFailureCategory.AUDIO,
                    message = "Recording saved, but microphone resources did not close cleanly.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `microphone stop failure cancels capture collectors and still saves audio`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.stopFailure = IllegalStateException("audio stop detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            LiveRecordingState.Completed(
                historyId = "live-1",
                warning = RecordingFailure(
                    category = RecordingFailureCategory.AUDIO,
                    message = "Recording saved after microphone shutdown reported an error.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "microphone.finish" })
        assertEquals(1, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `ASR event stream failure degrades recording without waiting for another frame`() = runTest {
        val fakes = RecordingFakes()
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        fakes.transcription.failEvents(IllegalStateException("observer transport detail"))
        runCurrent()

        val recording = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(
            StreamingStatus.AudioOnly(
                RecordingFailure(
                    category = RecordingFailureCategory.STREAMING,
                    message = "Live transcription stopped; audio recording continues.",
                ),
            ),
            recording.streamingStatus,
        )
        coordinator.stop()
        assertEquals(1, fakes.calls.count { it == "asr.stop" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
    }

    @Test
    fun `startup cancellation propagates instead of becoming a business failure`() = runTest {
        val fakes = RecordingFakes()
        fakes.history.createFailure = CancellationException("cancel start")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        var cancellation: CancellationException? = null
        try {
            coordinator.start()
        } catch (error: CancellationException) {
            cancellation = error
        }

        assertEquals("cancel start", cancellation?.message)
        assertEquals(LiveRecordingState.Idle, coordinator.state.value)
        assertEquals(
            listOf("credential.load", "provider.load", "id.next", "history.create"),
            fakes.calls,
        )
    }

    @Test
    fun `microphone start cancellation rolls back and restores idle`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.startFailure = CancellationException("cancel microphone start")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        var cancellation: CancellationException? = null
        try {
            coordinator.start()
        } catch (error: CancellationException) {
            cancellation = error
        }

        assertEquals("cancel microphone start", cancellation?.message)
        assertEquals(LiveRecordingState.Idle, coordinator.state.value)
        assertEquals(1, fakes.calls.count { it == "history.delete" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `WAV finalization failure preserves an incomplete draft and closes all resources`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.finishFailure = IllegalStateException("storage path detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            LiveRecordingState.Failed(
                RecordingFailure(
                    category = RecordingFailureCategory.PERSISTENCE,
                    message = "Recording could not be finalized; incomplete draft preserved.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(0, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "asr.flush" })
        assertEquals(1, fakes.calls.count { it == "asr.stop" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
        assertEquals(0, fakes.calls.count { it == "history.delete" })
    }

    @Test
    fun `ASR stop failure closes its event stream and still persists history`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.stopFailure = IllegalStateException("provider stop detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            LiveRecordingState.Completed(
                historyId = "live-1",
                warning = RecordingFailure(
                    category = RecordingFailureCategory.STREAMING,
                    message = "Recording saved, but the final transcript could not be confirmed.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `flush failure still persists audio and completes with a transcript warning`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.flushFailure = IllegalStateException("provider flush detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            LiveRecordingState.Completed(
                historyId = "live-1",
                warning = RecordingFailure(
                    category = RecordingFailureCategory.STREAMING,
                    message = "Recording saved, but the final transcript could not be confirmed.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "history.complete" })
        assertEquals(1, fakes.calls.count { it == "asr.stop" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `microphone start failure cancels collectors and rolls back the started ASR`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.startFailure = IllegalStateException("audio device detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        assertEquals(
            LiveRecordingState.Failed(
                RecordingFailure(
                    category = RecordingFailureCategory.STARTUP,
                    message = "Unable to start microphone capture.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "asr.stop" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
        assertEquals(1, fakes.calls.count { it == "history.delete" })
        assertEquals(0, fakes.calls.count { it == "microphone.stop" })
    }

    @Test
    fun `history duration comes from captured audio rather than UI elapsed time`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.capturedAudio = CapturedAudio(durationMillis = 250, bytesWritten = 8_000)
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()
        fakes.monotonicClock.nowMillis = 1_900

        coordinator.stop()

        assertEquals(250L, fakes.history.completedRequest?.durationMillis)
    }

    @Test
    fun `recording elapsed time follows the monotonic clock`() = runTest {
        val fakes = RecordingFakes()
        fakes.microphone.onStart = {
            fakes.monotonicClock.nowMillis = 5_000
        }
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        fakes.monotonicClock.nowMillis = 6_250
        advanceTimeBy(1_000)
        runCurrent()

        val recording = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(1_250L, recording.elapsedMillis)
        coordinator.stop()
    }

    @Test
    fun `later transcript update retries a failed checkpoint with newest segments`() = runTest {
        val fakes = RecordingFakes()
        fakes.history.checkpointFailuresRemaining = 1
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
            checkpointIntervalMillis = 2_000,
        )
        coordinator.start()
        fakes.transcription.emit(transcriptEvent(id = "same", text = "first"))
        runCurrent()
        advanceTimeBy(2_000)
        runCurrent()

        assertEquals(1, fakes.history.checkpointRequests.size)
        assertTrue(coordinator.state.value is LiveRecordingState.Recording)

        fakes.transcription.emit(transcriptEvent(id = "same", text = "recovered"))
        runCurrent()
        advanceTimeBy(2_000)
        runCurrent()

        assertEquals(2, fakes.history.checkpointRequests.size)
        assertEquals(
            listOf("recovered"),
            fakes.history.checkpointRequests.last().map(TranscriptSegment::text),
        )
        coordinator.stop()
    }

    @Test
    fun `failed checkpoint retries on the next interval without a new transcript`() = runTest {
        val fakes = RecordingFakes()
        fakes.history.checkpointFailuresRemaining = 1
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
            checkpointIntervalMillis = 2_000,
        )
        coordinator.start()
        fakes.transcription.emit(transcriptEvent(id = "retry", text = "preserve me"))
        runCurrent()

        advanceTimeBy(2_000)
        runCurrent()
        assertEquals(1, fakes.history.checkpointRequests.size)

        advanceTimeBy(1_999)
        runCurrent()
        assertEquals(1, fakes.history.checkpointRequests.size)

        advanceTimeBy(1)
        runCurrent()
        assertEquals(2, fakes.history.checkpointRequests.size)
        assertEquals(
            listOf("preserve me"),
            fakes.history.checkpointRequests.last().map(TranscriptSegment::text),
        )
        coordinator.stop()
    }

    @Test
    fun `input silencing changes status without fabricating transcript text`() = runTest {
        val fakes = RecordingFakes()
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        fakes.microphone.emitInput(AudioInputEvent.Silenced)
        runCurrent()

        val recording = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(AudioInputStatus.Silenced, recording.inputStatus)
        assertTrue(recording.segments.isEmpty())
        coordinator.stop()
    }

    @Test
    fun `input status stream failure marks monitoring unavailable while capture continues`() =
        runTest {
            val fakes = RecordingFakes()
            val coordinator = LiveRecordingCoordinator(
                credentialRepository = fakes.credentialRepository,
                providerCatalog = fakes.providerCatalog,
                microphoneCapture = fakes.microphone,
                streamingTranscription = fakes.transcription,
                history = fakes.history,
                monotonicClock = fakes.monotonicClock,
                recordingIds = fakes.recordingIds,
                scope = backgroundScope,
            )
            coordinator.start()

            fakes.microphone.failInputEvents(IllegalStateException("device callback detail"))
            runCurrent()

            val recording = coordinator.state.value as LiveRecordingState.Recording
            assertEquals(
                AudioInputStatus.MonitoringUnavailable,
                recording.inputStatus,
            )
            assertEquals(0, fakes.calls.count { it == "microphone.stop" })

            coordinator.stop()
            assertEquals(1, fakes.calls.count { it == "history.complete" })
        }

    @Test
    fun `repeated start and stop commands do not duplicate session resources`() = runTest {
        val fakes = RecordingFakes()
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()
        coordinator.start()
        coordinator.stop()
        coordinator.stop()

        for (call in listOf("history.create", "microphone.open", "asr.open", "history.complete")) {
            assertEquals(1, fakes.calls.count { it == call })
        }
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
    }

    @Test
    fun `history completion failure still closes resources and preserves the draft`() = runTest {
        val fakes = RecordingFakes()
        fakes.history.completeFailure = IllegalStateException("database path detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        coordinator.stop()

        assertEquals(
            LiveRecordingState.Failed(
                RecordingFailure(
                    category = RecordingFailureCategory.PERSISTENCE,
                    message = "Recording was preserved as an incomplete draft.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(1, fakes.calls.count { it == "microphone.close" })
        assertEquals(0, fakes.calls.count { it == "history.delete" })
    }

    @Test
    fun `blank stored credential is treated as not configured`() = runTest {
        val fakes = RecordingFakes()
        fakes.credentialRepository.credential = StreamingCredential("   ")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        assertEquals(LiveRecordingState.NeedsConfiguration, coordinator.state.value)
        assertEquals(listOf("credential.load"), fakes.calls)
    }

    @Test
    fun `transcript checkpoints are throttled and use the newest snapshot`() = runTest {
        val fakes = RecordingFakes()
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
            checkpointIntervalMillis = 2_000,
        )
        coordinator.start()
        fakes.transcription.emit(transcriptEvent(id = "same", text = "first"))
        runCurrent()

        advanceTimeBy(1_999)
        runCurrent()
        assertTrue(fakes.history.checkpointRequests.isEmpty())

        fakes.transcription.emit(transcriptEvent(id = "same", text = "newest"))
        runCurrent()
        advanceTimeBy(1)
        runCurrent()

        assertEquals(1, fakes.history.checkpointRequests.size)
        assertEquals(
            listOf("newest"),
            fakes.history.checkpointRequests.single().map(TranscriptSegment::text),
        )
        coordinator.stop()
    }

    @Test
    fun `streaming feed failure degrades to audio only and still completes history`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.successfulFeedsBeforeFailure = 1
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()
        fakes.transcription.emit(
            StreamingTranscriptionEvent.Transcript(
                TranscriptUpdate(
                    removeIds = emptyList(),
                    upsertSegments = listOf(segment("partial", "partial transcript")),
                ),
            ),
        )
        fakes.microphone.emitFrame(Pcm16Frame(byteArrayOf(1, 2)))
        runCurrent()

        fakes.microphone.emitFrame(Pcm16Frame(byteArrayOf(3, 4)))
        runCurrent()

        val degraded = coordinator.state.value as LiveRecordingState.Recording
        assertEquals(
            StreamingStatus.AudioOnly(
                RecordingFailure(
                    category = RecordingFailureCategory.STREAMING,
                    message = "Live transcription stopped; audio recording continues.",
                ),
            ),
            degraded.streamingStatus,
        )
        fakes.microphone.emitFrame(Pcm16Frame(byteArrayOf(5, 6)))
        runCurrent()
        assertEquals(1, fakes.transcription.fedFrames.size)

        coordinator.stop()

        assertEquals(
            listOf("partial transcript"),
            fakes.history.completedRequest?.segments?.map(TranscriptSegment::text),
        )
        assertEquals(1, fakes.calls.count { it == "asr.stop" })
        assertEquals(1, fakes.calls.count { it == "asr.close" })
        assertEquals(LiveRecordingState.Completed("live-1"), coordinator.state.value)
    }

    @Test
    fun `feed degradation cancels the event collector when ASR cleanup fails`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.successfulFeedsBeforeFailure = 0
        fakes.transcription.stopFailure = IllegalStateException("provider stop detail")
        fakes.transcription.closeFailure = IllegalStateException("provider close detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )
        coordinator.start()

        fakes.microphone.emitFrame(Pcm16Frame(byteArrayOf(1, 2)))
        runCurrent()

        assertEquals(1, fakes.transcription.eventCollectorCompletions)
        coordinator.stop()
        assertEquals(LiveRecordingState.Completed("live-1"), coordinator.state.value)
    }

    @Test
    fun `ASR startup failure rolls back opened resources without starting capture`() = runTest {
        val fakes = RecordingFakes()
        fakes.transcription.startFailure = IllegalStateException("secret provider detail")
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        assertEquals(
            LiveRecordingState.Failed(
                RecordingFailure(
                    category = RecordingFailureCategory.STARTUP,
                    message = "Unable to start live transcription.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(
            listOf(
                "credential.load",
                "provider.load",
                "id.next",
                "history.create",
                "microphone.open",
                "asr.open",
                "asr.start",
                "asr.close",
                "microphone.close",
                "history.delete",
            ),
            fakes.calls,
        )
    }

    @Test
    fun `invalid provider configuration fails before allocating a recording`() = runTest {
        val fakes = RecordingFakes()
        fakes.providerCatalog.profile = fakes.providerCatalog.profile.copy(
            streamingEndpoint = "   ",
        )
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        assertEquals(
            LiveRecordingState.Failed(
                RecordingFailure(
                    category = RecordingFailureCategory.INVALID_CONFIGURATION,
                    message = "Streaming provider configuration is incomplete.",
                ),
            ),
            coordinator.state.value,
        )
        assertEquals(listOf("credential.load", "provider.load"), fakes.calls)
    }

    @Test
    fun `missing credential stops before creating recording resources`() = runTest {
        val fakes = RecordingFakes()
        fakes.credentialRepository.credential = null
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()

        assertEquals(LiveRecordingState.NeedsConfiguration, coordinator.state.value)
        assertEquals(listOf("credential.load"), fakes.calls)
    }

    @Test
    fun `starts streams and completes a recording in application order`() = runTest {
        val fakes = RecordingFakes()
        val coordinator = LiveRecordingCoordinator(
            credentialRepository = fakes.credentialRepository,
            providerCatalog = fakes.providerCatalog,
            microphoneCapture = fakes.microphone,
            streamingTranscription = fakes.transcription,
            history = fakes.history,
            monotonicClock = fakes.monotonicClock,
            recordingIds = fakes.recordingIds,
            scope = backgroundScope,
        )

        coordinator.start()
        assertTrue(coordinator.state.value is LiveRecordingState.Recording)

        val frame = Pcm16Frame(byteArrayOf(1, 2))
        fakes.microphone.emitFrame(frame)
        fakes.transcription.emit(
            StreamingTranscriptionEvent.Transcript(
                TranscriptUpdate(
                    removeIds = emptyList(),
                    upsertSegments = listOf(segment("segment-1", "hello")),
                ),
            ),
        )
        runCurrent()

        fakes.monotonicClock.nowMillis = 1_125
        coordinator.stop()
        advanceUntilIdle()

        assertArrayEquals(frame.bytes, fakes.transcription.fedFrames.single().bytes)
        assertEquals(listOf("hello"), fakes.history.completedRequest?.segments?.map { it.text })
        assertEquals(125L, fakes.history.completedRequest?.durationMillis)
        assertEquals(LiveRecordingState.Completed("live-1"), coordinator.state.value)
        assertEquals(
            listOf(
                "credential.load",
                "provider.load",
                "id.next",
                "history.create",
                "microphone.open",
                "asr.open",
                "asr.start",
                "microphone.start",
                "microphone.stop",
                "microphone.finish",
                "asr.flush",
                "asr.stop",
                "history.complete",
                "asr.close",
                "microphone.close",
            ),
            fakes.calls.filterNot { it == "asr.feed" },
        )
    }

    private fun segment(id: String, text: String): TranscriptSegment = TranscriptSegment(
        id = id,
        text = text,
        startSeconds = 0.0,
        endSeconds = 1.0,
        isFinal = true,
    )

    private fun transcriptEvent(
        id: String,
        text: String,
    ): StreamingTranscriptionEvent = StreamingTranscriptionEvent.Transcript(
        TranscriptUpdate(
            removeIds = emptyList(),
            upsertSegments = listOf(segment(id, text)),
        ),
    )
}
