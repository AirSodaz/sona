package com.sona.android.adapters.android.recovery

import android.content.Context
import com.sona.android.adapters.android.audio.recoveryPayloadToJob
import com.sona.android.adapters.android.data.isFileWithinRoot
import com.sona.android.application.recording.AudioImportJobPort
import com.sona.android.application.recording.AudioImportJob
import com.sona.android.application.library.HistoryWorkspacePort
import com.sona.android.application.recovery.RecoveryControllerPort
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.recovery.RecoveryUnavailableReason
import com.sona.android.application.recovery.RecoverySource
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class AndroidRecoveryController(
    context: Context,
    private val recovery: RecoveryCoordinator,
    private val jobs: AudioImportJobPort,
    private val unavailableReason: suspend (AudioImportJob) -> RecoveryUnavailableReason? = { null },
    private val transcriptHistory: HistoryWorkspacePort? = null,
) : RecoveryControllerPort {
    private val appContext = context.applicationContext
    private val mutableState = MutableStateFlow(RecoverySnapshot(1, null, emptyList()))
    override val state: StateFlow<RecoverySnapshot> = mutableState.asStateFlow()

    override suspend fun refresh() {
        mutableState.value = recovery.load().decorate()
    }

    override suspend fun resume(itemId: String) {
        val snapshot = recovery.load().decorate()
        val item = snapshot.items.firstOrNull { it.id == itemId } ?: return
        require(item.source == RecoverySource.BATCH_IMPORT) { "This recovery item opens in its feature." }
        require(item.canResume && item.hasSourceFile) { "Recovery source is unavailable." }
        val job = recoveryPayloadToJob(item.payload.orEmpty())
            ?: throw IllegalArgumentException("Recovery payload is invalid.")
        jobs.enqueue(job)
        mutableState.value = recovery.load().decorate()
    }

    override suspend fun resumeAll() {
        val snapshot = recovery.load().decorate()
        snapshot.items
            .filter {
                it.source == RecoverySource.BATCH_IMPORT &&
                    it.resolution == RecoveryResolution.PENDING &&
                    it.canResume
            }
            .forEach { item ->
                recoveryPayloadToJob(item.payload.orEmpty())?.let { jobs.enqueue(it) }
            }
        mutableState.value = recovery.load().decorate()
    }

    override suspend fun discard(itemId: String) {
        val resolved = recovery.resolve(itemId)
        mutableState.value = resolved.snapshot.decorate()
        resolved.item?.let { deleteManagedSource(it.filePath) }
    }

    override suspend fun clearResolved() {
        val snapshot = recovery.load().decorate()
        mutableState.value = recovery.clearResolved()
    }

    private fun deleteManagedSource(path: String) {
        val file = File(path).canonicalFile
        val root = File(appContext.filesDir, "recovery/import-sources").canonicalFile
        if (isFileWithinRoot(file, root)) file.parentFile?.deleteRecursively()
    }

    private suspend fun RecoverySnapshot.decorate(): RecoverySnapshot =
        decorateRecoverySnapshot(this, transcriptHistory, unavailableReason)
}

internal suspend fun decorateRecoverySnapshot(
    snapshot: RecoverySnapshot,
    transcriptHistory: HistoryWorkspacePort? = null,
    unavailableReason: suspend (AudioImportJob) -> RecoveryUnavailableReason?,
): RecoverySnapshot = snapshot.copy(
    items = snapshot.items.map { item ->
        val job = recoveryPayloadToJob(item.payload.orEmpty())
        val reason = when {
            item.source == RecoverySource.AUTOMATION ->
                RecoveryUnavailableReason.AUTOMATION_UNSUPPORTED
            item.source == RecoverySource.TRANSCRIPT_EDIT -> {
                val draft = item.payload?.let(::decodeDraft)
                when {
                    draft == null -> RecoveryUnavailableReason.INVALID_PAYLOAD
                    transcriptHistory == null -> null
                    else -> {
                        val current = runCatching {
                            transcriptHistory.loadTranscript(draft.historyId)
                        }.getOrNull()
                        when {
                            current == null -> RecoveryUnavailableReason.HISTORY_MISSING
                            current != draft.baseSegments -> RecoveryUnavailableReason.TRANSCRIPT_CHANGED
                            else -> null
                        }
                    }
                }
            }
            !item.hasSourceFile -> RecoveryUnavailableReason.SOURCE_MISSING
            job == null -> RecoveryUnavailableReason.INVALID_PAYLOAD
            else -> unavailableReason(job)
        }
        item.copy(canResume = item.canResume && reason == null, unavailableReason = reason)
    },
)
