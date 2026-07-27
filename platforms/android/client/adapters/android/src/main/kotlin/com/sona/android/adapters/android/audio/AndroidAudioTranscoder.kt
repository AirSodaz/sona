package com.sona.android.adapters.android.audio

import android.content.ContentResolver
import android.content.Context
import android.media.MediaCodec
import android.media.MediaExtractor
import android.media.MediaFormat
import android.net.Uri
import android.os.StatFs
import android.provider.OpenableColumns
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor
import androidx.media3.common.audio.ChannelMixingAudioProcessor
import androidx.media3.common.audio.ChannelMixingMatrix
import androidx.media3.common.audio.SonicAudioProcessor
import androidx.media3.common.audio.ToInt16PcmAudioProcessor
import androidx.media3.common.util.UnstableApi
import com.sona.android.adapters.android.wav.CheckpointingWavWriter
import com.sona.android.application.recording.AudioImportFailure
import com.sona.android.application.recording.AudioImportPortException
import com.sona.android.application.recording.AudioImportProgressListener
import com.sona.android.application.recording.AudioImportSource
import com.sona.android.application.recording.AudioImportStage
import com.sona.android.application.recording.AudioTranscoderPort
import com.sona.android.application.recording.PreparedImportedAudio
import com.sona.android.application.recording.RecordingDestination
import java.io.File
import java.io.FileInputStream
import java.io.FileOutputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.coroutines.coroutineContext
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.withContext

