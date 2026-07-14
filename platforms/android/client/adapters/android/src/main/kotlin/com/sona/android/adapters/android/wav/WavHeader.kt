package com.sona.android.adapters.android.wav

import java.nio.ByteBuffer
import java.nio.ByteOrder

internal object WavHeader {
    const val SIZE_BYTES = 44
    const val SAMPLE_RATE_HZ = 16_000
    const val CHANNEL_COUNT = 1
    const val BITS_PER_SAMPLE = 16
    const val BYTES_PER_SECOND = 32_000L
    const val CHECKPOINT_BYTES = 32_000L
    const val RIFF_SIZE_OFFSET = 4L
    const val DATA_SIZE_OFFSET = 40L
    const val MAX_DATA_BYTES = 0xffff_ffffL - 36L

    fun encode(dataBytes: Long): ByteArray {
        require(dataBytes in 0..MAX_DATA_BYTES) { "PCM data is too large for a WAV file." }
        return ByteBuffer.allocate(SIZE_BYTES)
            .order(ByteOrder.LITTLE_ENDIAN)
            .put("RIFF".toByteArray(Charsets.US_ASCII))
            .putInt((36L + dataBytes).toInt())
            .put("WAVE".toByteArray(Charsets.US_ASCII))
            .put("fmt ".toByteArray(Charsets.US_ASCII))
            .putInt(16)
            .putShort(1)
            .putShort(CHANNEL_COUNT.toShort())
            .putInt(SAMPLE_RATE_HZ)
            .putInt(BYTES_PER_SECOND.toInt())
            .putShort((CHANNEL_COUNT * BITS_PER_SAMPLE / 8).toShort())
            .putShort(BITS_PER_SAMPLE.toShort())
            .put("data".toByteArray(Charsets.US_ASCII))
            .putInt(dataBytes.toInt())
            .array()
    }

    fun riffSize(dataBytes: Long): ByteArray = littleEndianInt(36L + dataBytes)

    fun dataSize(dataBytes: Long): ByteArray = littleEndianInt(dataBytes)

    private fun littleEndianInt(value: Long): ByteArray =
        ByteBuffer.allocate(Int.SIZE_BYTES)
            .order(ByteOrder.LITTLE_ENDIAN)
            .putInt(value.toInt())
            .array()
}
