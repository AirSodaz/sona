package com.sona.android.application.recording

import java.io.IOException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.Flow
import kotlinx.coroutines.flow.first

data class AudioImportSource(
    val locator: String,
)

sealed interface AudioImportTarget {
    data class NewImport(val source: AudioImportSource) : AudioImportTarget

    data class ExistingRecording(
        val historyId: String,
        val audioPath: String,
        val displayName: String,
        val durationMillis: Long,
    ) : AudioImportTarget
}

sealed interface AudioImportEngine {
    data class Local(val modelId: String) : AudioImportEngine
    data class Online(val provider: OnlineAsrProvider) : AudioImportEngine
}

data class AudioImportJob(
    val id: String,
    val target: AudioImportTarget,
    val engine: AudioImportEngine,
)

enum class AudioImportStage {
    QUEUED,
    STAGING,
    TRANSCODING,
    TRANSCRIBING,
    SAVING,
}

enum class AudioImportFailure {
    INVALID_SOURCE,
    UNSUPPORTED_AUDIO,
    DURATION_LIMIT,
    STORAGE,
    CONFIGURATION,
    TRANSCODING,
    TRANSCRIPTION,
    PERSISTENCE,
}

sealed interface AudioImportJobState {
    data object Idle : AudioImportJobState
    data class Running(
        val jobId: String,
        val displayName: String?,
        val stage: AudioImportStage,
        val progressPercent: Int?,
    ) : AudioImportJobState

    data class Completed(
        val jobId: String,
        val historyId: String,
        val transcriptionWarning: Boolean,
    ) : AudioImportJobState

    data class Failed(
        val jobId: String?,
        val reason: AudioImportFailure,
    ) : AudioImportJobState
}

interface AudioImportJobPort {
    val state: Flow<AudioImportJobState>

    suspend fun enqueue(job: AudioImportJob)
    suspend fun cancel(jobId: String)
}

data class PreparedImportedAudio(
    val sourcePath: String,
    val normalizedWavPath: String,
    val displayName: String,
    val durationMillis: Long,
)

fun interface AudioImportProgressListener {
    suspend fun onProgress(stage: AudioImportStage, progressPercent: Int?)
}

interface AudioTranscoderPort {
    suspend fun prepare(
        jobId: String,
        source: AudioImportSource,
        progress: AudioImportProgressListener,
    ): PreparedImportedAudio

    suspend fun cleanup(jobId: String)
}

fun interface PcmAudioReaderPort {
    fun readFrames(normalizedWavPath: String): Flow<Pcm16Frame>
}

data class LocalBatchTranscriptionRequest(
    val audioPath: String,
    val config: LocalSherpaConfig,
    val language: String,
    val enableItn: Boolean,
)

data class LocalBatchTranscriptionResult(
    val segments: List<TranscriptSegment>,
)

fun interface LocalBatchTranscriptionPort {
    suspend fun transcribe(request: LocalBatchTranscriptionRequest): LocalBatchTranscriptionResult
}

data class SaveImportedRecordingRequest(
    val historyId: String,
    val displayName: String,
    val normalizedWavPath: String,
    val durationMillis: Long,
    val segments: List<TranscriptSegment>,
)

interface ImportedRecordingHistoryPort {
    suspend fun contains(historyId: String): Boolean
    suspend fun saveImported(request: SaveImportedRecordingRequest): HistoryRecordingSummary
    suspend fun updateTranscript(historyId: String, segments: List<TranscriptSegment>)
}

sealed interface ScheduleAudioImportOutcome {
    data class Scheduled(val jobId: String) : ScheduleAudioImportOutcome
    data object NeedsConfiguration : ScheduleAudioImportOutcome
    data object Failed : ScheduleAudioImportOutcome
}

class ScheduleAudioImport(
    private val recognitionSettings: RecognitionSettingsPort,
    private val batchCredentials: BatchCredentialSettingsPort,
    private val recordingIds: RecordingIdPort,
    private val jobs: AudioImportJobPort,
) {
    suspend operator fun invoke(source: AudioImportSource): ScheduleAudioImportOutcome {
        if (source.locator.isBlank()) return ScheduleAudioImportOutcome.Failed
        return try {
            val settings = recognitionSettings.load()
            val engine = when (val selection = settings.batchSelection) {
                is AsrModelSelection.Local -> {
                    val model = settings.installedModels.firstOrNull { it.id == selection.modelId }
                        ?.takeIf { it.supports(AsrMode.BATCH) }
                        ?: return ScheduleAudioImportOutcome.NeedsConfiguration
                    AudioImportEngine.Local(model.id)
                }
                is AsrModelSelection.Online -> {
                    val configuration = batchCredentials.configuration.first()
                    if (configuration.statusFor(selection.provider) != CredentialStatus.CONFIGURED) {
                        return ScheduleAudioImportOutcome.NeedsConfiguration
                    }
                    AudioImportEngine.Online(selection.provider)
                }
                null -> return ScheduleAudioImportOutcome.NeedsConfiguration
            }
            val job = AudioImportJob(
                id = recordingIds.nextRecordingId(),
                target = AudioImportTarget.NewImport(source),
                engine = engine,
            )
            jobs.enqueue(job)
            ScheduleAudioImportOutcome.Scheduled(job.id)
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            ScheduleAudioImportOutcome.Failed
        }
    }
}

