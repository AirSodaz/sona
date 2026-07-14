package com.sona.android.adapters.android.wav

import com.sona.android.application.recording.RecordingDestination
import java.io.IOException
import java.nio.file.Files
import java.nio.file.Path
import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test

class CheckpointingWavWriterTest {
    private val temporaryFiles = mutableListOf<Path>()

    @After
    fun deleteTemporaryFiles() {
        temporaryFiles.forEach(Files::deleteIfExists)
    }

    @Test
    fun `new writer stores an empty 44 byte PCM WAV header`() {
        val destination = temporaryFile()
        val writer = CheckpointingWavWriter.open(RecordingDestination(destination.toString()))

        val bytes = Files.readAllBytes(destination)

        assertEquals(44, bytes.size)
        assertEquals("RIFF", bytes.ascii(0, 4))
        assertEquals(36, bytes.littleEndianInt(4))
        assertEquals("WAVE", bytes.ascii(8, 4))
        assertEquals("fmt ", bytes.ascii(12, 4))
        assertEquals(16, bytes.littleEndianInt(16))
        assertEquals(1, bytes.littleEndianShort(20))
        assertEquals(1, bytes.littleEndianShort(22))
        assertEquals(16_000, bytes.littleEndianInt(24))
        assertEquals(32_000, bytes.littleEndianInt(28))
        assertEquals(2, bytes.littleEndianShort(32))
        assertEquals(16, bytes.littleEndianShort(34))
        assertEquals("data", bytes.ascii(36, 4))
        assertEquals(0, bytes.littleEndianInt(40))

        writer.close()
    }

    @Test
    fun `one second checkpoint updates both lengths before finish`() {
        val destination = temporaryFile()
        val writer = CheckpointingWavWriter.open(RecordingDestination(destination.toString()))
        val pcm = ByteArray(32_000) { index -> (index % 251).toByte() }

        writer.write(pcm, 0, pcm.size)

        val bytes = Files.readAllBytes(destination)
        assertEquals(32_044, bytes.size)
        assertEquals(32_036, bytes.littleEndianInt(4))
        assertEquals(32_000, bytes.littleEndianInt(40))
        assertArrayEquals(pcm, bytes.copyOfRange(44, bytes.size))
        writer.close()
    }

