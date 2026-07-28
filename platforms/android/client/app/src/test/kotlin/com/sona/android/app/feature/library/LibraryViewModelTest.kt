package com.sona.android.app.feature.library

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.library.RecordingLibraryItemStatus
import com.sona.android.application.library.RecordingLibraryPage
import com.sona.android.application.library.RecordingLibraryPort
import com.sona.android.application.recording.ActiveBatchCredential
import com.sona.android.application.recording.CloudTranscriptionFailure
import com.sona.android.application.recording.CompleteLiveDraftRequest
import com.sona.android.application.recording.CreateLiveDraftRequest
import com.sona.android.application.recording.HistoryRecordingSummary
import com.sona.android.application.recording.OnlineBatchCredential
import com.sona.android.application.recording.OnlineAsrProvider
import com.sona.android.application.recording.OnlineBatchTranscriptionResult
import com.sona.android.application.recording.RecordingDraft
import com.sona.android.application.recording.RecordingHistoryPort
import com.sona.android.application.recording.TranscribeRecordingWithCloud
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import kotlin.coroutines.Continuation
import kotlin.coroutines.resume
import kotlin.coroutines.suspendCoroutine

class LibraryViewModelTest {
    @get:Rule
    val mainDispatcherRule = MainDispatcherRule()

    @Test
    fun `refresh and pagination append distinct history items`() = runTest {
        val port = FakeLibraryPort().apply {
            pages += RecordingLibraryPage(
                items = listOf(item("history-1"), item("history-2")),
                hasMore = true,
            )
            pages += RecordingLibraryPage(
                items = listOf(item("history-2"), item("history-3")),
                hasMore = true,
            )
            pages += RecordingLibraryPage(
                items = listOf(item("history-4")),
                hasMore = false,
            )
        }
        val viewModel = libraryViewModel(port)

        viewModel.refresh()
        assertTrue(viewModel.state.value.isInitialLoading)
        advanceUntilIdle()
        viewModel.loadNextPage()
        assertTrue(viewModel.state.value.isLoadingMore)
        advanceUntilIdle()
        viewModel.loadNextPage()
        advanceUntilIdle()

        assertEquals(
            listOf("history-1", "history-2", "history-3", "history-4"),
            viewModel.state.value.items.map(RecordingLibraryItem::historyId),
        )
        assertFalse(viewModel.state.value.hasMore)
        assertEquals(listOf(0 to 30, 2 to 30, 4 to 30), port.pageRequests)
    }

    @Test
    fun `pagination skips a fully duplicate page to reach later recordings`() = runTest {
        val port = FakeLibraryPort().apply {
            pages += RecordingLibraryPage(
                items = listOf(item("history-1"), item("history-2")),
                hasMore = true,
            )
            pages += RecordingLibraryPage(
                items = listOf(item("history-1"), item("history-2")),
                hasMore = true,
            )
            pages += RecordingLibraryPage(
                items = listOf(item("history-3")),
                hasMore = false,
            )
        }
        val viewModel = libraryViewModel(port)

        viewModel.refresh()
        advanceUntilIdle()
        viewModel.loadNextPage()
        advanceUntilIdle()

        assertEquals(
            listOf("history-1", "history-2", "history-3"),
            viewModel.state.value.items.map(RecordingLibraryItem::historyId),
        )
        assertEquals(listOf(0 to 30, 2 to 30, 4 to 30), port.pageRequests)
        assertFalse(viewModel.state.value.hasMore)
    }

    @Test
    fun `list failures expose only a localized category`() = runTest {
        val sensitiveMessage = "private database path C:/secret/history.db"
        val port = FakeLibraryPort().apply {
            pageFailure = IllegalStateException(sensitiveMessage)
        }
        val viewModel = libraryViewModel(port)

        viewModel.refresh()
        advanceUntilIdle()

        assertEquals(LibraryListError.LOAD_FAILED, viewModel.state.value.listError)
        assertFalse(viewModel.state.value.toString().contains(sensitiveMessage))
    }