@UnstableApi
class AndroidAudioTranscoder private constructor(
    private val contentResolver: ContentResolver,
    private val jobsRoot: File,
) : AudioTranscoderPort {
    companion object {
        fun create(context: Context): AndroidAudioTranscoder = AndroidAudioTranscoder(
            contentResolver = context.contentResolver,
            jobsRoot = File(context.filesDir, "audio-import-jobs"),
        )
    }

    override suspend fun prepare(
        jobId: String,
        source: AudioImportSource,
        progress: AudioImportProgressListener,
    ): PreparedImportedAudio = withContext(Dispatchers.IO) {
        requireValidJobId(jobId)
        val uri = parseContentUri(source.locator)
        val metadata = queryMetadata(uri)
        val jobDirectory = File(jobsRoot, jobId).apply { mkdirsOrThrow() }
        val extension = safeExtension(metadata.displayName)
        val sourceFile = File(jobDirectory, "source$extension")
        val reusableSource = sourceFile.isFile && sourceFile.length() > 0L &&
            (metadata.sizeBytes == null || sourceFile.length() == metadata.sizeBytes)
        if (!reusableSource) {
            sourceFile.delete()
            progress.onProgress(AudioImportStage.STAGING, 0)
            stageSource(uri, sourceFile, metadata.sizeBytes, progress)
        }

        val outputFile = File(jobDirectory, NORMALIZED_WAV_NAME)
        if (outputFile.isFile) {
            val header = ByteArray(WAV_HEADER_BYTES)
            FileInputStream(outputFile).use { it.read(header) }
            if (isNormalizedWav(header, outputFile.length())) {
                return@withContext PreparedImportedAudio(
                    sourcePath = sourceFile.absolutePath,
                    normalizedWavPath = outputFile.absolutePath,
                    displayName = metadata.displayName,
                    durationMillis = validatedWavDurationMillis(outputFile),
                )
            }
            outputFile.delete()
        }

        progress.onProgress(AudioImportStage.TRANSCODING, 0)
        val result = transcode(sourceFile, outputFile, progress)
        PreparedImportedAudio(
            sourcePath = sourceFile.absolutePath,
            normalizedWavPath = outputFile.absolutePath,
            displayName = metadata.displayName,
            durationMillis = result.durationMillis,
        )
    }

    override suspend fun cleanup(jobId: String) = withContext(Dispatchers.IO) {
        requireValidJobId(jobId)
        File(jobsRoot, jobId).deleteRecursively()
        Unit
    }

    private suspend fun stageSource(
        uri: Uri,
        destination: File,
        declaredSize: Long?,
        progress: AudioImportProgressListener,
    ) {
        val directory = destination.parentFile
            ?: throw AudioImportPortException(AudioImportFailure.STORAGE)
        val partial = File(directory, "${destination.name}.partial")
        partial.delete()
        ensureStorage(directory, (declaredSize ?: 0L) + MIN_FREE_BYTES)
        val input = contentResolver.openInputStream(uri)
            ?: throw AudioImportPortException(AudioImportFailure.INVALID_SOURCE)
        try {
            input.use { source ->
                FileOutputStream(partial).use { output ->
                    val buffer = ByteArray(COPY_BUFFER_BYTES)
                    var copied = 0L
                    while (true) {
                        coroutineContext.ensureActive()
                        val read = source.read(buffer)
                        if (read < 0) break
                        output.write(buffer, 0, read)
                        copied += read
                        if (copied % STORAGE_RECHECK_BYTES < read) {
                            ensureStorage(directory, MIN_FREE_BYTES)
                        }
                        val percent = declaredSize
                            ?.takeIf { it > 0 }
                            ?.let { ((copied * 100L / it).coerceIn(0, 99)).toInt() }
                        progress.onProgress(AudioImportStage.STAGING, percent)
                    }
                    output.fd.sync()
                }
            }
            if (partial.length() == 0L) {
                throw AudioImportPortException(AudioImportFailure.INVALID_SOURCE)
            }
            moveAtomically(partial, destination)
            progress.onProgress(AudioImportStage.STAGING, 100)
        } catch (error: CancellationException) {
            partial.delete()
            throw error
        } catch (error: AudioImportPortException) {
            partial.delete()
            throw error
        } catch (error: Exception) {
            partial.delete()
            throw AudioImportPortException(
                reason = AudioImportFailure.INVALID_SOURCE,
                retryable = true,
                cause = error,
            )
        }
    }

    private suspend fun transcode(
        source: File,
        destination: File,
        progress: AudioImportProgressListener,
    ): TranscodeResult {
        val extractor = MediaExtractor()
        var codec: MediaCodec? = null
        val directory = destination.parentFile
            ?: throw AudioImportPortException(AudioImportFailure.STORAGE)
        val partial = File(directory, "$NORMALIZED_WAV_NAME.partial")
        partial.delete()
        try {
            extractor.setDataSource(source.absolutePath)
            val trackIndex = (0 until extractor.trackCount).firstOrNull { index ->
                extractor.getTrackFormat(index)
                    .getString(MediaFormat.KEY_MIME)
                    ?.startsWith("audio/") == true
            } ?: throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
            val trackFormat = extractor.getTrackFormat(trackIndex)
            val durationUs = trackFormat.getLongOrDefault(MediaFormat.KEY_DURATION, -1L)
            validateDuration(durationUs)
            ensureStorage(
                directory,
                estimatedOutputBytes(durationUs) * 2L + MIN_FREE_BYTES,
            )
            val mime = trackFormat.getString(MediaFormat.KEY_MIME)
                ?: throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
            extractor.selectTrack(trackIndex)
            val decoder = try {
                MediaCodec.createDecoderByType(mime)
            } catch (error: Exception) {
                throw AudioImportPortException(
                    AudioImportFailure.UNSUPPORTED_AUDIO,
                    cause = error,
                )
            }
            codec = decoder
            decoder.configure(trackFormat, null, null, 0)
            decoder.start()

            var processors: PcmProcessorChain? = null
            var inputEnded = false
            var outputEnded = false
            var maxPresentationTimeUs = 0L
            val bufferInfo = MediaCodec.BufferInfo()
            CheckpointingWavWriter.open(RecordingDestination(partial.absolutePath)).use { writer ->
                while (!outputEnded) {
                    coroutineContext.ensureActive()
                    if (!inputEnded) {
                        val inputIndex = decoder.dequeueInputBuffer(CODEC_TIMEOUT_US)
                        if (inputIndex >= 0) {
                            val inputBuffer = decoder.getInputBuffer(inputIndex)
                                ?: throw AudioImportPortException(AudioImportFailure.TRANSCODING)
                            val sampleSize = extractor.readSampleData(inputBuffer, 0)
                            if (sampleSize < 0) {
                                decoder.queueInputBuffer(
                                    inputIndex,
                                    0,
                                    0,
                                    0,
                                    MediaCodec.BUFFER_FLAG_END_OF_STREAM,
                                )
                                inputEnded = true
                            } else {
                                decoder.queueInputBuffer(
                                    inputIndex,
                                    0,
                                    sampleSize,
                                    extractor.sampleTime.coerceAtLeast(0L),
                                    0,
                                )
                                extractor.advance()
                            }
                        }
                    }

                    when (val outputIndex = decoder.dequeueOutputBuffer(bufferInfo, CODEC_TIMEOUT_US)) {
                        MediaCodec.INFO_OUTPUT_FORMAT_CHANGED -> {
                            processors?.reset()
                            processors = PcmProcessorChain(decoder.outputFormat)
                        }
                        MediaCodec.INFO_TRY_AGAIN_LATER -> Unit
                        else -> if (outputIndex >= 0) {
                            val outputBuffer = decoder.getOutputBuffer(outputIndex)
                            if (bufferInfo.size > 0 && outputBuffer != null) {
                                val chain = processors ?: PcmProcessorChain(decoder.outputFormat)
                                    .also { processors = it }
                                outputBuffer.position(bufferInfo.offset)
                                outputBuffer.limit(bufferInfo.offset + bufferInfo.size)
                                chain.queue(outputBuffer.slice().order(ByteOrder.nativeOrder()), writer)
                                maxPresentationTimeUs = maxOf(
                                    maxPresentationTimeUs,
                                    bufferInfo.presentationTimeUs,
                                )
                                validateDuration(maxPresentationTimeUs)
                                val percent = durationUs.takeIf { it > 0 }?.let {
                                    ((maxPresentationTimeUs * 100L / it).coerceIn(0, 99)).toInt()
                                }
                                progress.onProgress(AudioImportStage.TRANSCODING, percent)
                            }
                            outputEnded = bufferInfo.flags and MediaCodec.BUFFER_FLAG_END_OF_STREAM != 0
                            decoder.releaseOutputBuffer(outputIndex, false)
                        }
                    }
                }
                val chain = processors
                    ?: throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
                chain.finish(writer)
                val captured = writer.finish()
                if (captured.bytesWritten == 0L) {
                    throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
                }
            }
            val durationMillis = validatedWavDurationMillis(partial)
            moveAtomically(partial, destination)
            progress.onProgress(AudioImportStage.TRANSCODING, 100)
            return TranscodeResult(durationMillis)
        } catch (error: CancellationException) {
            partial.delete()
            throw error
        } catch (error: AudioImportPortException) {
            partial.delete()
            throw error
        } catch (error: Exception) {
            partial.delete()
            throw AudioImportPortException(AudioImportFailure.TRANSCODING, cause = error)
        } finally {
            try {
                codec?.stop()
            } catch (_: Exception) {
                // The original codec failure remains authoritative.
            }
            codec?.release()
            extractor.release()
        }
    }

    private fun queryMetadata(uri: Uri): SourceMetadata {
        var displayName: String? = null
        var sizeBytes: Long? = null
        contentResolver.query(
            uri,
            arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE),
            null,
            null,
            null,
        )?.use { cursor ->
            if (cursor.moveToFirst()) {
                val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                val sizeIndex = cursor.getColumnIndex(OpenableColumns.SIZE)
                if (nameIndex >= 0 && !cursor.isNull(nameIndex)) displayName = cursor.getString(nameIndex)
                if (sizeIndex >= 0 && !cursor.isNull(sizeIndex)) sizeBytes = cursor.getLong(sizeIndex)
            }
        }
        return SourceMetadata(
            displayName = sanitizeDisplayName(displayName),
            sizeBytes = sizeBytes?.takeIf { it >= 0 },
        )
    }
}

