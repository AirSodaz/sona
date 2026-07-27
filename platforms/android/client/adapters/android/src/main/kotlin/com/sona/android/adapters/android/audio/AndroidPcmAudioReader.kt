package com.sona.android.adapters.android.audio

import com.sona.android.application.recording.AudioImportFailure
import com.sona.android.application.recording.AudioImportPortException
import com.sona.android.application.recording.Pcm16Frame
import com.sona.android.application.recording.PcmAudioReaderPort
import java.io.File
import java.io.FileInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.flow
import kotlinx.coroutines.flow.flowOn

class AndroidPcmAudioReader : PcmAudioReaderPort {
    override fun readFrames(normalizedWavPath: String): Flow<Pcm16Frame> = flow {
        val file = File(normalizedWavPath)
        FileInputStream(file).use { input ->
            val header = ByteArray(WAV_HEADER_BYTES)
            var headerBytes = 0
            while (headerBytes < header.size) {
                val read = input.read(header, headerBytes, header.size - headerBytes)
                if (read < 0) break
                headerBytes += read
            }
            if (headerBytes != header.size) {
                throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
            }
            if (!isNormalizedWav(header, file.length())) {
                throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
            }
            val buffer = ByteArray(PCM_FRAME_BYTES)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                val evenLength = read - (read % 2)
                if (evenLength > 0) emit(Pcm16Frame(buffer.copyOf(evenLength)))
            }
        }
    }.flowOn(Dispatchers.IO)
}

internal fun isNormalizedWav(header: ByteArray, fileLength: Long): Boolean {
    if (header.size != WAV_HEADER_BYTES || fileLength < WAV_HEADER_BYTES) return false
    val ascii = Charsets.US_ASCII
    if (
        header.copyOfRange(0, 4).toString(ascii) != "RIFF" ||
        header.copyOfRange(8, 12).toString(ascii) != "WAVE" ||
        header.copyOfRange(12, 16).toString(ascii) != "fmt " ||
        header.copyOfRange(36, 40).toString(ascii) != "data"
    ) return false
    val values = ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN)
    val channels = values.getShort(22).toInt()
    val sampleRate = values.getInt(24)
    val bitsPerSample = values.getShort(34).toInt()
    val dataBytes = values.getInt(40).toLong() and 0xffff_ffffL
    return channels == 1 && sampleRate == 16_000 && bitsPerSample == 16 &&
        dataBytes == fileLength - WAV_HEADER_BYTES
}

private const val WAV_HEADER_BYTES = 44
private const val PCM_FRAME_BYTES = 3_200