    @Test
    fun `an obsolete refresh cannot replace a newer result`() = runTest {
        val first = PendingResult<RecordingLibraryPage>()
        val second = PendingResult<RecordingLibraryPage>()
        val port = FakeLibraryPort().apply {
            pendingPages += first
            pendingPages += second
        }
        val viewModel = libraryViewModel(port)

        viewModel.refresh()
        runCurrent()
        viewModel.refresh()
        runCurrent()
        second.complete(RecordingLibraryPage(listOf(item("new")), hasMore = false))
        runCurrent()
        first.complete(RecordingLibraryPage(listOf(item("old")), hasMore = false))
        advanceUntilIdle()

        assertEquals(listOf("new"), viewModel.state.value.items.map { it.historyId })
    }

    @Test
    fun `transcript loading forwards the selection and classifies errors`() = runTest {
        val transcript = listOf(segment("segment-1"))
        val port = FakeLibraryPort().apply {
            transcripts["history-1"] = transcript
        }
        val viewModel = libraryViewModel(port)

        viewModel.loadTranscript("history-1")
        assertEquals(
            LibraryDetailUiState.Loading("history-1"),
            viewModel.state.value.detail,
        )
        advanceUntilIdle()
        assertEquals(
            LibraryDetailUiState.Ready("history-1", transcript),
            viewModel.state.value.detail,
        )

        val sensitiveMessage = "private transcript payload"
        port.transcriptFailure = IllegalStateException(sensitiveMessage)
        viewModel.loadTranscript("history-2")
        advanceUntilIdle()

        assertEquals(
            LibraryDetailUiState.Failed("history-2"),
            viewModel.state.value.detail,
        )
        assertFalse(viewModel.state.value.toString().contains(sensitiveMessage))
        assertEquals(listOf("history-1", "history-2"), port.transcriptRequests)
    }

    @Test
    fun `an obsolete transcript cannot replace the current selection`() = runTest {
        val pending = PendingResult<List<TranscriptSegment>>()
        val currentTranscript = listOf(segment("current-segment"))
        val port = FakeLibraryPort().apply {
            pendingTranscripts["history-old"] = pending
            transcripts["history-current"] = currentTranscript
        }
        val viewModel = libraryViewModel(port)

        viewModel.loadTranscript("history-old")
        runCurrent()
        viewModel.loadTranscript("history-current")
        runCurrent()
        assertEquals(
            LibraryDetailUiState.Ready("history-current", currentTranscript),
            viewModel.state.value.detail,
        )

        pending.complete(listOf(segment("obsolete-segment")))
        advanceUntilIdle()

        assertEquals(
            LibraryDetailUiState.Ready("history-current", currentTranscript),
            viewModel.state.value.detail,
        )
    }

    @Test
    fun `cloud transcription republishes the transcript and refreshes the list`() = runTest {
        val recording = item("history-1").copy(
            audioPath = "/recordings/history-1.wav",
            audioAvailable = true,
        )
        val port = FakeLibraryPort().apply {
            pages += RecordingLibraryPage(items = listOf(recording), hasMore = false)
        }
        val history = FakeHistoryPort()
        val viewModel = libraryViewModel(
            port = port,
            transcribe = TranscribeRecordingWithCloud(
                credentials = { activeCredential() },
                transcription = { batchResult(listOf(segment("cloud-segment"))) },
                history = history,
            ),
        )

        viewModel.transcribeWithCloud(recording)
        assertEquals(
            CloudTranscriptionUiState.Running("history-1"),
            viewModel.state.value.cloudTranscription,
        )
        advanceUntilIdle()

        assertEquals(
            CloudTranscriptionUiState.Completed("history-1"),
            viewModel.state.value.cloudTranscription,
        )
        assertEquals(
            LibraryDetailUiState.Ready("history-1", listOf(segment("cloud-segment"))),
            viewModel.state.value.detail,
        )
        assertEquals(listOf("history-1"), history.checkpointedHistoryIds)
        assertEquals(listOf(0 to 30), port.pageRequests)
    }

