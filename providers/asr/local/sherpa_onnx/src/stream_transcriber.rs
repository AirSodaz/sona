use sherpa_onnx::VoiceActivityDetector;
use sona_core::models::config::ModelFileConfig;
use sona_core::ports::aligner::AlignerEngineSet;
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, BatchSegmentationMode, BatchTranscriptionObserver,
};
use sona_core::ports::punctuation::{PunctuationEngineSet, PunctuationModel};
use sona_core::ports::vad::{VadDetectionOptions, VadEngineSet};
use sona_core::runtime::logging::diagnostic_label;
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, ensure_transcript_segment_timing,
    normalize_recognizer_text, synthesize_durations,
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::audio::{VadDetectorOptions, create_vad_config};
use crate::audio_stream::{AudioStreamReader, AudioStreamSourceConfig, sanitize_stream_diagnostic};
use crate::batch::{
    OfflineRecognizerCacheKey, finalize_transcript_text, get_or_create_offline_recognizer,
    get_or_create_punctuation,
};
use crate::gpu::{GpuFallbackNotice, is_int8_model, resolve_gpu_acceleration_plan};
use crate::recognizer::{SafeOfflineRecognizer, decode_offline_samples};

pub struct StreamTranscriptionJob {
    pub url: String,
    pub save_to_path: Option<PathBuf>,
    pub model_path: PathBuf,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<PathBuf>,
    pub alignment_model: Option<PathBuf>,
    pub vad_model: Option<PathBuf>,
    pub vad_buffer: f32,
    pub batch_segmentation_mode: BatchSegmentationMode,
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
    pub gpu_acceleration: Option<String>,
    pub quiet: bool,
    pub vad_engines: VadEngineSet,
    pub punct_engines: PunctuationEngineSet,
    pub aligner_engines: AlignerEngineSet,
    pub ffmpeg_enabled: bool,
    pub ffmpeg_path: Option<PathBuf>,
}

async fn process_speech_slice(
    samples: Vec<f32>,
    start_time: f64,
    duration: f64,
    recognizer: Arc<SafeOfflineRecognizer>,
    punctuation: Option<Arc<dyn PunctuationModel>>,
    observer: Arc<dyn BatchTranscriptionObserver>,
) -> Option<TranscriptSegment> {
    let diagnostic_id = diagnostic_label(observer.diagnostic_id());
    log::debug!(
        "[BatchStream] slice_start instance_id={} start_s={:.3} duration_s={:.3} samples={}",
        diagnostic_id,
        start_time,
        duration,
        samples.len(),
    );
    let started_at = std::time::Instant::now();
    let worker_id = diagnostic_id.clone();
    let decoded = tokio::task::spawn_blocking(move || {
        let Some(dec) = decode_offline_samples(&recognizer, &samples) else {
            log::warn!("[BatchStream] slice_skipped instance_id={worker_id} start_s={start_time:.3} duration_s={duration:.3} reason=no_result");
            return None;
        };
        let cleaned = normalize_recognizer_text(&dec.text);
        if cleaned.is_empty() {
            log::debug!("[BatchStream] slice_skipped instance_id={worker_id} start_s={start_time:.3} duration_s={duration:.3} reason=empty_recognition");
            return None;
        }
        let text = finalize_transcript_text(&cleaned, punctuation.as_deref());
        if text.is_empty() {
            log::debug!("[BatchStream] slice_skipped instance_id={worker_id} start_s={start_time:.3} duration_s={duration:.3} reason=empty_after_punctuation");
            return None;
        }
        Some((dec, text))
    })
    .await;
    let decoded = match decoded {
        Ok(result) => result?,
        Err(error) => {
            log::warn!(
                "[BatchStream] slice_skipped instance_id={diagnostic_id} start_s={start_time:.3} duration_s={duration:.3} reason=worker_failure panic={} cancelled={}",
                error.is_panic(),
                error.is_cancelled()
            );
            return None;
        }
    };

    let (decoded, text) = decoded;
    log::debug!(
        "[BatchStream] slice_complete instance_id={} start_s={:.3} duration_s={:.3} text_chars={} elapsed_ms={}",
        diagnostic_id,
        start_time,
        duration,
        text.chars().count(),
        started_at.elapsed().as_millis(),
    );
    let timestamps_abs = decoded.timestamps.as_ref().map(|ts| {
        ts.iter()
            .map(|t| *t + start_time as f32)
            .collect::<Vec<_>>()
    });
    let durations = decoded
        .durations
        .filter(|d| !d.is_empty() && d.len() == decoded.tokens.len())
        .or_else(|| {
            timestamps_abs
                .as_ref()
                .and_then(|ts| synthesize_durations(ts, (start_time + duration) as f32))
        });

    let mut transcript_segment = TranscriptSegment {
        id: uuid::Uuid::new_v4().to_string(),
        text,
        start: start_time,
        end: start_time + duration,
        is_final: true,
        timing: None,
        tokens: Some(decoded.tokens),
        timestamps: timestamps_abs,
        durations,
        translation: None,
        draft_translation: None,
        speaker: None,
        speaker_attribution: None,
    };
    ensure_transcript_segment_timing(&mut transcript_segment);
    observer.on_transcript_update(&TranscriptUpdate {
        remove_ids: Vec::new(),
        upsert_segments: vec![transcript_segment.clone()],
    });
    Some(transcript_segment)
}

