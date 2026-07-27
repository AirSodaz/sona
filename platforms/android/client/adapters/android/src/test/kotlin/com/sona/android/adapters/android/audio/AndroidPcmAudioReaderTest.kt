package com.sona.android.adapters.android.audio

import com.sona.android.adapters.android.wav.CheckpointingWavWriter
import com.sona.android.application.recording.RecordingDestination
import java.io.File
import kotlinx.coroutines.flow.toList
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class AndroidPcmAudioReaderTest {
    @Test
    fun `reads normalized wav as even pcm frames`() = runTest {
        val file = temporaryWav()
        val pcm = ByteArray(3_202) { (it % 127).toByte() }
        CheckpointingWavWriter.open(RecordingDestination(file.absolutePath)).use { writer ->
            writer.write(pcm, 0, pcm.size)
        }

        val frames = AndroidPcmAudioReader().readFrames(file.absolutePath).toList()

        assertEqualsBytes(pcm.copyOfRange(0, 3_200), frames[0].bytes)
        assertEqualsBytes(pcm.copyOfRange(3_200, 3_202), frames[1].bytes)
        file.delete()
    }

    @Test
    fun `validates normalized wav layout and declared data size`() {
        val file = temporaryWav()
        CheckpointingWavWriter.open(RecordingDestination(file.absolutePath)).use { writer ->
            writer.write(byteArrayOf(1, 2), 0, 2)
        }
        val header = file.inputStream().use { input -> ByteArray(44).also(input::read) }

        assertTrue(isNormalizedWav(header, file.length()))
        assertFalse(isNormalizedWav(header, file.length() + 2))
        file.delete()
    }

    private fun temporaryWav(): File = File.createTempFile("sona-import-", ".wav")

    private fun assertEqualsBytes(expected: ByteArray, actual: ByteArray) {
        assertArrayEquals(expected, actual)
    }
}