@UnstableApi
private class PcmProcessorChain(format: MediaFormat) {
    private val sampleRate = format.getInteger(MediaFormat.KEY_SAMPLE_RATE)
    private val channelCount = format.getInteger(MediaFormat.KEY_CHANNEL_COUNT)
    private val pcm16: ToInt16PcmAudioProcessor?
    private val mixing = if (channelCount == 1) null else ChannelMixingAudioProcessor()
    private val sonic = SonicAudioProcessor()

    init {
        val encoding = format.getIntegerOrDefault(
            PCM_ENCODING_KEY,
            C.ENCODING_PCM_16BIT,
        )
        if (channelCount !in 1..6 || sampleRate <= 0) {
            throw AudioImportPortException(AudioImportFailure.UNSUPPORTED_AUDIO)
        }
        pcm16 = if (encoding == C.ENCODING_PCM_16BIT) {
            null
        } else {
            try {
                ToInt16PcmAudioProcessor().also { processor ->
                    processor.configure(
                        AudioProcessor.AudioFormat(sampleRate, channelCount, encoding),
                    )
                    processor.flush()
                }
            } catch (error: AudioProcessor.UnhandledAudioFormatException) {
                throw AudioImportPortException(
                    AudioImportFailure.UNSUPPORTED_AUDIO,
                    cause = error,
                )
            }
        }
        mixing?.let { processor ->
            processor.putChannelMixingMatrix(
                ChannelMixingMatrix.createForConstantPower(channelCount, 1),
            )
            processor.configure(AudioProcessor.AudioFormat(sampleRate, channelCount, encoding))
            processor.flush()
        }
        sonic.setOutputSampleRateHz(16_000)
        sonic.configure(AudioProcessor.AudioFormat(sampleRate, 1, C.ENCODING_PCM_16BIT))
        sonic.flush()
    }