impl StreamTranscriptionJob {
    pub fn from_plan(
        plan: BatchTranscribePlan,
        vad_engines: &VadEngineSet,
        punct_engines: &PunctuationEngineSet,
        aligner_engines: &AlignerEngineSet,
    ) -> Result<Self, AsrPortError> {
        let batch_job = crate::batch::BatchTranscriptionJob::from_plan(
            plan,
            vad_engines,
            punct_engines,
            aligner_engines,
        )?;
        Ok(batch_job.into())
    }

    pub async fn transcribe(
        self,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let is_int8 = is_int8_model(&self.model_path, self.file_config.as_ref());
        let gpu_plan =
            resolve_gpu_acceleration_plan(self.gpu_acceleration.as_deref(), is_int8).await;
        let mut last_error = None;
        let mut fallback_notice: Option<GpuFallbackNotice> = None;
        let mut active_recognizer = None;

        for provider in gpu_plan.provider_options() {
            if observer.is_cancelled() {
                return Err(AsrPortError::runtime("Task cancelled."));
            }
            let cache_key = OfflineRecognizerCacheKey {
                model_path: self.model_path.clone(),
                model_type: self.model_type.clone(),
                file_config: self.file_config.clone(),
                enable_itn: self.enable_itn,
                language: self.language.clone(),
                hotwords: self.hotwords.clone(),
                num_threads: self.num_threads,
                provider: provider.clone(),
            };
            match get_or_create_offline_recognizer(cache_key) {
                Ok(recognizer) => {
                    active_recognizer = Some(recognizer);
                    break;
                }
                Err(error) => {
                    if provider
                        .as_deref()
                        .map(|p| gpu_plan.should_retry_after_failure(p))
                        .unwrap_or(false)
                    {
                        fallback_notice =
                            Some(GpuFallbackNotice::directml_retry(error.to_string()));
                        last_error = Some(error);
                    } else {
                        return Err(error);
                    }
                }
            }
        }

        let recognizer = match active_recognizer {
            Some(r) => r,
            None => {
                return Err(last_error
                    .unwrap_or_else(|| AsrPortError::runtime("Recognizer creation failed.")));
            }
        };

        if let Some(notice) = fallback_notice.take()
            && !self.quiet
        {
            log::warn!(
                "DirectML stream recognizer initialization failed, retrying with CPU: {}",
                notice.error
            );
        }

        self.transcribe_with_recognizer(recognizer, observer).await
    }

