package com.sona.android.application.recording

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AudioImportTest {
    @Test
    fun `schedule snapshots selected local model`() = runTest {
        val model = localModel("model-a")
        val jobs = FakeJobs()
        val outcome = ScheduleAudioImport(
            recognitionSettings = FakeRecognitionSettings(
                RecognitionSettings(
                    batchSelection = AsrModelSelection.Local(model.id),
                    installedModels = listOf(model),
                ),
            ),
            batchCredentials = FakeBatchSettings(),
            recordingIds = RecordingIdPort { "import-1" },
            jobs = jobs,
        )(AudioImportSource("content://audio/one"))

        assertEquals(ScheduleAudioImportOutcome.Scheduled("import-1"), outcome)
        assertEquals(
            AudioImportJob(
                id = "import-1",
                target = AudioImportTarget.NewImport(AudioImportSource("content://audio/one")),
                engine = AudioImportEngine.Local("model-a"),
            ),
            jobs.enqueued.single(),
        )
    }

    @Test
    fun `online import uploads original and stores normalized wav`() = runTest {
        val fixture = Fixture()
        fixture.onlineResult = listOf(segment("cloud"))

        val outcome = fixture.run(
            job = newJob(AudioImportEngine.Online(OnlineAsrProvider.GROQ_WHISPER)),
            allowWarning = false,
        )

        assertEquals(RunAudioImportOutcome.Completed("import-1", false), outcome)
        assertEquals("/jobs/source.m4a", fixture.onlineRequests.single().audioPath)
        assertEquals("/jobs/normalized.wav", fixture.history.saved.single().normalizedWavPath)
        assertEquals(listOf(segment("cloud")), fixture.history.saved.single().segments)
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `local import sends normalized wav through batch transcription`() = runTest {
        val fixture = Fixture()
        fixture.localResult = listOf(segment("local"))

        val outcome = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = false,
        )

        assertEquals(RunAudioImportOutcome.Completed("import-1", false), outcome)
        assertEquals("/jobs/normalized.wav", fixture.localRequests.single().audioPath)
        assertEquals(listOf(segment("local")), fixture.history.saved.single().segments)
    }

    @Test
    fun `final transcription failure saves audio with warning`() = runTest {
        val fixture = Fixture()
        fixture.onlineFailure = IllegalStateException("provider detail")

        val outcome = fixture.run(
            job = newJob(AudioImportEngine.Online(OnlineAsrProvider.VOLCENGINE_DOUBAO)),
            allowWarning = true,
        )

        assertEquals(RunAudioImportOutcome.Completed("import-1", true), outcome)
        assertTrue(fixture.history.saved.single().segments.isEmpty())
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `local batch failure retries before saving warning`() = runTest {
        val fixture = Fixture()
        fixture.localFailure = IllegalStateException("runtime")

        val retry = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = false,
        )
        val exhausted = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = true,
        )

        assertEquals(
            RunAudioImportOutcome.RetryableFailure(AudioImportFailure.TRANSCRIPTION),
            retry,
        )
        assertEquals(RunAudioImportOutcome.Completed("import-1", true), exhausted)
        assertTrue(fixture.history.saved.single().segments.isEmpty())
    }

    @Test
    fun `existing deterministic history completes without repeating import`() = runTest {
        val fixture = Fixture()
        fixture.history.existing = true

        val outcome = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = false,
        )

        assertEquals(RunAudioImportOutcome.Completed("import-1", false), outcome)
        assertTrue(fixture.history.saved.isEmpty())
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `terminal transcode failure cleans task files`() = runTest {
        val fixture = Fixture()
        fixture.transcoder.prepareFailure = AudioImportPortException(
            AudioImportFailure.UNSUPPORTED_AUDIO,
        )

        val outcome = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = false,
        )

        assertEquals(
            RunAudioImportOutcome.TerminalFailure(AudioImportFailure.UNSUPPORTED_AUDIO),
            outcome,
        )
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `retryable transcode failure preserves checkpoint until final attempt`() = runTest {
        val fixture = Fixture()
        fixture.transcoder.prepareFailure = AudioImportPortException(
            AudioImportFailure.INVALID_SOURCE,
            retryable = true,
        )

        val retry = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = false,
        )
        val exhausted = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = true,
        )

        assertEquals(
            RunAudioImportOutcome.RetryableFailure(AudioImportFailure.INVALID_SOURCE),
            retry,
        )
        assertEquals(
            RunAudioImportOutcome.RetryableFailure(AudioImportFailure.INVALID_SOURCE),
            exhausted,
        )
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `history failure cleans files after retries are exhausted`() = runTest {
        val fixture = Fixture()
        fixture.history.saveFailure = IllegalStateException("database unavailable")

        val retry = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = false,
        )
        val exhausted = fixture.run(
            job = newJob(AudioImportEngine.Local("model-a")),
            allowWarning = true,
        )

        assertEquals(
            RunAudioImportOutcome.RetryableFailure(AudioImportFailure.PERSISTENCE),
            retry,
        )
        assertEquals(
            RunAudioImportOutcome.TerminalFailure(AudioImportFailure.PERSISTENCE),
            exhausted,
        )
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `cancellation during prepare cleans task files and propagates`() = runTest {
        val fixture = Fixture()
        fixture.transcoder.prepareFailure = CancellationException("cancelled")

        var cancelled = false
        try {
            fixture.run(
                job = newJob(AudioImportEngine.Local("model-a")),
                allowWarning = false,
            )
        } catch (_: CancellationException) {
            cancelled = true
        }

        assertTrue(cancelled)
        assertEquals(listOf("import-1"), fixture.transcoder.cleaned)
    }

    @Test
    fun `existing recording retranscription updates transcript without copying audio`() = runTest {
        val fixture = Fixture()
        fixture.onlineResult = listOf(segment("replacement"))
        val job = AudioImportJob(
            id = "retry-1",
            target = AudioImportTarget.ExistingRecording(
                historyId = "history-1",
                audioPath = "/history/audio.wav",
                displayName = "Meeting",
                durationMillis = 4_000,
            ),
            engine = AudioImportEngine.Online(OnlineAsrProvider.MISTRAL_VOXTRAL),
        )

        val outcome = fixture.run(job, allowWarning = false)

        assertEquals(RunAudioImportOutcome.Completed("history-1", false), outcome)
        assertTrue(fixture.history.saved.isEmpty())
        assertEquals("history-1", fixture.history.updated.single().first)
        assertEquals(listOf(segment("replacement")), fixture.history.updated.single().second)
        assertTrue(fixture.transcoder.cleaned.isEmpty())
    }

    private class Fixture {
        val transcoder = FakeTranscoder()
        val history = FakeHistory()
        val localRequests = mutableListOf<LocalBatchTranscriptionRequest>()
        var localResult = emptyList<TranscriptSegment>()
        var localFailure: Throwable? = null
        val onlineRequests = mutableListOf<OnlineBatchTranscriptionRequest>()
        var onlineResult = emptyList<TranscriptSegment>()
        var onlineFailure: Throwable? = null
        private val model = localModel("model-a")

        suspend fun run(job: AudioImportJob, allowWarning: Boolean): RunAudioImportOutcome =
            RunAudioImport(
                transcoder = transcoder,
                recognitionSettings = FakeRecognitionSettings(
                    RecognitionSettings(
                        batchSelection = AsrModelSelection.Local(model.id),
                        installedModels = listOf(model),
                    ),
                ),
                batchCredentials = object : BatchCredentialResolverPort {
                    override suspend fun loadActive() = null
                    override suspend fun load(provider: OnlineAsrProvider) =
                        OnlineBatchCredential("secret")
                },
                localTranscription = LocalBatchTranscriptionPort { request ->
                    localRequests += request
                    localFailure?.let { throw it }
                    LocalBatchTranscriptionResult(localResult)
                },
                onlineTranscription = OnlineBatchTranscriptionPort { request ->
                    onlineRequests += request
                    onlineFailure?.let { throw it }
                    OnlineBatchTranscriptionResult(onlineResult, 1_000.0, 0u, "complete")
                },
                history = history,
            )(job, AudioImportProgressListener { _, _ -> }, allowWarning)
    }

    private class FakeTranscoder : AudioTranscoderPort {
        val cleaned = mutableListOf<String>()
        var prepareFailure: Throwable? = null

        override suspend fun prepare(
            jobId: String,
            source: AudioImportSource,
            progress: AudioImportProgressListener,
        ): PreparedImportedAudio {
            prepareFailure?.let { throw it }
            return PreparedImportedAudio(
                sourcePath = "/jobs/source.m4a",
                normalizedWavPath = "/jobs/normalized.wav",
                displayName = "Meeting.m4a",
                durationMillis = 1_000,
            )
        }

        override suspend fun cleanup(jobId: String) {
            cleaned += jobId
        }
    }

    private class FakeHistory : ImportedRecordingHistoryPort {
        val saved = mutableListOf<SaveImportedRecordingRequest>()
        val updated = mutableListOf<Pair<String, List<TranscriptSegment>>>()
        var existing = false
        var saveFailure: Throwable? = null

        override suspend fun contains(historyId: String) = existing

        override suspend fun saveImported(request: SaveImportedRecordingRequest): HistoryRecordingSummary {
            saveFailure?.let { throw it }
            saved += request
            return HistoryRecordingSummary(request.historyId)
        }

        override suspend fun updateTranscript(
            historyId: String,
            segments: List<TranscriptSegment>,
        ) {
            updated += historyId to segments
        }
    }

    private class FakeJobs : AudioImportJobPort {
        override val state = flowOf<AudioImportJobState>(AudioImportJobState.Idle)
        val enqueued = mutableListOf<AudioImportJob>()
        override suspend fun enqueue(job: AudioImportJob) { enqueued += job }
        override suspend fun cancel(jobId: String) = Unit
    }

    private class FakeBatchSettings : BatchCredentialSettingsPort {
        override val configuration = MutableStateFlow(BatchCredentialConfiguration())
        override suspend fun selectProvider(provider: OnlineAsrProvider) = Unit
        override suspend fun save(provider: OnlineAsrProvider, credential: OnlineBatchCredential) = Unit
        override suspend fun clear(provider: OnlineAsrProvider) = Unit
    }

    private class FakeRecognitionSettings(
        private val value: RecognitionSettings,
    ) : RecognitionSettingsPort {
        override val settings = MutableStateFlow(value)
        override suspend fun load() = value
        override suspend fun selectModel(slot: AsrSelectionSlot, selection: AsrModelSelection?) = Unit
        override suspend fun downloadLocalModel(
            model: LocalAsrCatalogModel,
            progress: LocalAsrDownloadProgressListener,
        ) = error("unused")
        override suspend fun validateLocalModel(modelId: String) = error("unused")
        override suspend fun deleteLocalModel(modelId: String) = Unit
    }
}

private fun newJob(engine: AudioImportEngine) = AudioImportJob(
    id = "import-1",
    target = AudioImportTarget.NewImport(AudioImportSource("content://audio/one")),
    engine = engine,
)

private fun localModel(id: String) = LocalAsrModel(
    id = id,
    displayName = id,
    config = LocalSherpaConfig(
        modelPath = "/models/$id",
        numThreads = 2,
        modelType = "sense_voice",
    ),
)

private fun segment(text: String) = TranscriptSegment(
    id = text,
    text = text,
    startSeconds = 0.0,
    endSeconds = 1.0,
    isFinal = true,
)
