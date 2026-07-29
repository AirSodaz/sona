package com.sona.android.application.recovery

import kotlinx.coroutines.flow.Flow

enum class RecoverySource { BATCH_IMPORT, AUTOMATION }
enum class RecoveryResolution { PENDING, RESUMED, DISCARDED }
enum class RecoveryStage { QUEUED, TRANSCODING, TRANSCRIBING, SAVING, EXPORTING }
enum class RecoveryUnavailableReason { SOURCE_MISSING, MODEL_MISSING, CREDENTIAL_MISSING, AUTOMATION_UNSUPPORTED, INVALID_PAYLOAD }

data class RecoveryItem(
    val id: String,
    val filename: String,
    val filePath: String,
    val source: RecoverySource,
    val resolution: RecoveryResolution,
    val progress: Double,
    val historyId: String?,
    val historyTitle: String?,
    val stage: RecoveryStage,
    val updatedAtEpochMillis: Long,
    val hasSourceFile: Boolean,
    val canResume: Boolean,
    val payload: String?,
    val unavailableReason: RecoveryUnavailableReason? = null,
)

data class RecoverySnapshot(
    val version: Int,
    val updatedAtEpochMillis: Long?,
    val items: List<RecoveryItem>,
)

data class RecoveryItemInput(
    val id: String,
    val filename: String,
    val filePath: String,
    val resolution: RecoveryResolution = RecoveryResolution.PENDING,
    val progress: Double = 0.0,
    val historyId: String? = null,
    val historyTitle: String? = null,
    val stage: RecoveryStage = RecoveryStage.QUEUED,
    val payload: String,
)

interface RecoveryPort {
    suspend fun load(): RecoverySnapshot
    suspend fun save(items: List<RecoveryItemInput>): RecoverySnapshot
    suspend fun persistQueue(items: List<RecoveryItemInput>, resolvedIds: List<String>): RecoverySnapshot
}

interface RecoveryControllerPort {
    val state: Flow<RecoverySnapshot>
    suspend fun refresh()
    suspend fun resume(itemId: String)
    suspend fun resumeAll()
    suspend fun discard(itemId: String)
    suspend fun clearResolved()
}
