package com.sona.android.adapters.uniffi.recovery

import com.sona.android.application.recovery.RecoveryItemInput
import com.sona.android.application.recovery.RecoveryStage
import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiRecoveryItemStageV1
import uniffi.sona_uniffi_bind.FfiRecoverySnapshotV1

class UniffiRecoveryMappingTest {
    @Test
    fun `maps Android audio import payload into typed recovery input`() {
        val payload = "{\"androidAudioImportV1\":{\"id\":\"job-1\"}}"
        val input = RecoveryItemInput(
            id = "job-1",
            filename = "Meeting.m4a",
            filePath = "/files/recovery/import-sources/job-1/source.m4a",
            stage = RecoveryStage.TRANSCRIBING,
            payload = payload,
        ).toFfi()

        assertEquals("job-1", input.recoveryId)
        assertEquals(FfiRecoveryItemStageV1.TRANSCRIBING, input.lastKnownStage)
        assertEquals(payload, input.resolvedConfigSnapshotJson)
    }

    @Test
    fun `rejects recovery version outside Android Int range`() {
        assertThrows(IllegalArgumentException::class.java) {
            FfiRecoverySnapshotV1(UInt.MAX_VALUE, null, emptyList()).toApplication()
        }
    }
}
