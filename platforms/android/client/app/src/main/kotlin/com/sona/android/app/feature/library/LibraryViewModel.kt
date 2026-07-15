package com.sona.android.app.feature.library

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.library.RecordingLibraryItem
import com.sona.android.application.library.RecordingLibraryPort
import com.sona.android.application.recording.TranscriptSegment
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
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

data class LibraryUiState(
    val items: List<RecordingLibraryItem> = emptyList(),
    val hasMore: Boolean = false,
    val isInitialLoading: Boolean = false,
    val isRefreshing: Boolean = false,
    val isLoadingMore: Boolean = false,
    val listError: LibraryListError? = null,
    val detail: LibraryDetailUiState = LibraryDetailUiState.None,
)

class LibraryViewModel(
    private val library: RecordingLibraryPort,
) : ViewModel() {
    private val mutableState = MutableStateFlow(LibraryUiState())
    val state: StateFlow<LibraryUiState> = mutableState.asStateFlow()

    private var listJob: Job? = null
    private var detailJob: Job? = null
    private var failedListOperation: FailedListOperation? = null
    private var nextOffset: Int = 0
    private var listGeneration: Int = 0
    private var detailGeneration: Int = 0

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
                val page = library.loadPage(offset = 0, limit = PAGE_SIZE)
                if (generation != listGeneration) return@launch
                nextOffset = page.items.size
                failedListOperation = null
                mutableState.update {
                    it.copy(
                        items = page.items.distinctBy(RecordingLibraryItem::historyId),
                        hasMore = page.hasMore && page.items.isNotEmpty(),
                        isInitialLoading = false,
                        isRefreshing = false,
                        listError = null,
                    )
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
                    val page = library.loadPage(offset = pageOffset, limit = PAGE_SIZE)
                    if (generation != listGeneration) return@launch
                    val existingIds = mutableState.value.items
                        .mapTo(mutableSetOf(), RecordingLibraryItem::historyId)
                    val containsNewItem = page.items.any { it.historyId !in existingIds }
                    val pageHasMore = page.hasMore && page.items.isNotEmpty()
                    nextOffset = pageOffset + page.items.size
                    if (containsNewItem || !pageHasMore) {
                        failedListOperation = null
                        mutableState.update { state ->
                            state.copy(
                                items = (state.items + page.items)
                                    .distinctBy(RecordingLibraryItem::historyId),
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
            it.copy(detail = LibraryDetailUiState.Loading(historyId))
        }
        detailJob = viewModelScope.launch {
            try {
                val segments = library.loadTranscript(historyId)
                if (generation != detailGeneration) return@launch
                mutableState.update {
                    it.copy(detail = LibraryDetailUiState.Ready(historyId, segments))
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

    companion object {
        internal const val PAGE_SIZE = 30

        fun factory(library: RecordingLibraryPort): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(LibraryViewModel::class.java))
                    return LibraryViewModel(library) as T
                }
            }
    }

    private enum class FailedListOperation {
        REFRESH,
        LOAD_MORE,
    }
}
