package com.sona.android.app.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.library.HistoryItem
import com.sona.android.application.library.HistoryItemStatus
import com.sona.android.application.library.HistoryDateFilter
import com.sona.android.application.library.HistoryFilterType
import com.sona.android.application.library.HistoryScope
import com.sona.android.application.library.HistorySortOrder
import com.sona.android.application.library.HistoryWorkspacePort
import com.sona.android.application.library.HistoryWorkspaceQuery
import com.sona.android.application.library.TagRecord
import com.sona.android.application.library.TagWorkspacePort
import com.sona.android.application.library.TranscriptSnapshot
import com.sona.android.application.library.TranscriptSnapshotDetail
import com.sona.android.application.library.CreateTagRequest
import com.sona.android.application.data.FileTransferPort
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.data.TranscriptExportPort
import com.sona.android.application.data.TranscriptExportRequest
import com.sona.android.application.recording.CloudTranscriptionFailure
import com.sona.android.application.recording.CloudTranscriptionOutcome
import com.sona.android.application.recording.CloudTranscriptionRequest
import com.sona.android.application.recording.AudioImportFailure
import com.sona.android.application.recording.AudioImportJobPort
import com.sona.android.application.recording.AudioImportJobState
import com.sona.android.application.recording.AudioImportSource
import com.sona.android.application.recording.ScheduleAudioImport
import com.sona.android.application.recording.ScheduleAudioImportOutcome
import com.sona.android.application.recording.ScheduleAudioRetranscription
import com.sona.android.application.recording.TranscribeRecordingWithCloud
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.distinctUntilChanged
import kotlinx.coroutines.flow.flowOf
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

enum class LibraryListError {
    LOAD_FAILED,
}

sealed interface LibraryDetailUiState {
    data object None : LibraryDetailUiState

    data class Loading(
        val historyId: String,
    ) : LibraryDetailUiState

    data class Ready(
        val historyId: String,
        val segments: List<TranscriptSegment>,
    ) : LibraryDetailUiState

    data class Failed(
        val historyId: String,
    ) : LibraryDetailUiState
}

sealed interface CloudTranscriptionUiState {
    data object Idle : CloudTranscriptionUiState

    data class Running(
        val historyId: String,
    ) : CloudTranscriptionUiState

    data class Completed(
        val historyId: String,
    ) : CloudTranscriptionUiState

    data class Failed(
        val historyId: String,
        val reason: CloudTranscriptionFailure,
    ) : CloudTranscriptionUiState
}

data class LibraryUiState(
    val items: List<HistoryItem> = emptyList(),
    val hasMore: Boolean = false,
    val isInitialLoading: Boolean = false,
    val isRefreshing: Boolean = false,
    val isLoadingMore: Boolean = false,
    val listError: LibraryListError? = null,
    val detail: LibraryDetailUiState = LibraryDetailUiState.None,
    val cloudTranscription: CloudTranscriptionUiState = CloudTranscriptionUiState.Idle,
    val audioImport: AudioImportJobState = AudioImportJobState.Idle,
    val query: HistoryWorkspaceQuery = HistoryWorkspaceQuery(),
    val tags: List<TagRecord> = emptyList(),
    val selectedIds: Set<String> = emptySet(),
    val snapshots: List<TranscriptSnapshot> = emptyList(),
    val snapshotDetail: TranscriptSnapshotDetail? = null,
    val workspaceCount: Long = 0,
    val trashCount: Long = 0,
    val operationInProgress: Boolean = false,
    val operationError: Boolean = false,
)

