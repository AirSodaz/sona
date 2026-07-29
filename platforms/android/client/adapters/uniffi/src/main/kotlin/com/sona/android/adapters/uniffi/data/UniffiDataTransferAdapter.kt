package com.sona.android.adapters.uniffi.data

import com.sona.android.application.data.BackupApplyResult
import com.sona.android.application.data.BackupCounts
import com.sona.android.application.data.BackupManifest
import com.sona.android.application.data.BackupPort
import com.sona.android.application.data.BackupScopes
import com.sona.android.application.data.PreparedBackupImport
import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import com.sona.android.application.data.TranscriptExportPort
import com.sona.android.application.data.TranscriptExportRequest
import com.sona.android.application.data.TranscriptExportResult
import com.sona.android.adapters.uniffi.recording.toFfi
import uniffi.sona_uniffi_bind.FfiBackupManifestV1
import uniffi.sona_uniffi_bind.FfiExportFormatV1
import uniffi.sona_uniffi_bind.FfiExportModeV1
import uniffi.sona_uniffi_bind.FfiExportTranscriptFileRequestV1
import uniffi.sona_uniffi_bind.exportBackupArchiveV1
import uniffi.sona_uniffi_bind.exportTranscriptFileV1
import uniffi.sona_uniffi_bind.importBackupArchiveV1
import uniffi.sona_uniffi_bind.inspectBackupArchiveV1
import uniffi.sona_uniffi_bind.releaseApplicationContext

class UniffiTranscriptExportAdapter : TranscriptExportPort {
    override suspend fun export(request: TranscriptExportRequest): TranscriptExportResult {
        require(request.outputPath.isNotBlank()) { "Export output path must not be blank." }
        val result = exportTranscriptFileV1(
            FfiExportTranscriptFileRequestV1(
                segments = request.segments.map { it.toFfi() },
                format = request.format.toFfi(),
                mode = request.mode.toFfi(),
                outputPath = request.outputPath,
            ),
        )
        return TranscriptExportResult(result.outputPath, result.bytesWritten.toLongChecked("Export size"))
    }
}

class UniffiBackupAdapter(private val appDataDir: String) : BackupPort {
    init { require(appDataDir.isNotBlank()) { "Backup app data directory must not be blank." } }

    override suspend fun exportBackup(archivePath: String, appVersion: String): BackupManifest =
        exportBackupArchiveV1(
            appDataDir,
            archivePath.requireNotBlank("Backup archive path"),
            appVersion.requireNotBlank("Backup app version"),
        ).toApplication()

    override suspend fun inspectBackup(archivePath: String): PreparedBackupImport =
        inspectBackupArchiveV1(archivePath.requireNotBlank("Backup archive path")).let {
            PreparedBackupImport(it.importId, it.archivePath, it.manifest.toApplication())
        }

    override suspend fun importBackup(
        archivePath: String,
        defaultRuleSetName: String,
        confirmReplace: Boolean,
    ): BackupApplyResult = importBackupArchiveV1(
        appDataDir,
        archivePath,
        defaultRuleSetName,
        confirmReplace,
    ).let { BackupApplyResult(it.importId, it.manifest.toApplication()) }

    override fun releaseApplicationContext(): Boolean = releaseApplicationContext(appDataDir)
}

internal fun TranscriptExportFormat.toFfi() = when (this) {
    TranscriptExportFormat.JSON -> FfiExportFormatV1.JSON
    TranscriptExportFormat.TXT -> FfiExportFormatV1.TXT
    TranscriptExportFormat.SRT -> FfiExportFormatV1.SRT
    TranscriptExportFormat.VTT -> FfiExportFormatV1.VTT
    TranscriptExportFormat.MARKDOWN -> FfiExportFormatV1.MD
}

internal fun TranscriptExportMode.toFfi() = when (this) {
    TranscriptExportMode.ORIGINAL -> FfiExportModeV1.ORIGINAL
    TranscriptExportMode.TRANSLATION -> FfiExportModeV1.TRANSLATION
    TranscriptExportMode.BILINGUAL -> FfiExportModeV1.BILINGUAL
}

internal fun FfiBackupManifestV1.toApplication() = BackupManifest(
    schemaVersion = schemaVersion.toLongChecked("Backup schema version"),
    createdAt = createdAt,
    appVersion = appVersion,
    historyMode = historyMode,
    scopes = BackupScopes(scopes.config, scopes.workspace, scopes.history, scopes.automation, scopes.analytics),
    counts = BackupCounts(
        tags = counts.tags.toLongChecked("Tag count"),
        historyItems = counts.historyItems.toLongChecked("History count"),
        transcriptFiles = counts.transcriptFiles.toLongChecked("Transcript count"),
        summaryFiles = counts.summaryFiles.toLongChecked("Summary count"),
        automationProfiles = counts.automationProfiles.toLongChecked("Automation profile count"),
        automationRules = counts.automationRules.toLongChecked("Automation rule count"),
        automationProcessedEntries = counts.automationProcessedEntries.toLongChecked("Automation processed count"),
        analyticsFiles = counts.analyticsFiles.toLongChecked("Analytics count"),
    ),
)

private fun ULong.toLongChecked(label: String): Long {
    require(this <= Long.MAX_VALUE.toULong()) { "$label exceeds the Android Long range." }
    return toLong()
}

private fun String.requireNotBlank(label: String): String = also {
    require(it.isNotBlank()) { "$label must not be blank." }
}
