package com.sona.android.adapters.android.audio

internal class PcmFrameAssembler(
    private val frameSizeBytes: Int,
) {
    private val partialFrame: ByteArray
    private var partialBytes = 0

    init {
        require(frameSizeBytes > 0) { "PCM frame size must be positive." }
        partialFrame = ByteArray(frameSizeBytes)
    }

    fun append(bytes: ByteArray, offset: Int, length: Int): List<ByteArray> {
        require(offset >= 0 && length >= 0 && offset <= bytes.size - length) {
            "PCM byte range is invalid."
        }
        if (length == 0) {
            return emptyList()
        }

        val frames = mutableListOf<ByteArray>()
        var sourceOffset = offset
        var remaining = length
        while (remaining > 0) {
            val copied = minOf(remaining, frameSizeBytes - partialBytes)
            bytes.copyInto(
                destination = partialFrame,
                destinationOffset = partialBytes,
                startIndex = sourceOffset,
                endIndex = sourceOffset + copied,
            )
            partialBytes += copied
            sourceOffset += copied
            remaining -= copied

            if (partialBytes == frameSizeBytes) {
                frames += partialFrame.copyOf()
                partialBytes = 0
            }
        }
        return frames
    }
}