    fun queue(input: ByteBuffer, writer: CheckpointingWavWriter) {
        val processor = pcm16
        if (processor != null) {
            processor.queueInput(input)
            drainPcm16(writer)
            return
        }
        queuePcm16(input, writer)
    }

    fun finish(writer: CheckpointingWavWriter) {
        pcm16?.let { processor ->
            processor.queueEndOfStream()
            drainPcm16(writer)
        }
        mixing?.let { processor ->
            processor.queueEndOfStream()
            drainMixing(writer)
        }
        sonic.queueEndOfStream()
        drainSonic(writer)
    }

    fun reset() {
        pcm16?.reset()
        mixing?.reset()
        sonic.reset()
    }

    private fun drainPcm16(writer: CheckpointingWavWriter) {
        while (true) {
            val output = checkNotNull(pcm16).output
            if (!output.hasRemaining()) break
            queuePcm16(output, writer)
        }
    }

    private fun queuePcm16(input: ByteBuffer, writer: CheckpointingWavWriter) {
        val processor = mixing
        if (processor == null) {
            sonic.queueInput(input)
            drainSonic(writer)
        } else {
            processor.queueInput(input)
            drainMixing(writer)
        }
    }

    private fun drainMixing(writer: CheckpointingWavWriter) {
        while (true) {
            val mixed = checkNotNull(mixing).output
            if (!mixed.hasRemaining()) break
            sonic.queueInput(mixed)
            drainSonic(writer)
        }
    }

    private fun drainSonic(writer: CheckpointingWavWriter) {
        while (true) {
            val output = sonic.output
            if (!output.hasRemaining()) break
            val bytes = ByteArray(output.remaining())
            output.get(bytes)
            writer.write(bytes, 0, bytes.size)
        }
    }
}

private data class SourceMetadata(val displayName: String, val sizeBytes: Long?)
private data class TranscodeResult(val durationMillis: Long)