    @Test
    fun `a draft is completed instead of checkpointed`() = runTest {
        val draft = item("history-draft").copy(
            status = RecordingLibraryItemStatus.DRAFT,
            audioPath = "/recordings/history-draft.wav",
            audioAvailable = true,
        )
        val port = FakeLibraryPort().apply {
            pages += RecordingLibraryPage(items = emptyList(), hasMore = false)
        }
        val history = FakeHistoryPort()
        val viewModel = libraryViewModel(
            port = port,
            transcribe = TranscribeRecordingWithCloud(
                credentials = { activeCredential() },
                transcription = {
                    batchResult(listOf(segment("cloud-segment")), audioDurationMillis = 2_400.4)
                },
                history = history,
            ),
        )

        viewModel.transcribeWithCloud(draft)
        advanceUntilIdle()

        assertEquals(emptyList<String>(), history.checkpointedHistoryIds)
        assertEquals(listOf("history-draft" to 2_400L), history.completedDrafts)
    }

    @Test
    fun `a missing credential is reported without touching history`() = runTest {
        val recording = item("history-1").copy(
            audioPath = "/recordings/history-1.wav",
            audioAvailable = true,
        )
        val history = FakeHistoryPort()
        val viewModel = libraryViewModel(
            port = FakeLibraryPort(),
            transcribe = TranscribeRecordingWithCloud(
                credentials = { null },
                transcription = { throw AssertionError("must not transcribe") },
                history = history,
            ),
        )

        viewModel.transcribeWithCloud(recording)
        advanceUntilIdle()

        assertEquals(
            CloudTranscriptionUiState.Failed(
                historyId = "history-1",
                reason = CloudTranscriptionFailure.MISSING_CREDENTIAL,
            ),
            viewModel.state.value.cloudTranscription,
        )
        assertEquals(emptyList<String>(), history.checkpointedHistoryIds)
    }

    @Test
    fun `provider failures stay categorized and never leak provider text`() = runTest {
        val sensitiveMessage = "Bearer sk-live-secret at https://provider.example/v1"
        val recording = item("history-1").copy(
            audioPath = "/recordings/history-1.wav",
            audioAvailable = true,
        )
        val history = FakeHistoryPort()
        val viewModel = libraryViewModel(
            port = FakeLibraryPort(),
            transcribe = TranscribeRecordingWithCloud(
                credentials = { activeCredential() },
                transcription = { throw IllegalStateException(sensitiveMessage) },
                history = history,
            ),
        )

        viewModel.transcribeWithCloud(recording)
        advanceUntilIdle()

        assertEquals(
            CloudTranscriptionUiState.Failed(
                historyId = "history-1",
                reason = CloudTranscriptionFailure.TRANSCRIPTION_FAILED,
            ),
            viewModel.state.value.cloudTranscription,
        )
        assertFalse(viewModel.state.value.toString().contains(sensitiveMessage))
        assertEquals(emptyList<String>(), history.checkpointedHistoryIds)
    }

    @Test
    fun `a second run is ignored while one is still in flight`() = runTest {
        val pending = PendingResult<OnlineBatchTranscriptionResult>()
        val recording = item("history-1").copy(
            audioPath = "/recordings/history-1.wav",
            audioAvailable = true,
        )
        var transcribeCalls = 0
        val viewModel = libraryViewModel(
            port = FakeLibraryPort(),
            transcribe = TranscribeRecordingWithCloud(
                credentials = { activeCredential() },
                transcription = {
                    transcribeCalls += 1
                    pending.await()
                },
                history = FakeHistoryPort(),
            ),
        )

        viewModel.transcribeWithCloud(recording)
        runCurrent()
        viewModel.transcribeWithCloud(recording)
        runCurrent()

        assertEquals(1, transcribeCalls)
        pending.complete(batchResult(listOf(segment("cloud-segment"))))
        advanceUntilIdle()
        assertEquals(
            CloudTranscriptionUiState.Completed("history-1"),
            viewModel.state.value.cloudTranscription,
        )
    }

