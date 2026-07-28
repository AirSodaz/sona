package com.sona.android.adapters.android.audio

import com.sona.android.application.recording.AudioImportEngine
import com.sona.android.application.recording.AudioImportJob
import com.sona.android.application.recording.AudioImportSource
import com.sona.android.application.recording.AudioImportTarget
import com.sona.android.application.recording.OnlineAsrProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class AudioImportWorkDataTest {
    @Test
    fun `round trips new import without credentials`() {
        val job = AudioImportJob(
            id = "job-1",
            target = AudioImportTarget.NewImport(AudioImportSource("content://audio/1")),
            engine = AudioImportEngine.Online(OnlineAsrProvider.GROQ_WHISPER),
        )

        val data = job.toWorkData("Meeting.m4a")

        assertEquals(job, data.toAudioImportJob())
        assertFalse(data.keyValueMap.values.any { it == "secret" })
    }

    @Test
    fun `round trips existing local retranscription`() {
        val job = AudioImportJob(
            id = "job-2",
            target = AudioImportTarget.ExistingRecording(
                historyId = "history-1",
                audioPath = "/history/one.wav",
                displayName = "Meeting",
                durationMillis = 9_000,
            ),
            engine = AudioImportEngine.Local("model-1"),
        )

        assertEquals(job, job.toWorkData("Meeting").toAudioImportJob())
    }
}
