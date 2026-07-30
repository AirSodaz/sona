package com.sona.android.adapters.android.recovery

import com.sona.android.application.recovery.RecoveryItem
import com.sona.android.application.recovery.RecoveryItemInput
import com.sona.android.application.recovery.RecoveryPort
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySnapshot
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/** Serializes Recovery snapshot read-modify-write operations across Android features. */
class RecoveryCoordinator(private val recovery: RecoveryPort) {
    private val mutex = Mutex()

    suspend fun load(): RecoverySnapshot = mutex.withLock { recovery.load() }

    suspend fun upsert(input: RecoveryItemInput): RecoverySnapshot = mutex.withLock {
        val pending = recovery.load().items
            .filter { it.resolution == RecoveryResolution.PENDING && it.id != input.id }
            .map(RecoveryItem::toInput)
        recovery.persistQueue(pending + input, emptyList())
    }

    suspend fun resolve(itemId: String): ResolvedRecoveryItem = mutex.withLock {
        val snapshot = recovery.load()
        val resolved = snapshot.items.firstOrNull { it.id == itemId }
        val updated = recovery.persistQueue(
            snapshot.items
                .filter { it.resolution == RecoveryResolution.PENDING && it.id != itemId }
                .map(RecoveryItem::toInput),
            listOf(itemId),
        )
        ResolvedRecoveryItem(updated, resolved)
    }

    suspend fun replacePending(
        items: List<RecoveryItemInput>,
        resolvedIds: List<String> = emptyList(),
    ): RecoverySnapshot = mutex.withLock {
        recovery.persistQueue(items, resolvedIds)
    }

    suspend fun clearResolved(): RecoverySnapshot = mutex.withLock {
        val pending = recovery.load().items
            .filter { it.resolution == RecoveryResolution.PENDING }
            .map(RecoveryItem::toInput)
        recovery.save(pending)
    }
}

data class ResolvedRecoveryItem(
    val snapshot: RecoverySnapshot,
    val item: RecoveryItem?,
)

internal fun RecoveryItem.toInput() = RecoveryItemInput(
    id = id,
    filename = filename,
    filePath = filePath,
    resolution = resolution,
    progress = progress,
    historyId = historyId,
    historyTitle = historyTitle,
    stage = stage,
    payload = payload.orEmpty(),
    source = source,
    hasSourceFile = hasSourceFile,
    canResume = canResume,
)
