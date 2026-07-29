package com.sona.android.adapters.uniffi.data

import com.sona.android.application.data.TranscriptExportFormat
import com.sona.android.application.data.TranscriptExportMode
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiBackupManifestCountsV1
import uniffi.sona_uniffi_bind.FfiBackupManifestScopesV1
import uniffi.sona_uniffi_bind.FfiBackupManifestV1
import uniffi.sona_uniffi_bind.FfiExportFormatV1
import uniffi.sona_uniffi_bind.FfiExportModeV1

class UniffiDataTransferMappingTest {
    @Test
    fun `maps every transcript export option`() {
        assertEquals(
            listOf(FfiExportFormatV1.JSON, FfiExportFormatV1.TXT, FfiExportFormatV1.SRT, FfiExportFormatV1.VTT, FfiExportFormatV1.MD),
            TranscriptExportFormat.entries.map { it.toFfi() },
        )
        assertEquals(
            listOf(FfiExportModeV1.ORIGINAL, FfiExportModeV1.TRANSLATION, FfiExportModeV1.BILINGUAL),
            TranscriptExportMode.entries.map { it.toFfi() },
        )
    }

    @Test
    fun `maps backup manifest without parsing the archive in Kotlin`() {
        val manifest = ffiManifest(8uL)

        val mapped = manifest.toApplication()

        assertEquals(3, mapped.schemaVersion)
        assertEquals("1.2.3", mapped.appVersion)
        assertEquals(8, mapped.counts.analyticsFiles)
        assertEquals(true, mapped.scopes.history)
    }

    @Test
    fun `rejects backup counts outside Android Long range`() {
        assertThrows(IllegalArgumentException::class.java) {
            ffiManifest(ULong.MAX_VALUE).toApplication()
        }
    }

    private fun ffiManifest(analytics: ULong) = FfiBackupManifestV1(
        3uL,
        "2026-07-29T00:00:00Z",
        "1.2.3",
        "lightweight",
        FfiBackupManifestScopesV1(true, true, true, true, true),
        FfiBackupManifestCountsV1(1uL, 2uL, 3uL, 4uL, 5uL, 6uL, 7uL, analytics),
    )
}