class LibraryViewModel(
    private val library: HistoryWorkspacePort,
    private val transcribeRecordingWithCloud: TranscribeRecordingWithCloud,
    private val scheduleAudioImport: ScheduleAudioImport? = null,
    private val scheduleAudioRetranscription: ScheduleAudioRetranscription? = null,
    private val audioImportJobs: AudioImportJobPort = IdleAudioImportJobPort,
    private val tags: TagWorkspacePort,
    private val exporter: TranscriptExportPort,
    private val files: FileTransferPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(LibraryUiState())
    val state: StateFlow<LibraryUiState> = mutableState.asStateFlow()

    private var listJob: Job? = null
    private var detailJob: Job? = null
    private var failedListOperation: FailedListOperation? = null
    private var nextOffset: Int = 0
    private var listGeneration: Int = 0
    private var detailGeneration: Int = 0

    init {
        viewModelScope.launch {
            audioImportJobs.state.distinctUntilChanged().collect { importState ->
                val previous = mutableState.value.audioImport
                mutableState.update { it.copy(audioImport = importState) }
                if (
                    importState is AudioImportJobState.Completed &&
                    importState != previous
                ) {
                    refresh()
                }
            }
        }
    }

    fun importAudio(sourceLocator: String) {
        if (sourceLocator.isBlank() || mutableState.value.audioImport is AudioImportJobState.Running) {
            return
        }
        val schedule = scheduleAudioImport ?: run {
            mutableState.update {
                it.copy(audioImport = AudioImportJobState.Failed(null, AudioImportFailure.CONFIGURATION))
            }
            return
        }
        viewModelScope.launch {
            when (schedule(AudioImportSource(sourceLocator))) {
                is ScheduleAudioImportOutcome.Scheduled -> Unit
                ScheduleAudioImportOutcome.NeedsConfiguration -> mutableState.update {
                    it.copy(
                        audioImport = AudioImportJobState.Failed(
                            jobId = null,
                            reason = AudioImportFailure.CONFIGURATION,
                        ),
                    )
                }
                ScheduleAudioImportOutcome.Failed -> mutableState.update {
                    it.copy(
                        audioImport = AudioImportJobState.Failed(
                            jobId = null,
                            reason = AudioImportFailure.INVALID_SOURCE,
                        ),
                    )
                }
            }
        }
    }

    fun cancelAudioImport() {
        val running = mutableState.value.audioImport as? AudioImportJobState.Running ?: return
        viewModelScope.launch { audioImportJobs.cancel(running.jobId) }
    }

    fun transcribeWithCurrentEngine(item: HistoryItem) {
        if (
            !item.audioAvailable ||
            item.audioPath.isBlank() ||
            mutableState.value.audioImport is AudioImportJobState.Running
        ) return
        val schedule = scheduleAudioRetranscription ?: return
        viewModelScope.launch {
            when (
                schedule(
                    historyId = item.historyId,
                    audioPath = item.audioPath,
                    displayName = item.title,
                    durationMillis = item.durationMillis,
                )
            ) {
                is ScheduleAudioImportOutcome.Scheduled -> Unit
                ScheduleAudioImportOutcome.NeedsConfiguration -> mutableState.update {
                    it.copy(audioImport = AudioImportJobState.Failed(null, AudioImportFailure.CONFIGURATION))
                }
                ScheduleAudioImportOutcome.Failed -> mutableState.update {
                    it.copy(audioImport = AudioImportJobState.Failed(null, AudioImportFailure.INVALID_SOURCE))
                }
            }
        }
    }

    fun refresh() {
        val generation = ++listGeneration
        listJob?.cancel()
        val hasItems = mutableState.value.items.isNotEmpty()
        mutableState.update {
            it.copy(
                isInitialLoading = !hasItems,
                isRefreshing = hasItems,
                isLoadingMore = false,
                listError = null,
            )
        }
        listJob = viewModelScope.launch {
            try {
                val page = library.query(mutableState.value.query.copy(offset = 0, limit = PAGE_SIZE))
                if (generation != listGeneration) return@launch
                nextOffset = page.items.size
                failedListOperation = null
                mutableState.update {
                    it.copy(
                        items = page.items.distinctBy(HistoryItem::historyId),
                        hasMore = page.hasMore && page.items.isNotEmpty(),
                        isInitialLoading = false,
                        isRefreshing = false,
                        listError = null,
                        workspaceCount = page.filteredItemCount,
                        trashCount = page.counts.trash,
                    )
                }
                runCatching { tags.listTags() }.getOrNull()?.let { loaded ->
                    mutableState.update { it.copy(tags = loaded) }
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                if (generation != listGeneration) return@launch
                failedListOperation = FailedListOperation.REFRESH
                mutableState.update {
                    it.copy(
                        isInitialLoading = false,
                        isRefreshing = false,
                        listError = LibraryListError.LOAD_FAILED,
                    )
                }
            }
        }
    }

    fun loadNextPage() {
        val current = mutableState.value
        if (
            !current.hasMore ||
            current.isInitialLoading ||
            current.isRefreshing ||
            current.isLoadingMore
        ) {
            return
        }
        val generation = listGeneration
        val offset = nextOffset
        mutableState.update { it.copy(isLoadingMore = true, listError = null) }
        listJob = viewModelScope.launch {
            try {
                var pageOffset = offset
                while (true) {
                    val page = library.query(mutableState.value.query.copy(offset = pageOffset, limit = PAGE_SIZE))
                    if (generation != listGeneration) return@launch
                    val existingIds = mutableState.value.items
                        .mapTo(mutableSetOf(), HistoryItem::historyId)
                    val containsNewItem = page.items.any { it.historyId !in existingIds }
                    val pageHasMore = page.hasMore && page.items.isNotEmpty()
                    nextOffset = pageOffset + page.items.size
                    if (containsNewItem || !pageHasMore) {
                        failedListOperation = null
                        mutableState.update { state ->
                            state.copy(
                                items = (state.items + page.items)
                                    .distinctBy(HistoryItem::historyId),
                                hasMore = pageHasMore,
                                isLoadingMore = false,
                                listError = null,
                            )
                        }
                        break
                    }
                    pageOffset = nextOffset
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                if (generation != listGeneration) return@launch
                failedListOperation = FailedListOperation.LOAD_MORE
                mutableState.update {
                    it.copy(
                        isLoadingMore = false,
                        listError = LibraryListError.LOAD_FAILED,
                    )
                }
            }
        }
    }

    fun retryList() {
        when (failedListOperation) {
            FailedListOperation.LOAD_MORE -> loadNextPage()
            FailedListOperation.REFRESH,
            null,
            -> refresh()
        }
    }

    fun loadTranscript(historyId: String) {
        if (historyId.isBlank()) return
        val generation = ++detailGeneration
        detailJob?.cancel()
        mutableState.update {
            it.copy(
                detail = LibraryDetailUiState.Loading(historyId),
                cloudTranscription = it.cloudTranscription.clearedOnReload(),
            )
        }
        detailJob = viewModelScope.launch {
            try {
                val segments = library.loadTranscript(historyId)
                if (generation != detailGeneration) return@launch
                mutableState.update {
                    it.copy(
                        detail = LibraryDetailUiState.Ready(historyId, segments),
                        snapshots = runCatching { library.listSnapshots(historyId) }.getOrDefault(emptyList()),
                        snapshotDetail = null,
                    )
                }
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                if (generation != detailGeneration) return@launch
                mutableState.update {
                    it.copy(detail = LibraryDetailUiState.Failed(historyId))
                }
            }
        }
    }

    fun setSearchQuery(value: String) = updateQuery { copy(query = value) }
    fun setScope(value: HistoryScope) = updateQuery { copy(scope = value) }
    fun setFilter(value: HistoryFilterType) = updateQuery { copy(filterType = value) }
    fun setDateFilter(value: HistoryDateFilter) = updateQuery { copy(dateFilter = value) }
    fun setSortOrder(value: HistorySortOrder) = updateQuery { copy(sortOrder = value) }

    fun toggleSelection(historyId: String) = mutableState.update { state ->
        state.copy(selectedIds = state.selectedIds.toMutableSet().apply {
            if (!add(historyId)) remove(historyId)
        })
    }

    fun clearSelection() = mutableState.update { it.copy(selectedIds = emptySet()) }

    fun resetAfterRestore() {
        detailGeneration += 1
        detailJob?.cancel()
        mutableState.update {
            it.copy(
                items = emptyList(),
                hasMore = false,
                detail = LibraryDetailUiState.None,
                cloudTranscription = CloudTranscriptionUiState.Idle,
                snapshots = emptyList(),
                snapshotDetail = null,
                selectedIds = emptySet(),
            )
        }
        refresh()
    }

    fun trashSelected() = mutateSelection { library.trash(it, System.currentTimeMillis()) }
    fun restoreSelected() = mutateSelection(library::restore)
    fun purgeSelected() = mutateSelection(library::purge)
    fun addTagToSelected(tagId: String) = mutateSelection { library.updateTags(it, listOf(tagId), emptyList()) }
    fun removeTagFromSelected(tagId: String) = mutateSelection { library.updateTags(it, emptyList(), listOf(tagId)) }

    fun updateTitle(historyId: String, title: String) = mutateWorkspace {
        library.updateTitle(historyId, title)
    }

    fun updateTags(historyId: String, selectedTagIds: Set<String>) = mutateWorkspace {
        val current = mutableState.value.items.firstOrNull { it.historyId == historyId }?.tagIds.orEmpty().toSet()
        library.updateTags(
            listOf(historyId),
            (selectedTagIds - current).toList(),
            (current - selectedTagIds).toList(),
        )
    }

    fun createTag(name: String) = mutateWorkspace {
        tags.createTag(CreateTagRequest(name))
    }

    fun loadSnapshot(historyId: String, snapshotId: String) {
        viewModelScope.launch {
            runCatching { library.loadSnapshot(historyId, snapshotId) }
                .onSuccess { detail -> mutableState.update { it.copy(snapshotDetail = detail) } }
                .onFailure { mutableState.update { it.copy(operationError = true) } }
        }
    }

    fun closeSnapshot() = mutableState.update { it.copy(snapshotDetail = null) }

    fun exportTranscript(
        destinationUri: String,
        format: TranscriptExportFormat,
        mode: TranscriptExportMode,
    ) {
        val detail = mutableState.value.detail as? LibraryDetailUiState.Ready ?: return
        mutateWorkspace {
            val extension = when (format) {
                TranscriptExportFormat.JSON -> "json"
                TranscriptExportFormat.TXT -> "txt"
                TranscriptExportFormat.SRT -> "srt"
                TranscriptExportFormat.VTT -> "vtt"
                TranscriptExportFormat.MARKDOWN -> "md"
            }
            val path = files.createExportStagingPath("sona-transcript.$extension")
            try {
                exporter.export(TranscriptExportRequest(detail.segments, format, mode, path))
                files.publishExport(path, destinationUri)
            } finally {
                files.cleanup(path)
            }
        }
    }

    private fun updateQuery(transform: HistoryWorkspaceQuery.() -> HistoryWorkspaceQuery) {
        mutableState.update { it.copy(query = it.query.transform().copy(offset = 0), selectedIds = emptySet()) }
        refresh()
    }

    private fun mutateSelection(operation: suspend (List<String>) -> Unit) {
        val ids = mutableState.value.selectedIds.toList()
        if (ids.isEmpty()) return
        mutateWorkspace {
            operation(ids)
            mutableState.update { it.copy(selectedIds = emptySet()) }
        }
    }

    private fun mutateWorkspace(operation: suspend () -> Unit) {
        if (mutableState.value.operationInProgress) return
        mutableState.update { it.copy(operationInProgress = true, operationError = false) }
        viewModelScope.launch {
            try {
                operation()
                refresh()
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                mutableState.update { it.copy(operationError = true) }
            } finally {
                mutableState.update { it.copy(operationInProgress = false) }
            }
        }
    }

    /**
     * Re-transcribes an existing recording through the configured cloud batch
     * provider and republishes the persisted transcript.
     */
    fun transcribeWithCloud(item: HistoryItem) {
        if (mutableState.value.cloudTranscription is CloudTranscriptionUiState.Running) {
            return
        }
        if (item.historyId.isBlank()) return
        mutableState.update {
            it.copy(cloudTranscription = CloudTranscriptionUiState.Running(item.historyId))
        }
        viewModelScope.launch {
            val outcome = try {
                transcribeRecordingWithCloud(
                    CloudTranscriptionRequest(
                        historyId = item.historyId,
                        audioPath = item.audioPath,
                        audioAvailable = item.audioAvailable,
                        isDraft = item.status == HistoryItemStatus.DRAFT,
                    ),
                )
            } catch (error: CancellationException) {
                throw error
            } catch (_: Exception) {
                CloudTranscriptionOutcome.Failed(
                    historyId = item.historyId,
                    reason = CloudTranscriptionFailure.TRANSCRIPTION_FAILED,
                )
            }
            when (outcome) {
                is CloudTranscriptionOutcome.Completed -> {
                    detailGeneration += 1
                    detailJob?.cancel()
                    mutableState.update {
                        it.copy(
                            detail = LibraryDetailUiState.Ready(
                                historyId = outcome.historyId,
                                segments = outcome.segments,
                            ),
                            cloudTranscription = CloudTranscriptionUiState.Completed(
                                outcome.historyId,
                            ),
                        )
                    }
                    refresh()
                }

                is CloudTranscriptionOutcome.Failed -> mutableState.update {
                    it.copy(
                        cloudTranscription = CloudTranscriptionUiState.Failed(
                            historyId = outcome.historyId,
                            reason = outcome.reason,
                        ),
                    )
                }
            }
        }
    }

    companion object {
        internal const val PAGE_SIZE = 30

        fun factory(
            library: HistoryWorkspacePort,
            transcribeRecordingWithCloud: TranscribeRecordingWithCloud,
            scheduleAudioImport: ScheduleAudioImport? = null,
            scheduleAudioRetranscription: ScheduleAudioRetranscription? = null,
            audioImportJobs: AudioImportJobPort = IdleAudioImportJobPort,
            tags: TagWorkspacePort,
            exporter: TranscriptExportPort,
            files: FileTransferPort,
        ): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(LibraryViewModel::class.java))
                    return LibraryViewModel(
                        library,
                        transcribeRecordingWithCloud,
                        scheduleAudioImport,
                        scheduleAudioRetranscription,
                        audioImportJobs,
                        tags,
                        exporter,
                        files,
                    ) as T
                }
            }
    }

    private enum class FailedListOperation {
        REFRESH,
        LOAD_MORE,
    }
}

private object IdleAudioImportJobPort : AudioImportJobPort {
    override val state = flowOf<AudioImportJobState>(AudioImportJobState.Idle)

    override suspend fun enqueue(job: com.sona.android.application.recording.AudioImportJob) = Unit

    override suspend fun cancel(jobId: String) = Unit
}

/**
 * A cloud transcription result belongs to the load that produced it; opening a
 * recording detail again must not inherit an earlier banner. A run in flight
 * keeps reporting itself.
 */
private fun CloudTranscriptionUiState.clearedOnReload(): CloudTranscriptionUiState =
    if (this is CloudTranscriptionUiState.Running) this else CloudTranscriptionUiState.Idle
