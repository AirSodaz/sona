package com.sona.android.app.feature.bootstrap

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import com.sona.android.application.bootstrap.LoadSonaBootstrap
import com.sona.android.application.bootstrap.SonaBootstrapSnapshot
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

sealed interface SonaBootstrapUiState {
    data object Loading : SonaBootstrapUiState

    data class Ready(
        val snapshot: SonaBootstrapSnapshot,
    ) : SonaBootstrapUiState

    data class Error(
        val message: String,
    ) : SonaBootstrapUiState
}

class SonaBootstrapViewModel(
    private val loadSonaBootstrap: LoadSonaBootstrap,
) : ViewModel() {
    private val mutableBootstrapState = MutableStateFlow<SonaBootstrapUiState>(
        SonaBootstrapUiState.Loading,
    )
    val bootstrapState: StateFlow<SonaBootstrapUiState> = mutableBootstrapState.asStateFlow()

    init {
        refresh()
    }

    fun refresh() {
        mutableBootstrapState.value = SonaBootstrapUiState.Loading
        viewModelScope.launch(Dispatchers.Default) {
            mutableBootstrapState.value = try {
                SonaBootstrapUiState.Ready(loadSonaBootstrap())
            } catch (error: LinkageError) {
                SonaBootstrapUiState.Error(error.message.orEmpty())
            } catch (error: Exception) {
                SonaBootstrapUiState.Error(error.message.orEmpty())
            }
        }
    }

    companion object {
        fun factory(loadSonaBootstrap: LoadSonaBootstrap): ViewModelProvider.Factory =
            object : ViewModelProvider.Factory {
                @Suppress("UNCHECKED_CAST")
                override fun <T : ViewModel> create(modelClass: Class<T>): T {
                    require(modelClass.isAssignableFrom(SonaBootstrapViewModel::class.java))
                    return SonaBootstrapViewModel(loadSonaBootstrap) as T
                }
            }
    }
}
