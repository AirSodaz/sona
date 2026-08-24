package com.sona.android.application.recovery

import kotlinx.coroutines.flow.Flow

enum class RecoverySource { BATCH_IMPORT, AUTOMATION, TRANSCRIPT_EDIT }
enum class RecoveryResolution { PENDING, RESUMED, DISCARDED }
enum class RecoveryStage { QUEUED, TRANSCODING, TRANSCRIBING, SAVING, EXPORTING }
enum class RecoveryUnavailableReason {
    SOURCE_MISSING,
    MODEL_MISSING,
    CREDENTIAL_MISSING,
    AUTOMATION_UNSUPPORTED,
    INVALID_PAYLOAD,
    HISTORY_MISSING,
    TRANSCRIPT_CHANGED,
}

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
    val attemptCount: Int = 0,
    val lastError: com.sona.android.application.recording.AudioImportFailure? = null,
    val retryable: Boolean = false,
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
    val source: RecoverySource = RecoverySource.BATCH_IMPORT,
    val hasSourceFile: Boolean? = null,
    val canResume: Boolean? = null,
    val attemptCount: Int = 0,
    val lastError: com.sona.android.application.recording.AudioImportFailure? = null,
    val retryable: Boolean = false,
)

data class TranscriptEditDraft(
    val recoveryId: String,
    val editSessionId: String,
    val historyId: String,
    val historyTitle: String,
    val baseSegments: List<com.sona.android.application.recording.TranscriptSegment>,
    val draftSegments: List<com.sona.android.application.recording.TranscriptSegment>,
)

interface TranscriptEditRecoveryPort {
    suspend fun load(historyId: String): TranscriptEditDraft?
    suspend fun save(draft: TranscriptEditDraft)
    suspend fun discard(historyId: String)
}

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
