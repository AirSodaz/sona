package com.sona.android.app.feature.library

import com.sona.android.app.MainDispatcherRule
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.data.TranscriptExportPort
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.data.TranscriptExportRequest
import com.sona.android.application.data.TranscriptExportResult
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemKind
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.HistoryWorkspaceCounts
import com.sona.android.application.library.HistoryWorkspacePage
import com.sona.android.application.library.HistoryWorkspacePort
import com.sona.android.application.library.HistoryWorkspaceQuery
import com.sona.android.application.library.HistoryWorkspaceSummary
import com.sona.android.application.library.TagRecord
import com.sona.android.application.library.TagWorkspacePort
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.CommitTranscriptEditRequest
import com.sona.android.application.library.CommitTranscriptEditResult
import com.sona.android.application.library.HistoryMediaSource
import com.sona.android.application.library.HistoryMediaSourcePort
import com.sona.android.application.library.TranscriptEditOperation
import com.sona.android.application.library.TranscriptEditorPort
import com.sona.android.application.library.TranscriptSnapshotReason
import com.sona.android.application.media.AudioPlaybackPort
import com.sona.android.application.media.AudioPlaybackState
import com.sona.android.application.recovery.TranscriptEditDraft
import com.sona.android.application.recovery.TranscriptEditRecoveryPort
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
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.flow.MutableStateFlow
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
    fun `edit draft debounces recovery supports undo and commits explicitly`() = runTest {
        val port = FakeLibraryPort().apply { transcripts["history-1"] = listOf(segment("segment-1")) }
        val editor = FakeTranscriptEditor()
        val recovery = FakeTranscriptRecovery()
        val viewModel = libraryViewModel(port, editor = editor, editRecovery = recovery)

        viewModel.loadTranscript("history-1")
        advanceUntilIdle()
        viewModel.startEditing("history-1")
        viewModel.updateSegmentText("segment-1", "Edited")
        runCurrent()
        assertEquals("Edited", viewModel.state.value.editor.draftSegments.single().text)
        advanceTimeBy(499)
        assertEquals(0, recovery.saved.size)
        advanceTimeBy(1)
        runCurrent()
        assertEquals("Edited", recovery.saved.single().draftSegments.single().text)

        viewModel.undoEdit()
        assertEquals("Hello", viewModel.state.value.editor.draftSegments.single().text)
        viewModel.redoEdit()
        assertEquals("Edited", viewModel.state.value.editor.draftSegments.single().text)
        viewModel.saveEdit()
        advanceUntilIdle()

        assertEquals("Edited", editor.committed?.editedSegments?.single()?.text)
        assertFalse(viewModel.state.value.editor.active)
        assertEquals(listOf("history-1"), recovery.discarded)
    }

    @Test
    fun `edit auto saves after two seconds and keeps one edit session`() = runTest {
        val port = FakeLibraryPort().apply { transcripts["history-1"] = listOf(segment("segment-1")) }
        val editor = FakeTranscriptEditor()
        val recovery = FakeTranscriptRecovery()
        val viewModel = libraryViewModel(port, editor = editor, editRecovery = recovery)

        viewModel.loadTranscript("history-1")
        advanceUntilIdle()
        viewModel.startEditing("history-1")
        viewModel.updateSegmentText("segment-1", "First")
        runCurrent()
        advanceTimeBy(LibraryViewModel.EDIT_AUTO_SAVE_DEBOUNCE_MILLIS - 1)
        assertTrue(editor.commits.isEmpty())
        advanceTimeBy(1)
        runCurrent()

        assertTrue(viewModel.state.value.editor.active)
        assertFalse(viewModel.state.value.editor.dirty)
        val editSessionId = editor.commits.single().editSessionId
        assertTrue(editSessionId.isNotBlank())

        viewModel.updateSegmentText("segment-1", "Second")
        runCurrent()
        advanceTimeBy(LibraryViewModel.EDIT_AUTO_SAVE_DEBOUNCE_MILLIS)
        advanceUntilIdle()

        assertEquals(2, editor.commits.size)
        assertEquals(listOf(editSessionId, editSessionId), editor.commits.map { it.editSessionId })
        assertEquals("First", editor.commits[1].baseSegments.single().text)
        assertEquals("Second", editor.commits[1].editedSegments.single().text)
    }

    @Test
    fun `refresh and pagination append distinct history items`() = runTest {
        val port = FakeLibraryPort().apply {
            pages += page(
                items = listOf(item("history-1"), item("history-2")),
                hasMore = true,
            )
            pages += page(
                items = listOf(item("history-2"), item("history-3")),
                hasMore = true,
            )
            pages += page(
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
            viewModel.state.value.items.map(HistoryItem::historyId),
        )
        assertFalse(viewModel.state.value.hasMore)
        assertEquals(listOf(0 to 30, 2 to 30, 4 to 30), port.pageRequests)
    }

    @Test
    fun `pagination skips a fully duplicate page to reach later recordings`() = runTest {
        val port = FakeLibraryPort().apply {
            pages += page(
                items = listOf(item("history-1"), item("history-2")),
                hasMore = true,
            )
            pages += page(
                items = listOf(item("history-1"), item("history-2")),
                hasMore = true,
            )
            pages += page(
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
            viewModel.state.value.items.map(HistoryItem::historyId),
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
        val first = PendingResult<HistoryWorkspacePage>()
        val second = PendingResult<HistoryWorkspacePage>()
        val port = FakeLibraryPort().apply {
            pendingPages += first
            pendingPages += second
        }
        val viewModel = libraryViewModel(port)

        viewModel.refresh()
        runCurrent()
        viewModel.refresh()
        runCurrent()
        second.complete(page(listOf(item("new")), hasMore = false))
        runCurrent()
        first.complete(page(listOf(item("old")), hasMore = false))
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
    fun `late media source resolution cannot replace the current detail playback`() = runTest {
        val oldSource = PendingResult<HistoryMediaSource?>()
        val currentSource = PendingResult<HistoryMediaSource?>()
        val mediaSources = object : HistoryMediaSourcePort {
            override suspend fun resolve(historyId: String): HistoryMediaSource? =
                if (historyId == "history-old") oldSource.await() else currentSource.await()
        }
        val playback = FakeAudioPlayback()
        val port = FakeLibraryPort().apply {
            transcripts["history-old"] = listOf(segment("old-segment"))
            transcripts["history-current"] = listOf(segment("current-segment"))
        }
        val viewModel = libraryViewModel(port, mediaSources = mediaSources, playback = playback)

        viewModel.loadTranscript("history-old")
        runCurrent()
        viewModel.loadTranscript("history-current")
        runCurrent()
        currentSource.complete(HistoryMediaSource("/current.m4a"))
        runCurrent()
        oldSource.complete(HistoryMediaSource("/old.m4a"))
        advanceUntilIdle()

        assertEquals(listOf("history-current" to "/current.m4a"), playback.prepared)
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
            pages += page(items = listOf(recording), hasMore = false)
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
            status = HistoryItemStatus.DRAFT,
            audioPath = "/recordings/history-draft.wav",
            audioAvailable = true,
        )
        val port = FakeLibraryPort().apply {
            pages += page(items = emptyList(), hasMore = false)
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
            pages += page(items = listOf(recording), hasMore = false)
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

    @Test
    fun `query changes reset offset and forward typed filters`() = runTest {
        val port = FakeLibraryPort().apply {
            repeat(3) { pages += page(emptyList(), false) }
        }
        val viewModel = libraryViewModel(port)

        viewModel.refresh()
        advanceUntilIdle()
        viewModel.setSearchQuery("meeting")
        advanceUntilIdle()
        viewModel.setScope(HistoryScope.Trash)
        advanceUntilIdle()

        assertEquals(listOf(0, 0, 0), port.queries.map { it.offset })
        assertEquals("meeting", port.queries.last().query)
        assertEquals(HistoryScope.Trash, port.queries.last().scope)
    }

    @Test
    fun `selection mutations use typed batch operations`() = runTest {
        val port = FakeLibraryPort().apply {
            repeat(3) { pages += page(emptyList(), false) }
        }
        val viewModel = libraryViewModel(port)

        viewModel.toggleSelection("history-1")
        viewModel.addTagToSelected("tag-1")
        advanceUntilIdle()
        viewModel.toggleSelection("history-2")
        viewModel.trashSelected()
        advanceUntilIdle()

        assertEquals(listOf(listOf("history-1") to listOf("tag-1")), port.addedTags)
        assertEquals(listOf(listOf("history-2")), port.trashed)
        assertTrue(viewModel.state.value.selectedIds.isEmpty())
    }

    @Test
    fun `transcript export publishes and always cleans staging`() = runTest {
        val port = FakeLibraryPort().apply { transcripts["history-1"] = listOf(segment("segment-1")) }
        val exporter = FakeExporter()
        val files = RecordingFileTransfer()
        val viewModel = libraryViewModel(port, exporter = exporter, files = files)
        viewModel.loadTranscript("history-1")
        advanceUntilIdle()

        viewModel.exportTranscript("content://destination", TranscriptExportFormat.VTT, TranscriptExportMode.BILINGUAL)
        advanceUntilIdle()

        assertEquals(TranscriptExportFormat.VTT, exporter.request?.format)
        assertEquals(TranscriptExportMode.BILINGUAL, exporter.request?.mode)
        assertEquals(listOf("/cache/sona-transcript.vtt" to "content://destination"), files.published)
        assertEquals(listOf("/cache/sona-transcript.vtt"), files.cleaned)
    }

    private fun libraryViewModel(
        port: HistoryWorkspacePort,
        transcribe: TranscribeRecordingWithCloud = TranscribeRecordingWithCloud(
            credentials = { null },
            transcription = { throw AssertionError("must not transcribe") },
            history = FakeHistoryPort(),
        ),
        exporter: TranscriptExportPort = TranscriptExportPort { error("must not export") },
        files: FileTransferPort = FakeFileTransfer,
        editor: TranscriptEditorPort? = null,
        editRecovery: TranscriptEditRecoveryPort? = null,
        mediaSources: HistoryMediaSourcePort? = null,
        playback: AudioPlaybackPort = FakeAudioPlayback(),
    ) = LibraryViewModel(
        library = port,
        transcribeRecordingWithCloud = transcribe,
        tags = FakeTagWorkspace,
        exporter = exporter,
        files = files,
        editor = editor,
        editRecovery = editRecovery,
        mediaSources = mediaSources,
        playback = playback,
    )

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

    private fun item(id: String) = HistoryItem(
        historyId = id,
        title = "Recording $id",
        timestampEpochMillis = 1_725_000_000_000,
        durationMillis = 1_000,
        previewText = "Preview",
        status = HistoryItemStatus.COMPLETE,
        kind = HistoryItemKind.RECORDING,
    )

    private fun segment(id: String) = TranscriptSegment(
        id = id,
        text = "Hello",
        startSeconds = 0.0,
        endSeconds = 1.0,
        isFinal = true,
    )

    private class FakeLibraryPort : HistoryWorkspacePort {
        val pages = ArrayDeque<HistoryWorkspacePage>()
        val pendingPages = ArrayDeque<PendingResult<HistoryWorkspacePage>>()
        val pageRequests = mutableListOf<Pair<Int, Int>>()
        val queries = mutableListOf<HistoryWorkspaceQuery>()
        val transcripts = mutableMapOf<String, List<TranscriptSegment>>()
        val pendingTranscripts = mutableMapOf<String, PendingResult<List<TranscriptSegment>>>()
        val transcriptRequests = mutableListOf<String>()
        var pageFailure: Throwable? = null
        var transcriptFailure: Throwable? = null
        val addedTags = mutableListOf<Pair<List<String>, List<String>>>()
        val trashed = mutableListOf<List<String>>()

        override suspend fun query(request: HistoryWorkspaceQuery): HistoryWorkspacePage {
            pageRequests += request.offset to request.limit
            queries += request
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

        override suspend fun updateTitle(historyId: String, title: String) = Unit
        override suspend fun updateTags(ids: List<String>, addTagIds: List<String>, removeTagIds: List<String>) {
            if (addTagIds.isNotEmpty()) addedTags += ids to addTagIds
        }
        override suspend fun trash(ids: List<String>, deletedAtEpochMillis: Long) { trashed += ids }
        override suspend fun restore(ids: List<String>) = Unit
        override suspend fun purge(ids: List<String>) = Unit
        override suspend fun listSnapshots(historyId: String) = emptyList<TranscriptSnapshot>()
        override suspend fun loadSnapshot(historyId: String, snapshotId: String) = null
    }

    private object FakeTagWorkspace : TagWorkspacePort {
        override suspend fun listTags() = emptyList<TagRecord>()
        override suspend fun createTag(request: com.sona.android.application.library.CreateTagRequest) =
            error("must not create a tag")
        override suspend fun renameTag(tagId: String, name: String): TagRecord? = null
        override suspend fun deleteTag(tagId: String) = Unit
    }

    private object FakeFileTransfer : FileTransferPort {
        override suspend fun stageImport(sourceUri: String) = error("must not stage")
        override suspend fun publishExport(stagedPath: String, destinationUri: String) = Unit
        override suspend fun createExportStagingPath(fileName: String) = error("must not export")
        override suspend fun cleanup(path: String) = Unit
        override suspend fun publishText(text: String, destinationUri: String) = Unit
    }

    private class FakeExporter : TranscriptExportPort {
        var request: TranscriptExportRequest? = null
        override suspend fun export(request: TranscriptExportRequest): TranscriptExportResult {
            this.request = request
            return TranscriptExportResult(request.outputPath, 10)
        }
    }

    private class RecordingFileTransfer : FileTransferPort {
        val published = mutableListOf<Pair<String, String>>()
        val cleaned = mutableListOf<String>()
        override suspend fun stageImport(sourceUri: String) = error("unused")
        override suspend fun publishExport(stagedPath: String, destinationUri: String) {
            published += stagedPath to destinationUri
        }
        override suspend fun createExportStagingPath(fileName: String) = "/cache/$fileName"
        override suspend fun cleanup(path: String) { cleaned += path }
        override suspend fun publishText(text: String, destinationUri: String) = Unit
    }

    companion object {
        private fun page(
            items: List<HistoryItem>,
            hasMore: Boolean,
        ) = HistoryWorkspacePage(
            items = items,
            filteredItemCount = items.size.toLong(),
            hasMore = hasMore,
            summary = HistoryWorkspaceSummary(
                totalItems = items.size.toLong(),
                totalDurationMillis = items.sumOf(HistoryItem::durationMillis),
                latestTimestampEpochMillis = items.maxOfOrNull(HistoryItem::timestampEpochMillis),
                recordingCount = items.count { it.kind == HistoryItemKind.RECORDING }.toLong(),
                batchCount = items.count { it.kind == HistoryItemKind.BATCH }.toLong(),
            ),
            counts = HistoryWorkspaceCounts(0, 0, emptyMap()),
        )
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

    private class FakeAudioPlayback : AudioPlaybackPort {
        override val state = MutableStateFlow(AudioPlaybackState())
        val prepared = mutableListOf<Pair<String, String>>()

        override suspend fun prepare(historyId: String, nativePath: String) {
            prepared += historyId to nativePath
        }

        override fun play() = Unit
        override fun pause() = Unit
        override fun seekTo(positionMillis: Long) = Unit
        override fun seekBy(deltaMillis: Long) = Unit
        override fun setSpeed(speed: Float) = Unit
        override fun release() = Unit
    }

    private class FakeTranscriptEditor : TranscriptEditorPort {
        var committed: CommitTranscriptEditRequest? = null
        val commits = mutableListOf<CommitTranscriptEditRequest>()

        override suspend fun apply(
            segments: List<TranscriptSegment>,
            operation: TranscriptEditOperation,
        ): List<TranscriptSegment> = when (operation) {
            is TranscriptEditOperation.UpdateText -> segments.map {
                if (it.id == operation.segmentId) it.copy(text = operation.text) else it
            }
            is TranscriptEditOperation.UpdateTranslation -> segments.map {
                if (it.id == operation.segmentId) it.copy(translation = operation.translation) else it
            }
            is TranscriptEditOperation.Delete -> segments.filterNot { it.id == operation.segmentId }
            is TranscriptEditOperation.MergeNext, is TranscriptEditOperation.Split -> segments
        }

        override suspend fun commit(request: CommitTranscriptEditRequest): CommitTranscriptEditResult {
            committed = request
            commits += request
            return CommitTranscriptEditResult.Committed(
                TranscriptSnapshot("snapshot-1", request.historyId, TranscriptSnapshotReason.MANUAL_EDIT, 1, request.baseSegments.size.toLong()),
            )
        }
    }

    private class FakeTranscriptRecovery : TranscriptEditRecoveryPort {
        val saved = mutableListOf<TranscriptEditDraft>()
        val discarded = mutableListOf<String>()
        override suspend fun load(historyId: String): TranscriptEditDraft? = null
        override suspend fun save(draft: TranscriptEditDraft) { saved += draft }
        override suspend fun discard(historyId: String) { discarded += historyId }
    }
}