private fun parseContentUri(value: String): Uri {
    val uri = runCatching { Uri.parse(value) }.getOrNull()
    if (uri == null || uri.scheme != ContentResolver.SCHEME_CONTENT) {
        throw AudioImportPortException(AudioImportFailure.INVALID_SOURCE)
    }
    return uri
}

private fun requireValidJobId(jobId: String) {
    require(jobId.matches(Regex("[A-Za-z0-9-]{1,64}"))) { "Audio import job ID is invalid." }
}

private fun File.mkdirsOrThrow() {
    val expectedParent = parentFile
        ?: throw AudioImportPortException(AudioImportFailure.STORAGE)
    if ((!isDirectory && !mkdirs()) || canonicalFile.parentFile != expectedParent.canonicalFile) {
        throw AudioImportPortException(AudioImportFailure.STORAGE)
    }
}

private fun ensureStorage(directory: File, requiredBytes: Long) {
    val available = StatFs(directory.absolutePath).availableBytes
    if (available < requiredBytes.coerceAtLeast(MIN_FREE_BYTES)) {
        throw AudioImportPortException(AudioImportFailure.STORAGE)
    }
}

private fun validateDuration(durationUs: Long) {
    if (durationUs > MAX_DURATION_US) {
        throw AudioImportPortException(AudioImportFailure.DURATION_LIMIT)
    }
}

private fun estimatedOutputBytes(durationUs: Long): Long =
    if (durationUs <= 0) {
        MAX_DURATION_PCM_BYTES + WAV_HEADER_BYTES
    } else {
        durationUs * PCM_BYTES_PER_SECOND / 1_000_000L + WAV_HEADER_BYTES
    }

private fun sanitizeDisplayName(value: String?): String {
    val cleaned = value.orEmpty()
        .substringAfterLast('/')
        .substringAfterLast('\\')
        .replace(Regex("[\\p{Cntrl}]"), "")
        .trim()
        .take(180)
    return cleaned.ifBlank { "Imported audio" }
}

private fun safeExtension(displayName: String): String {
    val extension = displayName.substringAfterLast('.', "")
        .lowercase()
        .takeIf { it.matches(Regex("[a-z0-9]{1,10}")) }
    return extension?.let { ".$it" } ?: ".audio"
}

private fun validatedWavDurationMillis(file: File): Long {
    val dataBytes = (file.length() - WAV_HEADER_BYTES).coerceAtLeast(0L)
    if (dataBytes > MAX_DURATION_PCM_BYTES) {
        throw AudioImportPortException(AudioImportFailure.DURATION_LIMIT)
    }
    return dataBytes * 1_000L / PCM_BYTES_PER_SECOND
}

private fun MediaFormat.getLongOrDefault(key: String, defaultValue: Long): Long =
    if (containsKey(key)) getLong(key) else defaultValue

private fun MediaFormat.getIntegerOrDefault(key: String, defaultValue: Int): Int =
    if (containsKey(key)) getInteger(key) else defaultValue

private const val NORMALIZED_WAV_NAME = "normalized.wav"
private fun moveAtomically(source: File, destination: File) {
    if (destination.exists() && !destination.delete()) {
        throw AudioImportPortException(AudioImportFailure.STORAGE)
    }
    if (!source.renameTo(destination)) {
        throw AudioImportPortException(AudioImportFailure.STORAGE)
    }
}

private const val WAV_HEADER_BYTES = 44
private const val MAX_DURATION_US = 6L * 60L * 60L * 1_000_000L
private const val PCM_BYTES_PER_SECOND = 32_000L
private const val MAX_DURATION_PCM_BYTES = 6L * 60L * 60L * PCM_BYTES_PER_SECOND
private const val MIN_FREE_BYTES = 64L * 1024L * 1024L
private const val STORAGE_RECHECK_BYTES = 16L * 1024L * 1024L
private const val COPY_BUFFER_BYTES = 128 * 1024
private const val CODEC_TIMEOUT_US = 10_000L
private const val PCM_ENCODING_KEY = "pcm-encoding"