    @Test
    fun `reopening a recording clears an earlier cloud banner`() = runTest {
        val recording = item("history-1").copy(
            audioPath = "/recordings/history-1.wav",
            audioAvailable = true,
        )
        val port = FakeLibraryPort().apply {
            pages += RecordingLibraryPage(items = listOf(recording), hasMore = false)
            transcripts["history-1"] = listOf(segment("cloud-segment"))
        }
        val viewModel = libraryViewModel(
            port = port,
            transcribe = TranscribeRecordingWithCloud(
                credentials = { activeCredential() },
                transcription = { batchResult(listOf(segment("cloud-segment"))) },
                history = FakeHistoryPort(),
            ),
        )

        viewModel.transcribeWithCloud(recording)
        advanceUntilIdle()
        viewModel.loadTranscript("history-1")
        advanceUntilIdle()

        assertEquals(CloudTranscriptionUiState.Idle, viewModel.state.value.cloudTranscription)
    }

    private fun libraryViewModel(
        port: RecordingLibraryPort,
        transcribe: TranscribeRecordingWithCloud = TranscribeRecordingWithCloud(
            credentials = { null },
            transcription = { throw AssertionError("must not transcribe") },
            history = FakeHistoryPort(),
        ),
    ) = LibraryViewModel(port, transcribe)

    private fun activeCredential() = ActiveBatchCredential(
        provider = OnlineAsrProvider.GROQ_WHISPER,
        credential = OnlineBatchCredential("temporary-secret"),
    )

    private fun batchResult(
        segments: List<TranscriptSegment>,
        audioDurationMillis: Double = 1_000.0,
    ) = OnlineBatchTranscriptionResult(
        segments = segments,
        audioDurationMillis = audioDurationMillis,
        bufferedSamples = 16_000uL,
        stage = "batch_complete",
    )

    private fun item(id: String) = RecordingLibraryItem(
        historyId = id,
        title = "Recording $id",
        timestampEpochMillis = 1_725_000_000_000,
        durationMillis = 1_000,
        previewText = "Preview",
        status = RecordingLibraryItemStatus.COMPLETE,
    )

    private fun segment(id: String) = TranscriptSegment(
        id = id,
        text = "Hello",
        startSeconds = 0.0,
        endSeconds = 1.0,
        isFinal = true,
    )

    private class FakeLibraryPort : RecordingLibraryPort {
        val pages = ArrayDeque<RecordingLibraryPage>()
        val pendingPages = ArrayDeque<PendingResult<RecordingLibraryPage>>()
        val pageRequests = mutableListOf<Pair<Int, Int>>()
        val transcripts = mutableMapOf<String, List<TranscriptSegment>>()
        val pendingTranscripts = mutableMapOf<String, PendingResult<List<TranscriptSegment>>>()
        val transcriptRequests = mutableListOf<String>()
        var pageFailure: Throwable? = null
        var transcriptFailure: Throwable? = null

        override suspend fun loadPage(offset: Int, limit: Int): RecordingLibraryPage {
            pageRequests += offset to limit
            pageFailure?.let { throw it }
            pendingPages.removeFirstOrNull()?.let { return it.await() }
            return pages.removeFirst()
        }

        override suspend fun loadTranscript(historyId: String): List<TranscriptSegment> {
            transcriptRequests += historyId
            transcriptFailure?.let { throw it }
            pendingTranscripts[historyId]?.let { return it.await() }
            return transcripts.getValue(historyId)
        }
    }

    private class FakeHistoryPort : RecordingHistoryPort {
        val checkpointedHistoryIds = mutableListOf<String>()
        val completedDrafts = mutableListOf<Pair<String, Long>>()

        override suspend fun createLiveDraft(request: CreateLiveDraftRequest): RecordingDraft =
            throw AssertionError("cloud transcription must not create drafts")

        override suspend fun checkpointTranscript(
            historyId: String,
            segments: List<TranscriptSegment>,
        ) {
            checkpointedHistoryIds += historyId
        }

        override suspend fun completeLiveDraft(
            request: CompleteLiveDraftRequest,
        ): HistoryRecordingSummary {
            completedDrafts += request.historyId to request.durationMillis
            return HistoryRecordingSummary(request.historyId)
        }

        override suspend fun deleteDraft(historyId: String) =
            throw AssertionError("cloud transcription must not delete drafts")
    }

    private class PendingResult<T> {
        private lateinit var continuation: Continuation<T>

        suspend fun await(): T = suspendCoroutine { continuation = it }

        fun complete(value: T) {
            continuation.resume(value)
        }
    }
}