class ScheduleAudioRetranscription(
    private val recognitionSettings: RecognitionSettingsPort,
    private val batchCredentials: BatchCredentialSettingsPort,
    private val recordingIds: RecordingIdPort,
    private val jobs: AudioImportJobPort,
) {
    suspend operator fun invoke(
        historyId: String,
        audioPath: String,
        displayName: String,
        durationMillis: Long,
    ): ScheduleAudioImportOutcome {
        if (historyId.isBlank() || audioPath.isBlank()) return ScheduleAudioImportOutcome.Failed
        return try {
            val settings = recognitionSettings.load()
            val engine = when (val selection = settings.batchSelection) {
                is AsrModelSelection.Local -> {
                    val model = settings.installedModels.firstOrNull { it.id == selection.modelId }
                        ?.takeIf { it.supports(AsrMode.BATCH) }
                        ?: return ScheduleAudioImportOutcome.NeedsConfiguration
                    AudioImportEngine.Local(model.id)
                }
                is AsrModelSelection.Online -> {
                    val configuration = batchCredentials.configuration.first()
                    if (configuration.statusFor(selection.provider) != CredentialStatus.CONFIGURED) {
                        return ScheduleAudioImportOutcome.NeedsConfiguration
                    }
                    AudioImportEngine.Online(selection.provider)
                }
                null -> return ScheduleAudioImportOutcome.NeedsConfiguration
            }
            val job = AudioImportJob(
                id = recordingIds.nextRecordingId(),
                target = AudioImportTarget.ExistingRecording(
                    historyId = historyId,
                    audioPath = audioPath,
                    displayName = displayName,
                    durationMillis = durationMillis,
                ),
                engine = engine,
            )
            jobs.enqueue(job)
            ScheduleAudioImportOutcome.Scheduled(job.id)
        } catch (error: CancellationException) {
            throw error
        } catch (_: Exception) {
            ScheduleAudioImportOutcome.Failed
        }
    }
}

sealed interface RunAudioImportOutcome {
    data class Completed(
        val historyId: String,
        val transcriptionWarning: Boolean,
    ) : RunAudioImportOutcome

    data class RetryableFailure(val reason: AudioImportFailure) : RunAudioImportOutcome
    data class TerminalFailure(val reason: AudioImportFailure) : RunAudioImportOutcome
}

class AudioImportPortException(
    val reason: AudioImportFailure,
    val retryable: Boolean = false,
    cause: Throwable? = null,
) : IOException(reason.name, cause)

