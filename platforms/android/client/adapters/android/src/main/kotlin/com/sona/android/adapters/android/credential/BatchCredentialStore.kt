package com.sona.android.adapters.android.credential

import com.sona.android.application.recording.OnlineBatchProvider
import kotlinx.coroutines.flow.Flow

/**
 * Stable on-disk identifiers for cloud batch providers. They name key aliases,
 * AAD bindings, and preference keys, so they must never follow enum renames.
 */
internal val OnlineBatchProvider.storageId: String
    get() = when (this) {
        OnlineBatchProvider.VOLCENGINE_DOUBAO -> "volcengine-doubao"
        OnlineBatchProvider.GROQ_WHISPER -> "groq-whisper"
        OnlineBatchProvider.MISTRAL_VOXTRAL -> "mistral-voxtral"
    }

internal fun batchProviderForStorageId(storageId: String?): OnlineBatchProvider? =
    OnlineBatchProvider.entries.firstOrNull { it.storageId == storageId }

/**
 * One encrypted slot per provider plus the non-secret active provider marker.
 */
internal data class BatchCredentialRecords(
    val selectedProviderStorageId: String? = null,
    val slots: Map<String, CredentialRecord> = emptyMap(),
) {
    fun slotFor(storageId: String): CredentialRecord = slots[storageId] ?: CredentialRecord()

    override fun toString(): String =
        "BatchCredentialRecords(selectedProviderStorageId=$selectedProviderStorageId, " +
            "slots=<redacted>)"
}

internal interface BatchCredentialStore {
    val records: Flow<BatchCredentialRecords>

    suspend fun read(): BatchCredentialRecords

    suspend fun writeSlot(storageId: String, record: CredentialRecord)

    suspend fun clearSlot(storageId: String)

    suspend fun writeSelectedProvider(storageId: String)
}
