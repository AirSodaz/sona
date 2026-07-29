package com.sona.android.application.data

import com.sona.android.application.recording.TranscriptSegment

enum class TranscriptExportFormat { JSON, TXT, SRT, VTT, MARKDOWN }
enum class TranscriptExportMode { ORIGINAL, TRANSLATION, BILINGUAL }

data class TranscriptExportRequest(
    val segments: List<TranscriptSegment>,
    val format: TranscriptExportFormat,
    val mode: TranscriptExportMode,
    val outputPath: String,
)

data class TranscriptExportResult(val outputPath: String, val bytesWritten: Long)

fun interface TranscriptExportPort {
    suspend fun export(request: TranscriptExportRequest): TranscriptExportResult
}

data class BackupScopes(
    val config: Boolean,
    val workspace: Boolean,
    val history: Boolean,
    val automation: Boolean,
    val analytics: Boolean,
)

data class BackupCounts(
    val tags: Long,
    val historyItems: Long,
    val transcriptFiles: Long,
    val summaryFiles: Long,
    val automationProfiles: Long,
    val automationRules: Long,
    val automationProcessedEntries: Long,
    val analyticsFiles: Long,
)

data class BackupManifest(
    val schemaVersion: Long,
    val createdAt: String,
    val appVersion: String,
    val historyMode: String,
    val scopes: BackupScopes,
    val counts: BackupCounts,
)

data class PreparedBackupImport(
    val importId: String,
    val archivePath: String,
    val manifest: BackupManifest,
)

data class BackupApplyResult(val importId: String, val manifest: BackupManifest)

interface BackupPort {
    suspend fun exportBackup(archivePath: String, appVersion: String): BackupManifest
    suspend fun inspectBackup(archivePath: String): PreparedBackupImport
    suspend fun importBackup(
        archivePath: String,
        defaultRuleSetName: String,
        confirmReplace: Boolean,
    ): BackupApplyResult

    fun releaseApplicationContext(): Boolean
}

enum class DataTransferBlocker { LIVE_RECORDING, AUDIO_IMPORT, SYNC, RECOVERY }

interface FileTransferPort {
    suspend fun stageImport(sourceUri: String): String
    suspend fun publishExport(stagedPath: String, destinationUri: String)
    suspend fun createExportStagingPath(fileName: String): String
    suspend fun cleanup(path: String)
    suspend fun publishText(text: String, destinationUri: String)
}
