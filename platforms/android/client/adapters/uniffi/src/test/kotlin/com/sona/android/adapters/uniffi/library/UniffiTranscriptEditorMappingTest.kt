package com.sona.android.adapters.uniffi.library

import com.sona.android.application.library.TranscriptEditOperation
import com.sona.android.application.library.TranscriptSnapshotReason
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.sona_uniffi_bind.FfiTranscriptEditOperationV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotMetadataV1
import uniffi.sona_uniffi_bind.FfiTranscriptSnapshotReasonV1

class UniffiTranscriptEditorMappingTest {
    @Test
    fun `all edit operation payloads remain typed`() {
        val update = TranscriptEditOperation.UpdateText("segment-1", "Edited").toFfi()
        assertTrue(update is FfiTranscriptEditOperationV1.UpdateText)
        assertEquals("Edited", (update as FfiTranscriptEditOperationV1.UpdateText).text)

        val split = TranscriptEditOperation.Split(
            "segment-1", "segment-2", "left", "right", "左", "右",
        ).toFfi()
        assertTrue(split is FfiTranscriptEditOperationV1.Split)
        split as FfiTranscriptEditOperationV1.Split
        assertEquals("segment-2", split.newSegmentId)
        assertEquals("右", split.rightTranslation)
    }

    @Test
    fun `manual edit snapshot reason maps to application`() {
        val mapped = FfiTranscriptSnapshotMetadataV1(
            id = "snapshot-1",
            historyId = "history-1",
            reason = FfiTranscriptSnapshotReasonV1.MANUAL_EDIT,
            createdAt = 10u,
            segmentCount = 2u,
        ).toApplication()

        assertEquals(TranscriptSnapshotReason.MANUAL_EDIT, mapped.reason)
        assertEquals(2L, mapped.segmentCount)
    }
}