class RunAudioImport(
    private val transcoder: AudioTranscoderPort,
    private val recognitionSettings: RecognitionSettingsPort,
    private val batchCredentials: BatchCredentialResolverPort,
    private val localTranscription: LocalBatchTranscriptionPort,
    private val onlineTranscription: OnlineBatchTranscriptionPort,
    private val history: ImportedRecordingHistoryPort,
) {
    suspend operator fun invoke(
        job: AudioImportJob,
        progress: AudioImportProgressListener,
        allowTranscriptionWarning: Boolean,
    ): RunAudioImportOutcome {
        val target = job.target
        if (target is AudioImportTarget.NewImport && history.contains(job.id)) {
            transcoder.cleanup(job.id)
            return RunAudioImportOutcome.Completed(job.id, transcriptionWarning = false)
        }

        val prepared = when (target) {
            is AudioImportTarget.NewImport -> try {
                transcoder.prepare(job.id, target.source, progress)
            } catch (error: CancellationException) {
                transcoder.cleanup(job.id)
                throw error
            } catch (error: AudioImportPortException) {
                val outcome = error.toOutcome()
                if (outcome is RunAudioImportOutcome.TerminalFailure || allowTranscriptionWarning) {
                    transcoder.cleanup(job.id)
                }
                return outcome
            } catch (_: Exception) {
                transcoder.cleanup(job.id)
                return RunAudioImportOutcome.TerminalFailure(AudioImportFailure.TRANSCODING)
            }
            is AudioImportTarget.ExistingRecording -> PreparedImportedAudio(
                sourcePath = target.audioPath,
                normalizedWavPath = target.audioPath,
                displayName = target.displayName,
                durationMillis = target.durationMillis,
            )
        }

        progress.onProgress(AudioImportStage.TRANSCRIBING, null)
        var warning = false
        val segments = try {
            when (val engine = job.engine) {
                is AudioImportEngine.Local -> transcribeLocal(job, engine, prepared)
                is AudioImportEngine.Online -> transcribeOnline(engine, prepared)
            }
        } catch (error: CancellationException) {
            cleanupNewImport(job)
            throw error
        } catch (error: AudioImportPortException) {
            if (target is AudioImportTarget.ExistingRecording) return error.toOutcome()
            if (!allowTranscriptionWarning && error.retryable) return error.toOutcome()
            warning = true
            emptyList()
        } catch (_: Exception) {
            if (target is AudioImportTarget.ExistingRecording) {
                return RunAudioImportOutcome.RetryableFailure(AudioImportFailure.TRANSCRIPTION)
            }
            if (!allowTranscriptionWarning) {
                return RunAudioImportOutcome.RetryableFailure(AudioImportFailure.TRANSCRIPTION)
            }
            warning = true
            emptyList()
        }

        return try {
            progress.onProgress(AudioImportStage.SAVING, null)
            val historyId = when (target) {
                is AudioImportTarget.NewImport -> {
                    history.saveImported(
                        SaveImportedRecordingRequest(
                            historyId = job.id,
                            displayName = prepared.displayName,
                            normalizedWavPath = prepared.normalizedWavPath,
                            durationMillis = prepared.durationMillis,
                            segments = segments,
                        ),
                    )
                    transcoder.cleanup(job.id)
                    job.id
                }
                is AudioImportTarget.ExistingRecording -> {
                    history.updateTranscript(target.historyId, segments)
                    target.historyId
                }
            }
            RunAudioImportOutcome.Completed(historyId, warning)
        } catch (error: CancellationException) {
            cleanupNewImport(job)
            throw error
        } catch (error: AudioImportPortException) {
            if (!error.retryable || allowTranscriptionWarning) {
                cleanupNewImport(job)
                RunAudioImportOutcome.TerminalFailure(error.reason)
            } else {
                RunAudioImportOutcome.RetryableFailure(error.reason)
            }
        } catch (_: Exception) {
            if (allowTranscriptionWarning) {
                cleanupNewImport(job)
                RunAudioImportOutcome.TerminalFailure(AudioImportFailure.PERSISTENCE)
            } else {
                RunAudioImportOutcome.RetryableFailure(AudioImportFailure.PERSISTENCE)
            }
        }
    }

    private suspend fun cleanupNewImport(job: AudioImportJob) {
        if (job.target is AudioImportTarget.NewImport) transcoder.cleanup(job.id)
    }

    private suspend fun transcribeOnline(
        engine: AudioImportEngine.Online,
        prepared: PreparedImportedAudio,
    ): List<TranscriptSegment> {
        val credential = batchCredentials.load(engine.provider)
            ?: throw AudioImportPortException(AudioImportFailure.CONFIGURATION)
        return onlineTranscription.transcribe(
            OnlineBatchTranscriptionRequest(
                audioPath = prepared.sourcePath,
                provider = engine.provider,
                credential = credential,
                language = "auto",
            ),
        ).segments
    }

    private suspend fun transcribeLocal(
        @Suppress("UNUSED_PARAMETER") job: AudioImportJob,
        engine: AudioImportEngine.Local,
        prepared: PreparedImportedAudio,
    ): List<TranscriptSegment> {
        val model = recognitionSettings.load().installedModels.firstOrNull { it.id == engine.modelId }
            ?.takeIf { it.supports(AsrMode.BATCH) }
            ?: throw AudioImportPortException(AudioImportFailure.CONFIGURATION)
        return localTranscription.transcribe(
            LocalBatchTranscriptionRequest(
                audioPath = prepared.normalizedWavPath,
                config = model.config,
                language = "auto",
                enableItn = true,
            ),
        ).segments
    }
}

private fun AudioImportPortException.toOutcome(): RunAudioImportOutcome =
    if (retryable) {
        RunAudioImportOutcome.RetryableFailure(reason)
    } else {
        RunAudioImportOutcome.TerminalFailure(reason)
    }
