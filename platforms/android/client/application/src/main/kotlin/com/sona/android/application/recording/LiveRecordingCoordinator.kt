package com.sona.android.application.recording

import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.delay
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

class LiveRecordingCoordinator(
    private val credentialRepository: StreamingCredentialRepository,
    private val providerCatalog: StreamingProviderCatalogPort,
    private val microphoneCapture: MicrophoneCapturePort,
    private val streamingTranscription: StreamingTranscriptionPort,
    private val history: RecordingHistoryPort,
    private val monotonicClock: MonotonicClockPort,
    private val recordingIds: RecordingIdPort,
    private val scope: CoroutineScope,
    private val checkpointIntervalMillis: Long = 2_000,
    private val elapsedUpdateIntervalMillis: Long = 1_000,
) {
    private val commandMutex = Mutex()
    private val mutableState = MutableStateFlow<LiveRecordingState>(LiveRecordingState.Idle)
    val state: StateFlow<LiveRecordingState> = mutableState.asStateFlow()

    private val ownerJob = scope.coroutineContext[Job]
    private val cleanupJob = SupervisorJob()
    private val cleanupScope = CoroutineScope(scope.coroutineContext.minusKey(Job) + cleanupJob)
    private var activeSession: ActiveSession? = null

    init {
        ownerJob?.invokeOnCompletion {
            cleanupScope.launch {
                stop()
                cleanupJob.cancel()
            }
        }
    }

    suspend fun start() {
        commandMutex.withLock {
            if (ownerJob?.isActive == false) {
                return
            }
            if (activeSession != null) {
                return
            }

            val credential = credentialRepository.load()
            if (credential == null || credential.apiKey.isBlank()) {
                mutableState.value = LiveRecordingState.NeedsConfiguration
                return
            }
            val profile = providerCatalog.loadVolcengineStreamingProfile()
            if (
                profile.providerId.isBlank() ||
                profile.profileId.isBlank() ||
                profile.streamingEndpoint.isBlank() ||
                profile.streamingResourceId.isBlank()
            ) {
                mutableState.value = LiveRecordingState.Failed(
                    RecordingFailure(
                        category = RecordingFailureCategory.INVALID_CONFIGURATION,
                        message = "Streaming provider configuration is incomplete.",
                    ),
                )
                return
            }
            val recordingId = recordingIds.nextRecordingId()
            mutableState.value = LiveRecordingState.Preparing(recordingId)

            var draft: RecordingDraft? = null
            var microphone: MicrophoneCaptureSession? = null
            var transcription: StreamingTranscriptionSession? = null
            try {
                draft = history.createLiveDraft(
                    CreateLiveDraftRequest(recordingId = recordingId, audioExtension = "wav"),
                )
                microphone = microphoneCapture.open(
                    MicrophoneCaptureRequest(
                        recordingId = recordingId,
                        destination = draft.destination,
                        sampleRateHz = 16_000,
                        channelCount = 1,
                        bitsPerSample = 16,
                    ),
                )
                transcription = streamingTranscription.open(
                    StreamingTranscriptionRequest(
                        recordingId = recordingId,
                        credential = credential,
                        profile = profile,
                        language = "auto",
                        enableItn = true,
                    ),
                )
                transcription.start()
            } catch (error: CancellationException) {
                rollbackStartup(draft, microphone, transcription)
                mutableState.value = LiveRecordingState.Idle
                throw error
            } catch (_: Exception) {
                rollbackStartup(draft, microphone, transcription)
                mutableState.value = LiveRecordingState.Failed(
                    RecordingFailure(
                        category = RecordingFailureCategory.STARTUP,
                        message = "Unable to start live transcription.",
                    ),
                )
                return
            }

            val openedDraft = checkNotNull(draft)
            val openedMicrophone = checkNotNull(microphone)
            val openedTranscription = checkNotNull(transcription)

            val session = ActiveSession(
                recordingId = recordingId,
                historyId = openedDraft.historyId,
                microphone = openedMicrophone,
                transcription = openedTranscription,
            )
            activeSession = session
            session.checkpointJob = scope.launch(start = CoroutineStart.UNDISPATCHED) {
                for (signal in session.checkpointSignals) {
                    val requiresCheckpoint = session.transcriptMutex.withLock {
                        session.transcriptVersion != session.lastCheckpointVersion
                    }
                    if (!requiresCheckpoint) {
                        continue
                    }
                    delay(checkpointIntervalMillis)
                    val snapshot = session.transcriptMutex.withLock {
                        TranscriptSnapshot(
                            version = session.transcriptVersion,
                            segments = session.segments,
                        )
                    }
                    try {
                        history.checkpointTranscript(session.historyId, snapshot.segments)
                        session.transcriptMutex.withLock {
                            session.lastCheckpointVersion = snapshot.version
                        }
                    } catch (_: Exception) {
                        session.checkpointSignals.trySend(Unit)
                    }
                }
            }
            session.transcriptJob = scope.launch(start = CoroutineStart.UNDISPATCHED) {
                try {
                    openedTranscription.events.collect { event ->
                        if (event is StreamingTranscriptionEvent.Transcript) {
                            session.publicationMutex.withLock {
                                val segments = session.transcriptMutex.withLock {
                                    session.segments =
                                        TranscriptReducer.apply(session.segments, event.update)
                                    session.transcriptVersion += 1
                                    session.segments
                                }
                                mutableState.update { current ->
                                    if (current is LiveRecordingState.Recording) {
                                        current.copy(segments = segments)
                                    } else {
                                        current
                                    }
                                }
                                session.checkpointSignals.trySend(Unit)
                            }
                        }
                    }
                } catch (error: CancellationException) {
                    throw error
                } catch (_: Exception) {
                    degradeStreaming(session, cancelEventCollector = false)
                }
            }
            session.frameJob = scope.launch(start = CoroutineStart.UNDISPATCHED) {
                try {
                    openedMicrophone.frames.collect { frame ->
                        val activeTranscription = session.transcription
                        if (activeTranscription != null) {
                            try {
                                activeTranscription.feed(frame)
                            } catch (error: CancellationException) {
                                throw error
                            } catch (_: Exception) {
                                degradeStreaming(session, cancelEventCollector = true)
                            }
                        }
                    }
                } catch (error: CancellationException) {
                    throw error
                } catch (_: Exception) {
                    recordCompletionWarning(
                        session,
                        RecordingFailure(
                            category = RecordingFailureCategory.AUDIO,
                            message =
                                "Recording saved after microphone capture stopped unexpectedly.",
                        ),
                    )
                    scope.launch { stop() }
                }
            }
            session.inputJob = scope.launch(start = CoroutineStart.UNDISPATCHED) {
                try {
                    openedMicrophone.inputEvents.collect { event ->
                        val inputStatus = when (event) {
                            AudioInputEvent.Active -> AudioInputStatus.Active
                            AudioInputEvent.Silenced -> AudioInputStatus.Silenced
                            is AudioInputEvent.DeviceChanged ->
                                AudioInputStatus.DeviceChanged(event.deviceName)
                        }
                        session.publicationMutex.withLock {
                            session.inputStatus = inputStatus
                            mutableState.update { current ->
                                if (current is LiveRecordingState.Recording) {
                                    current.copy(inputStatus = inputStatus)
                                } else {
                                    current
                                }
                            }
                        }
                    }
                } catch (error: CancellationException) {
                    throw error
                } catch (_: Exception) {
                    session.publicationMutex.withLock {
                        session.inputStatus = AudioInputStatus.MonitoringUnavailable
                        mutableState.update { current ->
                            if (current is LiveRecordingState.Recording) {
                                current.copy(inputStatus = session.inputStatus)
                            } else {
                                current
                            }
                        }
                    }
                }
            }

            try {
                openedMicrophone.start()
            } catch (error: CancellationException) {
                rollbackMicrophoneStart(session)
                mutableState.value = LiveRecordingState.Idle
                throw error
            } catch (_: Exception) {
                rollbackMicrophoneStart(session)
                mutableState.value = LiveRecordingState.Failed(
                    RecordingFailure(
                        category = RecordingFailureCategory.STARTUP,
                        message = "Unable to start microphone capture.",
                    ),
                )
                return
            }
            session.startedAtMillis = monotonicClock.elapsedRealtimeMillis()
            session.publicationMutex.withLock {
                mutableState.value = LiveRecordingState.Recording(
                    recordingId = recordingId,
                    elapsedMillis = 0,
                    segments = session.transcriptMutex.withLock { session.segments },
                    streamingStatus = session.streamingStatus,
                    inputStatus = session.inputStatus,
                )
            }
            session.elapsedJob = scope.launch {
                while (true) {
                    delay(elapsedUpdateIntervalMillis)
                    val elapsedMillis =
                        (monotonicClock.elapsedRealtimeMillis() - session.startedAtMillis)
                            .coerceAtLeast(0)
                    mutableState.update { current ->
                        if (current is LiveRecordingState.Recording) {
                            current.copy(elapsedMillis = elapsedMillis)
                        } else {
                            current
                        }
                    }
                }
            }
        }
    }

    suspend fun stop() {
        commandMutex.withLock {
            val session = activeSession ?: return
            mutableState.value = LiveRecordingState.Stopping(session.recordingId)

            withContext(NonCancellable) {
                session.elapsedJob?.cancelAndJoin()
                val microphoneStoppedCleanly = try {
                    session.microphone.stop()
                    true
                } catch (_: Exception) {
                    recordCompletionWarning(
                        session,
                        RecordingFailure(
                            category = RecordingFailureCategory.AUDIO,
                            message =
                                "Recording saved after microphone shutdown reported an error.",
                        ),
                    )
                    false
                }
                if (microphoneStoppedCleanly) {
                    session.frameJob.join()
                    session.inputJob.join()
                } else {
                    session.frameJob.cancelAndJoin()
                    session.inputJob.cancelAndJoin()
                }
                var persistenceFailure: RecordingFailure? = null
                val capturedAudio = try {
                    session.microphone.finish()
                } catch (_: Exception) {
                    persistenceFailure = RecordingFailure(
                        category = RecordingFailureCategory.PERSISTENCE,
                        message = "Recording could not be finalized; incomplete draft preserved.",
                    )
                    null
                }
                var transcriptionToClose: StreamingTranscriptionSession? = null
                var transcriptionStoppedCleanly = true
                session.transcriptionMutex.withLock {
                    val activeTranscription = session.transcription
                    if (activeTranscription != null) {
                        try {
                            activeTranscription.flush()
                        } catch (_: Exception) {
                            recordCompletionWarning(
                                session,
                                RecordingFailure(
                                    category = RecordingFailureCategory.STREAMING,
                                    message =
                                        "Recording saved, but the final transcript could not be confirmed.",
                                ),
                            )
                        }
                        try {
                            activeTranscription.stop()
                        } catch (_: Exception) {
                            transcriptionStoppedCleanly = false
                            recordCompletionWarning(
                                session,
                                RecordingFailure(
                                    category = RecordingFailureCategory.STREAMING,
                                    message =
                                        "Recording saved, but the final transcript could not be confirmed.",
                                ),
                            )
                        }
                        session.transcription = null
                        transcriptionToClose = activeTranscription
                    }
                }
                if (transcriptionStoppedCleanly) {
                    session.transcriptJob.join()
                } else {
                    try {
                        transcriptionToClose?.close()
                    } catch (_: Exception) {
                        // The collector is cancelled below even if close fails.
                    }
                    transcriptionToClose = null
                    session.transcriptJob.cancelAndJoin()
                }
                session.checkpointSignals.close()
                session.checkpointJob.cancelAndJoin()
                val segments = session.transcriptMutex.withLock { session.segments }
                val summary = if (capturedAudio != null) {
                    try {
                        history.completeLiveDraft(
                            CompleteLiveDraftRequest(
                                historyId = session.historyId,
                                segments = segments,
                                durationMillis = capturedAudio.durationMillis,
                            ),
                        )
                    } catch (_: Exception) {
                        persistenceFailure = RecordingFailure(
                            category = RecordingFailureCategory.PERSISTENCE,
                            message = "Recording was preserved as an incomplete draft.",
                        )
                        null
                    }
                } else {
                    null
                }
                try {
                    transcriptionToClose?.close()
                } catch (_: Exception) {
                    recordCompletionWarning(
                        session,
                        RecordingFailure(
                            category = RecordingFailureCategory.STREAMING,
                            message =
                                "Recording saved, but transcription resources did not close cleanly.",
                        ),
                    )
                }
                try {
                    session.microphone.close()
                } catch (_: Exception) {
                    recordCompletionWarning(
                        session,
                        RecordingFailure(
                            category = RecordingFailureCategory.AUDIO,
                            message =
                                "Recording saved, but microphone resources did not close cleanly.",
                        ),
                    )
                }
                val completionWarning = session.warningMutex.withLock {
                    session.completionWarning
                }
                activeSession = null
                mutableState.value = if (persistenceFailure != null) {
                    LiveRecordingState.Failed(checkNotNull(persistenceFailure))
                } else if (summary != null) {
                    LiveRecordingState.Completed(
                        historyId = summary.historyId,
                        warning = completionWarning,
                    )
                } else {
                    LiveRecordingState.Failed(
                        RecordingFailure(
                            category = RecordingFailureCategory.PERSISTENCE,
                            message = "Recording was preserved as an incomplete draft.",
                        ),
                    )
                }
            }
        }
    }

    private suspend fun recordCompletionWarning(
        session: ActiveSession,
        warning: RecordingFailure,
    ) {
        session.warningMutex.withLock {
            if (session.completionWarning == null) {
                session.completionWarning = warning
            }
        }
    }

    private suspend fun rollbackMicrophoneStart(session: ActiveSession) =
        withContext(NonCancellable) {
            session.elapsedJob?.cancelAndJoin()
            session.checkpointSignals.close()
            session.checkpointJob.cancelAndJoin()
            session.frameJob.cancelAndJoin()
            session.inputJob.cancelAndJoin()
            session.transcriptJob.cancelAndJoin()

            val transcription = session.transcriptionMutex.withLock {
                session.transcription.also { session.transcription = null }
            }
            if (transcription != null) {
                try {
                    transcription.stop()
                } catch (_: Exception) {
                    // Continue closing all resources acquired before capture.
                }
                try {
                    transcription.close()
                } catch (_: Exception) {
                    // Continue closing the microphone and deleting the draft.
                }
            }
            try {
                session.microphone.close()
            } catch (_: Exception) {
                // Empty draft cleanup is still required.
            }
            try {
                history.deleteDraft(session.historyId)
            } catch (_: Exception) {
                // Startup remains failed even if best-effort deletion fails.
            }
            activeSession = null
        }

    private suspend fun degradeStreaming(
        session: ActiveSession,
        cancelEventCollector: Boolean,
    ) {
        session.transcriptionMutex.withLock {
            val transcription = session.transcription ?: return
            try {
                transcription.stop()
            } catch (_: Exception) {
                // Closing the failed session remains mandatory.
            }
            try {
                transcription.close()
            } catch (_: Exception) {
                // Recording can continue because WAV ownership is independent.
            }
            session.transcription = null
        }
        session.publicationMutex.withLock {
            session.streamingStatus = StreamingStatus.AudioOnly(
                RecordingFailure(
                    category = RecordingFailureCategory.STREAMING,
                    message = "Live transcription stopped; audio recording continues.",
                ),
            )
            mutableState.update { current ->
                if (current is LiveRecordingState.Recording) {
                    current.copy(streamingStatus = session.streamingStatus)
                } else {
                    current
                }
            }
        }
        if (cancelEventCollector) {
            session.transcriptJob.cancelAndJoin()
        }
    }

    private suspend fun rollbackStartup(
        draft: RecordingDraft?,
        microphone: MicrophoneCaptureSession?,
        transcription: StreamingTranscriptionSession?,
    ) = withContext(NonCancellable) {
        try {
            transcription?.close()
        } catch (_: Exception) {
            // Continue releasing the remaining acquired resources.
        }
        try {
            microphone?.close()
        } catch (_: Exception) {
            // Continue deleting an empty managed draft.
        }
        if (draft != null) {
            try {
                history.deleteDraft(draft.historyId)
            } catch (_: Exception) {
                // Startup still fails even if best-effort draft cleanup fails.
            }
        }
    }

    private class ActiveSession(
        val recordingId: String,
        val historyId: String,
        val microphone: MicrophoneCaptureSession,
        var transcription: StreamingTranscriptionSession?,
    ) {
        val transcriptionMutex = Mutex()
        val transcriptMutex = Mutex()
        val warningMutex = Mutex()
        val publicationMutex = Mutex()
        val checkpointSignals = Channel<Unit>(Channel.CONFLATED)
        var segments: List<TranscriptSegment> = emptyList()
        var transcriptVersion: Long = 0
        var lastCheckpointVersion: Long = 0
        var completionWarning: RecordingFailure? = null
        var streamingStatus: StreamingStatus = StreamingStatus.Connected
        var inputStatus: AudioInputStatus = AudioInputStatus.Active
        var startedAtMillis: Long = 0
        var elapsedJob: Job? = null
        lateinit var checkpointJob: Job
        lateinit var transcriptJob: Job
        lateinit var frameJob: Job
        lateinit var inputJob: Job
    }

    private data class TranscriptSnapshot(
        val version: Long,
        val segments: List<TranscriptSegment>,
    )
}
