package com.sona.android.adapters.android.recovery

import com.sona.android.application.recording.SpeakerTag
import com.sona.android.application.recording.TranscriptSegment
import com.sona.android.application.recording.TranscriptTiming
import com.sona.android.application.recording.TranscriptTimingLevel
import com.sona.android.application.recording.TranscriptTimingSource
import com.sona.android.application.recording.TranscriptTimingUnit
import com.sona.android.application.recovery.RecoveryItem
import com.sona.android.application.recovery.RecoveryItemInput
import com.sona.android.application.recovery.RecoveryPort
import com.sona.android.application.recovery.RecoveryResolution
import com.sona.android.application.recovery.RecoverySnapshot
import com.sona.android.application.recovery.RecoverySource
import com.sona.android.application.recovery.RecoveryStage
import com.sona.android.application.recovery.TranscriptEditDraft
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class TranscriptEditRecoveryTest {
    @Test
    fun `draft codec preserves structured transcript fields`() {
        val segment = TranscriptSegment(
            id = "segment-1",
            text = "Hello",
            startSeconds = 1.0,
            endSeconds = 2.0,
            isFinal = true,
            timing = TranscriptTiming(
                TranscriptTimingLevel.TOKEN,
                TranscriptTimingSource.MODEL,
                listOf(TranscriptTimingUnit("Hello", 1.0, 2.0)),
            ),
            tokens = listOf("Hello"),
            timestamps = listOf(1f),
            durations = listOf(1f),
            translation = "你好",
            speaker = SpeakerTag("speaker-1", "Speaker 1", "known", 0.9f),
        )
        val draft = TranscriptEditDraft(
            "transcript-edit-history-1",
            "session-1",
            "history-1",
            "Meeting",
            listOf(segment),
            listOf(segment.copy(text = "Hello there")),
        )

        assertEquals(draft, decodeDraft(encodeDraft(draft)))
    }

    @Test
    fun `shared coordinator preserves concurrent audio and transcript items`() = runTest {
        val recovery = FakeRecoveryPort()
        val coordinator = RecoveryCoordinator(recovery)
        val transcript = AndroidTranscriptEditRecoveryAdapter(coordinator)
        val draft = TranscriptEditDraft(
            "transcript-edit-history-1",
            "session-1",
            "history-1",
            "Meeting",
            listOf(segment("base")),
            listOf(segment("draft")),
        )
        val audio = RecoveryItemInput(
            id = "audio-1",
            filename = "audio.wav",
            filePath = "/files/audio.wav",
            payload = "{}",
        )

        listOf(
            async { transcript.save(draft) },
            async { coordinator.upsert(audio) },
        ).awaitAll()

        assertEquals(setOf("audio-1", draft.recoveryId), coordinator.load().items.map { it.id }.toSet())
        assertEquals(draft, transcript.load("history-1"))
        transcript.discard("history-1")
        assertNull(transcript.load("history-1"))
        assertEquals(listOf("audio-1"), coordinator.load().items.map { it.id })
    }

    private fun segment(text: String) = TranscriptSegment(
        id = "segment-1",
        text = text,
        startSeconds = 0.0,
        endSeconds = 1.0,
        isFinal = true,
    )

    private class FakeRecoveryPort : RecoveryPort {
        private var snapshot = RecoverySnapshot(1, null, emptyList())

        override suspend fun load() = snapshot

        override suspend fun save(items: List<RecoveryItemInput>) = persistQueue(items, emptyList())

        override suspend fun persistQueue(
            items: List<RecoveryItemInput>,
            resolvedIds: List<String>,
        ): RecoverySnapshot {
            snapshot = RecoverySnapshot(1, null, items.map { input ->
                RecoveryItem(
                    id = input.id,
                    filename = input.filename,
                    filePath = input.filePath,
                    source = input.source,
                    resolution = RecoveryResolution.PENDING,
                    progress = input.progress,
                    historyId = input.historyId,
                    historyTitle = input.historyTitle,
                    stage = input.stage,
                    updatedAtEpochMillis = 1,
                    hasSourceFile = input.hasSourceFile ?: true,
                    canResume = input.canResume ?: true,
                    payload = input.payload,
                )
            })
            return snapshot
        }
    }
}
