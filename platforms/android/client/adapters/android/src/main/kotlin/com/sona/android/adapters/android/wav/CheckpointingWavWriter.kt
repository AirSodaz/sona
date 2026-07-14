package com.sona.android.adapters.android.wav

import com.sona.android.application.recording.CapturedAudio
import com.sona.android.application.recording.RecordingDestination
import java.io.IOException
import java.io.RandomAccessFile

interface PcmWriter : AutoCloseable {
    fun write(bytes: ByteArray, offset: Int, length: Int)

    fun finish(): CapturedAudio
}

internal interface RandomAccessWavFile : AutoCloseable {
    fun setLength(length: Long)

    fun seek(position: Long)

    fun write(bytes: ByteArray, offset: Int, length: Int)

    fun sync()
}

class CheckpointingWavWriter internal constructor(
    private val file: RandomAccessWavFile,
) : PcmWriter {
    companion object {
        private const val STORAGE_ERROR_MESSAGE = "Unable to write recording audio."

        fun open(destination: RecordingDestination): CheckpointingWavWriter {
            val file = try {
                RandomAccessFileWavFile(RandomAccessFile(destination.value, "rw"))
            } catch (_: Exception) {
                throw storageError()
            }
            return try {
                CheckpointingWavWriter(file)
            } catch (_: Exception) {
                try {
                    file.close()
                } catch (_: Exception) {
                    // Initialization failure remains the public error.
                }
                throw storageError()
            }
        }

        private fun storageError(): IOException = IOException(STORAGE_ERROR_MESSAGE)
    }

    private var dataBytes = 0L
    private var nextCheckpointBytes = WavHeader.CHECKPOINT_BYTES
    private var finishedAudio: CapturedAudio? = null
    private var closed = false

    init {
        fileOperation {
            file.setLength(0)
            file.seek(0)
            val header = WavHeader.encode(dataBytes = 0)
            file.write(header, 0, header.size)
        }
    }

    @Synchronized
    override fun write(bytes: ByteArray, offset: Int, length: Int) {
        check(!closed) { "WAV writer is closed." }
        check(finishedAudio == null) { "WAV writer is already finished." }
        require(offset >= 0 && length >= 0 && offset <= bytes.size - length) {
            "PCM byte range is invalid."
        }
        require(dataBytes + length <= WavHeader.MAX_DATA_BYTES) {
            "PCM data is too large for a WAV file."
        }

        var sourceOffset = offset
        var remaining = length
        while (remaining > 0) {
            val untilCheckpoint = (nextCheckpointBytes - dataBytes).toInt()
            val chunkLength = minOf(remaining, untilCheckpoint)
            fileOperation {
                file.write(bytes, sourceOffset, chunkLength)
            }
            sourceOffset += chunkLength
            remaining -= chunkLength
            dataBytes += chunkLength

            if (dataBytes == nextCheckpointBytes) {
                checkpoint()
                nextCheckpointBytes += WavHeader.CHECKPOINT_BYTES
            }
        }
    }

    @Synchronized
    override fun finish(): CapturedAudio {
        finishedAudio?.let { return it }
        check(!closed) { "WAV writer is closed." }
        checkpoint()
        return CapturedAudio(
            durationMillis = dataBytes * 1_000L / WavHeader.BYTES_PER_SECOND,
            bytesWritten = dataBytes,
        ).also { finishedAudio = it }
    }

    @Synchronized
    override fun close() {
        if (closed) {
            return
        }

        var failure: IOException? = null
        try {
            finish()
        } catch (_: Exception) {
            failure = storageError()
        }
        try {
            file.close()
        } catch (_: Exception) {
            if (failure == null) {
                failure = storageError()
            }
        } finally {
            closed = true
        }
        failure?.let { throw it }
    }

    private fun checkpoint() {
        val appendPosition = WavHeader.SIZE_BYTES + dataBytes
        fileOperation {
            val riffSize = WavHeader.riffSize(dataBytes)
            val dataSize = WavHeader.dataSize(dataBytes)
            file.seek(WavHeader.RIFF_SIZE_OFFSET)
            file.write(riffSize, 0, riffSize.size)
            file.seek(WavHeader.DATA_SIZE_OFFSET)
            file.write(dataSize, 0, dataSize.size)
            file.seek(appendPosition)
            file.sync()
        }
    }

    private inline fun fileOperation(operation: () -> Unit) {
        try {
            operation()
        } catch (_: Exception) {
            throw storageError()
        }
    }

    private class RandomAccessFileWavFile(
        private val delegate: RandomAccessFile,
    ) : RandomAccessWavFile {
        override fun setLength(length: Long) = delegate.setLength(length)

        override fun seek(position: Long) = delegate.seek(position)

        override fun write(bytes: ByteArray, offset: Int, length: Int) =
            delegate.write(bytes, offset, length)

        override fun sync() = delegate.fd.sync()

        override fun close() = delegate.close()
    }
}