    @Test
    fun `each full second checkpoints and syncs the file`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)

        writer.write(ByteArray(64_000), 0, 64_000)

        assertEquals(listOf(32_000L, 64_000L), file.syncedDataLengths)
        writer.close()
    }

    @Test
    fun `finish finalizes partial PCM and derives duration with integer arithmetic`() {
        val destination = temporaryFile()
        val writer = CheckpointingWavWriter.open(RecordingDestination(destination.toString()))
        val pcm = ByteArray(9_600) { 0x2a }
        writer.write(pcm, 0, pcm.size)

        val captured = writer.finish()
        val bytes = Files.readAllBytes(destination)

        assertEquals(9_600L, captured.bytesWritten)
        assertEquals(300L, captured.durationMillis)
        assertEquals(9_636, bytes.littleEndianInt(4))
        assertEquals(9_600, bytes.littleEndianInt(40))
        assertArrayEquals(pcm, bytes.copyOfRange(44, bytes.size))
        writer.close()
    }

    @Test
    fun `finish is idempotent`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)
        writer.write(ByteArray(320), 0, 320)

        val first = writer.finish()
        val operationsAfterFirstFinish = file.operationCount
        val second = writer.finish()

        assertEquals(first, second)
        assertEquals(operationsAfterFirstFinish, file.operationCount)
        writer.close()
    }

    @Test
    fun `close finalizes partial PCM and is idempotent`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)
        writer.write(ByteArray(320), 0, 320)

        writer.close()
        val operationsAfterFirstClose = file.operationCount
        writer.close()

        assertEquals(356, file.bytes.littleEndianInt(4))
        assertEquals(320, file.bytes.littleEndianInt(40))
        assertEquals(1, file.closeCalls)
        assertEquals(operationsAfterFirstClose, file.operationCount)
    }

    @Test
    fun `write failure is redacted`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)
        file.failOn = FileOperation.WRITE

        val error = assertThrows(IOException::class.java) {
            writer.write(ByteArray(2), 0, 2)
        }

        assertRedacted(error)
    }

    @Test
    fun `checkpoint seek failure is redacted`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)
        file.failOn = FileOperation.SEEK

        val error = assertThrows(IOException::class.java) {
            writer.write(ByteArray(32_000), 0, 32_000)
        }

        assertRedacted(error)
    }

    @Test
    fun `checkpoint sync failure is redacted`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)
        file.failOn = FileOperation.SYNC

        val error = assertThrows(IOException::class.java) {
            writer.write(ByteArray(32_000), 0, 32_000)
        }

        assertRedacted(error)
    }

    @Test
    fun `finalization failure is redacted`() {
        val file = InMemoryRandomAccessWavFile()
        val writer = CheckpointingWavWriter(file)
        writer.write(ByteArray(320), 0, 320)
        file.failOn = FileOperation.WRITE

        val error = assertThrows(IOException::class.java) {
            writer.finish()
        }

        assertRedacted(error)
    }

    private fun temporaryFile(): Path =
        Files.createTempFile("sona-wav-writer-", ".wav").also(temporaryFiles::add)

    private fun assertRedacted(error: IOException) {
        assertEquals("Unable to write recording audio.", error.message)
        assertFalse(error.toString().contains(InMemoryRandomAccessWavFile.SECRET_PATH))
        assertNull(error.cause)
    }
}

private enum class FileOperation {
    WRITE,
    SEEK,
    SYNC,
}

private class InMemoryRandomAccessWavFile : RandomAccessWavFile {
    companion object {
        const val SECRET_PATH = "C:\\Users\\private\\recording.wav"
    }

    var bytes = ByteArray(0)
        private set
    var failOn: FileOperation? = null
    var operationCount = 0
        private set
    var closeCalls = 0
        private set
    val syncedDataLengths = mutableListOf<Long>()
    private var position = 0

    override fun setLength(length: Long) {
        operationCount += 1
        bytes = bytes.copyOf(length.toInt())
        position = position.coerceAtMost(bytes.size)
    }

    override fun seek(position: Long) {
        operationCount += 1
        failIfRequested(FileOperation.SEEK)
        this.position = position.toInt()
    }

    override fun write(bytes: ByteArray, offset: Int, length: Int) {
        operationCount += 1
        failIfRequested(FileOperation.WRITE)
        val end = position + length
        if (end > this.bytes.size) {
            this.bytes = this.bytes.copyOf(end)
        }
        bytes.copyInto(this.bytes, position, offset, offset + length)
        position = end
    }

    override fun sync() {
        operationCount += 1
        failIfRequested(FileOperation.SYNC)
        syncedDataLengths += bytes.littleEndianInt(40).toLong()
    }

    override fun close() {
        operationCount += 1
        closeCalls += 1
    }

    private fun failIfRequested(operation: FileOperation) {
        if (failOn == operation) {
            throw IOException("operation failed for $SECRET_PATH")
        }
    }
}

private fun ByteArray.ascii(offset: Int, length: Int): String =
    copyOfRange(offset, offset + length).toString(Charsets.US_ASCII)

private fun ByteArray.littleEndianShort(offset: Int): Int =
    (this[offset].toInt() and 0xff) or
        ((this[offset + 1].toInt() and 0xff) shl 8)

private fun ByteArray.littleEndianInt(offset: Int): Int =
    (this[offset].toInt() and 0xff) or
        ((this[offset + 1].toInt() and 0xff) shl 8) or
        ((this[offset + 2].toInt() and 0xff) shl 16) or
        ((this[offset + 3].toInt() and 0xff) shl 24)
