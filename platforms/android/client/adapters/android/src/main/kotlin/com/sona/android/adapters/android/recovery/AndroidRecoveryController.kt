package com.sona.android.adapters.android.recovery

import android.content.Context
import com.sona.android.adapters.android.audio.recoveryPayloadToJob
import com.sona.android.adapters.android.audio.toInput
import com.sona.android.adapters.android.data.isFileWithinRoot
import com.sona.android.application.recording.AudioImportJobPort
import com.sona.android.application.recording.AudioImportJob
import com.sona.android.application.recovery.RecoveryControllerPort
import com.sona.android.application.recovery.RecoveryPort
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.recovery.RecoveryUnavailableReason
import java.io.File
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow

class AndroidRecoveryController(
    context: Context,
    private val recovery: RecoveryPort,
    private val jobs: AudioImportJobPort,
    private val unavailableReason: suspend (AudioImportJob) -> RecoveryUnavailableReason? = { null },
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
        require(item.source == com.sona.android.application.recovery.RecoverySource.BATCH_IMPORT) {
            "Automation recovery is unavailable on Android."
        }
        require(item.canResume && item.hasSourceFile) { "Recovery source is unavailable." }
        val job = recoveryPayloadToJob(item.payload.orEmpty())
            ?: throw IllegalArgumentException("Recovery payload is invalid.")
        jobs.enqueue(job)
        mutableState.value = recovery.load().decorate()
    }

    override suspend fun resumeAll() {
        val snapshot = recovery.load().decorate()
        snapshot.items
            .filter { it.resolution == RecoveryResolution.PENDING && it.canResume }
            .forEach { item ->
                recoveryPayloadToJob(item.payload.orEmpty())?.let { jobs.enqueue(it) }
            }
        mutableState.value = recovery.load().decorate()
    }

    override suspend fun discard(itemId: String) {
        val snapshot = recovery.load().decorate()
        val item = snapshot.items.firstOrNull { it.id == itemId } ?: return
        mutableState.value = recovery.persistQueue(
            snapshot.items
                .filter { it.resolution == RecoveryResolution.PENDING && it.id != itemId }
                .map { it.toInput() },
            listOf(itemId),
        )
        deleteManagedSource(item.filePath)
    }

    override suspend fun clearResolved() {
        val snapshot = recovery.load().decorate()
        mutableState.value = recovery.save(
            snapshot.items.filter { it.resolution == RecoveryResolution.PENDING }.map { it.toInput() },
        )
    }

    private fun deleteManagedSource(path: String) {
        val file = File(path).canonicalFile
        val root = File(appContext.filesDir, "recovery/import-sources").canonicalFile
        if (isFileWithinRoot(file, root)) file.parentFile?.deleteRecursively()
    }

    private suspend fun RecoverySnapshot.decorate(): RecoverySnapshot =
        decorateRecoverySnapshot(this, unavailableReason)
}

internal suspend fun decorateRecoverySnapshot(
    snapshot: RecoverySnapshot,
    unavailableReason: suspend (AudioImportJob) -> RecoveryUnavailableReason?,
): RecoverySnapshot = snapshot.copy(
    items = snapshot.items.map { item ->
        val job = recoveryPayloadToJob(item.payload.orEmpty())
        val reason = when {
            item.source == com.sona.android.application.recovery.RecoverySource.AUTOMATION ->
                RecoveryUnavailableReason.AUTOMATION_UNSUPPORTED
            !item.hasSourceFile -> RecoveryUnavailableReason.SOURCE_MISSING
            job == null -> RecoveryUnavailableReason.INVALID_PAYLOAD
            else -> unavailableReason(job)
        }
        item.copy(canResume = item.canResume && reason == null, unavailableReason = reason)
    },
)
