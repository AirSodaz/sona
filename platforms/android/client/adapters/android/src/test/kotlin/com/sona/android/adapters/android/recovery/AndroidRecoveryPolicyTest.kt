package com.sona.android.adapters.android.recovery

import com.sona.android.adapters.android.audio.toRecoveryPayload
import com.sona.android.application.recording.AudioImportEngine
import com.sona.android.application.recording.AudioImportJob
import com.sona.android.application.recording.AudioImportSource
import com.sona.android.application.recording.AudioImportTarget
import com.sona.android.application.recovery.RecoveryItem
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.recovery.RecoverySource
import com.sona.android.application.recovery.RecoveryStage
import com.sona.android.application.recovery.RecoveryUnavailableReason
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class AndroidRecoveryPolicyTest {
    @Test
    fun `missing source remains visible and cannot resume`() = runTest {
        val mapped = decorateRecoverySnapshot(snapshot(hasSource = false, payload = job().toRecoveryPayload())) { null }

        assertEquals(RecoveryUnavailableReason.SOURCE_MISSING, mapped.items.single().unavailableReason)
        assertFalse(mapped.items.single().canResume)
    }

    @Test
    fun `missing model or credential reason is retained`() = runTest {
        val mapped = decorateRecoverySnapshot(snapshot(hasSource = true, payload = job().toRecoveryPayload())) {
            RecoveryUnavailableReason.MODEL_MISSING
        }

        assertEquals(RecoveryUnavailableReason.MODEL_MISSING, mapped.items.single().unavailableReason)
        assertFalse(mapped.items.single().canResume)
    }

    @Test
    fun `unsupported payload remains pending`() = runTest {
        val mapped = decorateRecoverySnapshot(snapshot(hasSource = true, payload = "{}")) { null }

        assertEquals(RecoveryResolution.PENDING, mapped.items.single().resolution)
        assertEquals(RecoveryUnavailableReason.INVALID_PAYLOAD, mapped.items.single().unavailableReason)
    }

    private fun job() = AudioImportJob(
        "job-1",
        AudioImportTarget.NewImport(AudioImportSource("/files/source.wav")),
        AudioImportEngine.Local("model-1"),
    )

    private fun snapshot(hasSource: Boolean, payload: String) = RecoverySnapshot(
        1,
        null,
        listOf(
            RecoveryItem(
                "job-1",
                "Meeting.wav",
                "/files/source.wav",
                RecoverySource.BATCH_IMPORT,
                RecoveryResolution.PENDING,
                0.5,
                null,
                null,
                RecoveryStage.TRANSCRIBING,
                1,
                hasSource,
                true,
                payload,
            ),
        ),
    )
}
