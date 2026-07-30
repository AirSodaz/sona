package com.sona.android.adapters.uniffi.recovery

import com.sona.android.application.recovery.RecoveryItem
import com.sona.android.application.recovery.RecoveryItemInput
import com.sona.android.application.recovery.RecoveryPort
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.recovery.RecoverySource
import com.sona.android.application.recovery.RecoveryStage
import uniffi.sona_uniffi_bind.FfiRecoveredQueueItemV1
import uniffi.sona_uniffi_bind.FfiRecoveryItemInputV1
import uniffi.sona_uniffi_bind.FfiRecoveryItemStageV1
import uniffi.sona_uniffi_bind.FfiRecoveryQueueStatusV1
import uniffi.sona_uniffi_bind.FfiRecoveryResolutionV1
import uniffi.sona_uniffi_bind.FfiRecoverySnapshotV1
import uniffi.sona_uniffi_bind.FfiRecoverySourceV1
import uniffi.sona_uniffi_bind.loadRecoverySnapshotV1
import uniffi.sona_uniffi_bind.persistRecoveryQueueSnapshotV1
import uniffi.sona_uniffi_bind.saveRecoverySnapshotV1

class UniffiRecoveryAdapter(private val appDataDir: String) : RecoveryPort {
    init { require(appDataDir.isNotBlank()) { "Recovery app data directory must not be blank." } }
    override suspend fun load(): RecoverySnapshot = loadRecoverySnapshotV1(appDataDir).toApplication()

    override suspend fun save(items: List<RecoveryItemInput>): RecoverySnapshot =
        saveRecoverySnapshotV1(appDataDir, items.map(RecoveryItemInput::toFfi)).toApplication()

    override suspend fun persistQueue(
        items: List<RecoveryItemInput>,
        resolvedIds: List<String>,
    ): RecoverySnapshot = persistRecoveryQueueSnapshotV1(
        appDataDir,
        items.map(RecoveryItemInput::toFfi),
        resolvedIds,
    ).toApplication()
}

internal fun RecoveryItemInput.toFfi() = FfiRecoveryItemInputV1(
    id = id,
    recoveryId = id,
    filename = filename,
    filePath = filePath,
    source = source.toFfi(),
    origin = source.toFfi(),
    resolution = resolution.toFfi(),
    status = FfiRecoveryQueueStatusV1.PENDING,
    progress = progress,
    segments = emptyList(),
    tagIds = emptyList(),
    projectId = null,
    historyId = historyId,
    historyTitle = historyTitle,
    lastKnownStage = stage.toFfi(),
    updatedAt = null,
    hasSourceFile = hasSourceFile,
    canResume = canResume,
    automationRuleId = null,
    automationRuleName = null,
    resolvedConfigSnapshotJson = payload,
    automationResolutionSnapshotJson = null,
    exportConfigJson = "{}",
    stageConfigJson = "{}",
    sourceFingerprint = null,
    fileStat = null,
    exportFileNamePrefix = null,
)

internal fun FfiRecoverySnapshotV1.toApplication() = RecoverySnapshot(
    version = version.toIntChecked("Recovery version"),
    updatedAtEpochMillis = updatedAt?.toLongChecked("Recovery timestamp"),
    items = items.map(FfiRecoveredQueueItemV1::toApplication),
)

private fun FfiRecoveredQueueItemV1.toApplication() = RecoveryItem(
    id = id,
    filename = filename,
    filePath = filePath,
    source = when (source) {
        FfiRecoverySourceV1.BATCH_IMPORT -> RecoverySource.BATCH_IMPORT
        FfiRecoverySourceV1.AUTOMATION -> RecoverySource.AUTOMATION
        FfiRecoverySourceV1.TRANSCRIPT_EDIT -> RecoverySource.TRANSCRIPT_EDIT
    },
    resolution = when (resolution) {
        FfiRecoveryResolutionV1.PENDING -> RecoveryResolution.PENDING
        FfiRecoveryResolutionV1.RESUMED -> RecoveryResolution.RESUMED
        FfiRecoveryResolutionV1.DISCARDED -> RecoveryResolution.DISCARDED
    },
    progress = progress,
    historyId = historyId,
    historyTitle = historyTitle,
    stage = lastKnownStage.toApplication(),
    updatedAtEpochMillis = updatedAt.toLongChecked("Recovery item timestamp"),
    hasSourceFile = hasSourceFile,
    canResume = canResume,
    payload = resolvedConfigSnapshotJson,
)

private fun RecoveryResolution.toFfi() = when (this) {
    RecoveryResolution.PENDING -> FfiRecoveryResolutionV1.PENDING
    RecoveryResolution.RESUMED -> FfiRecoveryResolutionV1.RESUMED
    RecoveryResolution.DISCARDED -> FfiRecoveryResolutionV1.DISCARDED
}

private fun RecoverySource.toFfi() = when (this) {
    RecoverySource.BATCH_IMPORT -> FfiRecoverySourceV1.BATCH_IMPORT
    RecoverySource.AUTOMATION -> FfiRecoverySourceV1.AUTOMATION
    RecoverySource.TRANSCRIPT_EDIT -> FfiRecoverySourceV1.TRANSCRIPT_EDIT
}

private fun RecoveryStage.toFfi() = when (this) {
    RecoveryStage.QUEUED, RecoveryStage.TRANSCODING -> FfiRecoveryItemStageV1.QUEUED
    RecoveryStage.TRANSCRIBING -> FfiRecoveryItemStageV1.TRANSCRIBING
    RecoveryStage.SAVING, RecoveryStage.EXPORTING -> FfiRecoveryItemStageV1.EXPORTING
}

private fun FfiRecoveryItemStageV1.toApplication() = when (this) {
    FfiRecoveryItemStageV1.QUEUED -> RecoveryStage.QUEUED
    FfiRecoveryItemStageV1.TRANSCRIBING -> RecoveryStage.TRANSCRIBING
    FfiRecoveryItemStageV1.POLISHING, FfiRecoveryItemStageV1.TRANSLATING -> RecoveryStage.TRANSCRIBING
    FfiRecoveryItemStageV1.EXPORTING -> RecoveryStage.EXPORTING
}

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}

private fun UInt.toIntChecked(label: String): Int {
    require(this <= Int.MAX_VALUE.toUInt()) { "$label exceeds the Android Int range." }
    return toInt()
}