    async fn transcribe_with_recognizer(
        &self,
        recognizer: Arc<SafeOfflineRecognizer>,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let punctuation =
            get_or_create_punctuation(&self.punct_engines, self.punctuation_model.as_deref())?;
        let is_funasr_nano = recognizer.model_type() == "funasr-nano";
        let effective_punctuation = if is_funasr_nano { None } else { punctuation };
        let stream_config = AudioStreamSourceConfig {
            url: &self.url,
            target_sample_rate: 16000,
            ffmpeg_enabled: self.ffmpeg_enabled,
            custom_ffmpeg_path: self.ffmpeg_path.as_deref(),
            save_to_path: self.save_to_path.clone(),
        };
        let mut reader = AudioStreamReader::open(stream_config).await?;
        let diagnostic_id = diagnostic_label(observer.diagnostic_id());
        let started_at = std::time::Instant::now();
        log::info!(
            "[BatchStream] opened instance_id={} model_type={} segmentation={:?} ffmpeg={} sample_rate=16000 vad_configured={}",
            diagnostic_id,
            self.model_type,
            self.batch_segmentation_mode,
            self.ffmpeg_enabled,
            self.vad_model.is_some(),
        );
        observer.on_progress(5.0);

        if self.alignment_model.is_some() {
            log::warn!(
                "Forced alignment is not supported for live audio stream transcription; skipping alignment."
            );
        }
        if self.speaker_processing.is_some() {
            log::warn!(
                "Speaker diarization is not supported for live audio stream transcription; skipping speaker processing."
            );
        }

        let mut vad_detector = if self.batch_segmentation_mode == BatchSegmentationMode::Vad {
            if let Some(vad_path) = self.vad_model.as_deref() {
                let vad_batch = VadDetectionOptions::batch_defaults(vad_path);
                log::info!(
                    "[BatchStream] vad_options instance_id={diagnostic_id} threshold={} min_silence_s={} min_speech_s={} max_speech_s={} escalation_s={} buffer_s={}",
                    vad_batch.threshold,
                    vad_batch.min_silence_duration,
                    vad_batch.min_speech_duration,
                    vad_batch.max_speech_duration,
                    vad_batch.threshold_escalation_duration,
                    self.vad_buffer
                );
                let options = VadDetectorOptions {
                    threshold: vad_batch.threshold,
                    min_silence_duration: vad_batch.min_silence_duration,
                    min_speech_duration: vad_batch.min_speech_duration,
                    max_speech_duration: vad_batch.threshold_escalation_duration,
                    window_size: 512,
                    sample_rate: 16000,
                    num_threads: 1,
                };
                let buffer_size = (vad_batch.max_speech_duration
                    + if self.vad_buffer > 0.0 {
                        self.vad_buffer
                    } else {
                        vad_batch.buffer_seconds
                    })
                .max(30.0);
                match create_vad_config(vad_path, options) {
                    Ok(config) => VoiceActivityDetector::create(&config, buffer_size),
                    Err(err) => {
                        log::warn!(
                            "Failed to create stream VAD detector: {err}; falling back to fixed chunks"
                        );
                        None
                    }
                }
            } else {
                None
            }
        } else {
            None
        };

        struct SpeechSliceTask {
            samples: Vec<f32>,
            start_time: f64,
            duration: f64,
        }

        let (slice_tx, mut slice_rx) = tokio::sync::mpsc::channel::<SpeechSliceTask>(128);
        let worker_recognizer = Arc::clone(&recognizer);
        let worker_punctuation = effective_punctuation.clone();
        let worker_observer = Arc::clone(&observer);
        let output_segment_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let worker_count = Arc::clone(&output_segment_count);
        let worker_handle = tokio::spawn(async move {
            let mut segments = Vec::new();
            while let Some(task) = slice_rx.recv().await {
                if worker_observer.is_cancelled() {
                    break;
                }
                if let Some(segment) = process_speech_slice(
                    task.samples,
                    task.start_time,
                    task.duration,
                    Arc::clone(&worker_recognizer),
                    worker_punctuation.clone(),
                    Arc::clone(&worker_observer),
                )
                .await
                {
                    worker_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    segments.push(segment);
                }
            }
            segments
        });

        log::info!(
            "[BatchStream] segmentation instance_id={} effective={} fixed_chunk_s=15",
            diagnostic_id,
            if vad_detector.is_some() {
                "vad"
            } else {
                "fixed"
            },
        );
        let mut total_samples_streamed: usize = 0;
        let mut chunk_buffer = Vec::new();
        let fixed_chunk_size = 16000 * 15; // 15 seconds
        let mut last_reported_progress: f32 = 5.0;
        let mut stream_error = None;
        let mut slice_count = 0_usize;
        let mut next_audio_report = 16_000 * 60;
        loop {
            if observer.is_cancelled() {
                break;
            }

            let frame = match reader.next_samples(512).await {
                Ok(Some(samples)) => samples,
                Ok(None) => break, // EOF reached
                Err(err) => {
                    log::warn!(
                        "[BatchStream] read_interrupted instance_id={diagnostic_id} detail={}",
                        sanitize_stream_diagnostic(&err.to_string())
                    );
                    stream_error = Some(err);
                    break;
                }
            };

            total_samples_streamed += frame.len();
            if total_samples_streamed >= next_audio_report {
                log::info!(
                    "[BatchStream] audio_progress instance_id={} received_s={:.3} slices={} output_segments={} elapsed_ms={}",
                    diagnostic_id,
                    total_samples_streamed as f64 / 16000.0,
                    slice_count,
                    output_segment_count.load(std::sync::atomic::Ordering::Relaxed),
                    started_at.elapsed().as_millis(),
                );
                next_audio_report = total_samples_streamed + 16_000 * 60;
            }
            let elapsed_seconds = total_samples_streamed as f32 / 16000.0;
            let progress = ((1.0 - (-elapsed_seconds / 300.0).exp()) * 85.0 + 5.0).clamp(5.0, 95.0);
            if progress - last_reported_progress >= 1.0 {
                observer.on_progress(progress);
                last_reported_progress = progress;
            }

            if let Some(detector) = vad_detector.as_mut() {
                detector.accept_waveform(&frame);
                while !detector.is_empty() {
                    if let Some(speech_segment) = detector.front() {
                        let seg_samples = speech_segment.samples().to_vec();
                        let start_sample = speech_segment.start().max(0);
                        let start_time = start_sample as f64 / 16000.0;
                        let duration = seg_samples.len() as f64 / 16000.0;
                        slice_count += 1;
                        if slice_tx
                            .send(SpeechSliceTask {
                                samples: seg_samples,
                                start_time,
                                duration,
                            })
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    detector.pop();
                }
            } else {
                chunk_buffer.extend_from_slice(&frame);
                if chunk_buffer.len() >= fixed_chunk_size {
                    let chunk_duration = chunk_buffer.len() as f64 / 16000.0;
                    let chunk_start =
                        (total_samples_streamed - chunk_buffer.len()) as f64 / 16000.0;
                    slice_count += 1;
                    if slice_tx
                        .send(SpeechSliceTask {
                            samples: std::mem::take(&mut chunk_buffer),
                            start_time: chunk_start,
                            duration: chunk_duration,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }

        log::info!(
            "[BatchStream] read_end instance_id={} received_s={:.3} slices={} output_segments={} cancelled={} read_error={} elapsed_ms={}",
            diagnostic_id,
            total_samples_streamed as f64 / 16000.0,
            slice_count,
            output_segment_count.load(std::sync::atomic::Ordering::Relaxed),
            observer.is_cancelled(),
            stream_error.is_some(),
            started_at.elapsed().as_millis(),
        );
        if (total_samples_streamed > 0 || stream_error.is_some()) && !observer.is_cancelled() {
            reader.log_end_state(&diagnostic_id).await;
        }

        if observer.is_cancelled() {
            worker_handle.abort();
            reader.finish();
            return Err(AsrPortError::runtime("Task cancelled."));
        }

        if let Some(err) = stream_error {
            worker_handle.abort();
            reader.finish();
            return Err(err);
        }

        if total_samples_streamed == 0 {
            worker_handle.abort();
            let err_msg = reader.take_stderr_output().await;
            reader.finish();
            let msg = err_msg
                .map(|msg| sanitize_stream_diagnostic(&msg))
                .unwrap_or_else(|| "No audio data received from stream".to_string());
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!("Stream failed: {msg}"),
            ));
        }

        // Flush remaining audio in detector or chunk buffer
        if let Some(detector) = vad_detector.as_mut() {
            detector.flush();
            while !detector.is_empty() {
                if observer.is_cancelled() {
                    worker_handle.abort();
                    reader.finish();
                    return Err(AsrPortError::runtime("Task cancelled."));
                }
                if let Some(speech_segment) = detector.front() {
                    let seg_samples = speech_segment.samples().to_vec();
                    let start_sample = speech_segment.start().max(0);
                    let start_time = start_sample as f64 / 16000.0;
                    let duration = seg_samples.len() as f64 / 16000.0;
                    slice_count += 1;

                    if slice_tx
                        .send(SpeechSliceTask {
                            samples: seg_samples,
                            start_time,
                            duration,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                detector.pop();
            }
        } else if !chunk_buffer.is_empty() {
            if observer.is_cancelled() {
                worker_handle.abort();
                reader.finish();
                return Err(AsrPortError::runtime("Task cancelled."));
            }
            let chunk_duration = chunk_buffer.len() as f64 / 16000.0;
            let chunk_start =
                total_samples_streamed.saturating_sub(chunk_buffer.len()) as f64 / 16000.0;
            slice_count += 1;
            let _ = slice_tx
                .send(SpeechSliceTask {
                    samples: std::mem::take(&mut chunk_buffer),
                    start_time: chunk_start,
                    duration: chunk_duration,
                })
                .await;
        }

        drop(slice_tx);

        let results = worker_handle.await.map_err(|e| {
            AsrPortError::runtime(format!(
                "Transcription worker encountered an unexpected error: {e}"
            ))
        })?;

        log::info!(
            "[BatchStream] complete instance_id={} received_s={:.3} slices={} output_segments={} skipped_slices={} last_end_s={:.3} elapsed_ms={}",
            diagnostic_id,
            total_samples_streamed as f64 / 16000.0,
            slice_count,
            results.len(),
            slice_count.saturating_sub(results.len()),
            results
                .iter()
                .map(|segment| segment.end)
                .fold(0.0_f64, f64::max),
            started_at.elapsed().as_millis(),
        );
        reader.finish();
        observer.on_progress(100.0);
        Ok(results)
    }
}
