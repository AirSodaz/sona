use sherpa_onnx::VoiceActivityDetector;
use sona_core::models::config::ModelFileConfig;
use sona_core::ports::aligner::AlignerEngineSet;
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, BatchSegmentationMode, BatchTranscriptionObserver,
};
use sona_core::ports::punctuation::{PunctuationEngineSet, PunctuationModel};
use sona_core::ports::vad::VadEngineSet;
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, ensure_transcript_segment_timing,
    normalize_recognizer_text, synthesize_durations,
};
use std::path::PathBuf;
use std::sync::Arc;

use crate::audio::{VadDetectorOptions, create_vad_config};
use crate::audio_stream::{AudioStreamReader, AudioStreamSourceConfig};
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
    let decoded = tokio::task::spawn_blocking(move || {
        let dec = decode_offline_samples(&recognizer, &samples)?;
        let cleaned = normalize_recognizer_text(&dec.text);
        if cleaned.is_empty() {
            return None;
        }
        let text = finalize_transcript_text(&cleaned, punctuation.as_deref());
        if text.is_empty() {
            return None;
        }
        Some((dec, text))
    })
    .await
    .ok()??;

    let (decoded, text) = decoded;
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

        let mut vad_detector = if let Some(vad_path) = self.vad_model.as_deref() {
            let options = VadDetectorOptions {
                threshold: 0.5,
                min_silence_duration: 0.6,
                min_speech_duration: 0.25,
                max_speech_duration: 15.0,
                window_size: 512,
                sample_rate: 16000,
                num_threads: 1,
            };
            let buffer_size = (15.0
                + if self.vad_buffer > 0.0 {
                    self.vad_buffer
                } else {
                    5.0
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
        };

        let mut results = Vec::new();
        let mut total_samples_streamed: usize = 0;
        let mut chunk_buffer = Vec::new();
        let fixed_chunk_size = 16000 * 15; // 15 seconds
        let mut last_reported_progress: f32 = 5.0;
        let mut stream_error = None;
        loop {
            if observer.is_cancelled() {
                break;
            }

            let frame = match reader.next_samples(512).await {
                Ok(Some(samples)) => samples,
                Ok(None) => break, // EOF reached
                Err(err) => {
                    log::warn!("Stream read interrupted: {err}");
                    stream_error = Some(err);
                    break;
                }
            };

            total_samples_streamed += frame.len();
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
                        if let Some(segment) = process_speech_slice(
                            seg_samples,
                            start_time,
                            duration,
                            Arc::clone(&recognizer),
                            effective_punctuation.clone(),
                            Arc::clone(&observer),
                        )
                        .await
                        {
                            results.push(segment);
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
                    if let Some(segment) = process_speech_slice(
                        std::mem::take(&mut chunk_buffer),
                        chunk_start,
                        chunk_duration,
                        Arc::clone(&recognizer),
                        effective_punctuation.clone(),
                        Arc::clone(&observer),
                    )
                    .await
                    {
                        results.push(segment);
                    }
                }
            }
        }

        if observer.is_cancelled() {
            reader.finish();
            return Err(AsrPortError::runtime("Task cancelled."));
        }

        if let Some(err) = stream_error {
            reader.finish();
            return Err(err);
        }

        if total_samples_streamed == 0 {
            let err_msg = reader.take_stderr_output().await;
            reader.finish();
            let msg = err_msg.unwrap_or_else(|| "No audio data received from stream".to_string());
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!("Stream failed: {msg}"),
            ));
        }

        // Flush remaining audio in detector or chunk buffer
        if let Some(detector) = vad_detector.as_mut() {
            detector.flush();
            while !detector.is_empty() {
                if let Some(speech_segment) = detector.front() {
                    let seg_samples = speech_segment.samples().to_vec();
                    let start_sample = speech_segment.start().max(0);
                    let start_time = start_sample as f64 / 16000.0;
                    let duration = seg_samples.len() as f64 / 16000.0;

                    if let Some(segment) = process_speech_slice(
                        seg_samples,
                        start_time,
                        duration,
                        Arc::clone(&recognizer),
                        effective_punctuation.clone(),
                        Arc::clone(&observer),
                    )
                    .await
                    {
                        results.push(segment);
                    }
                }
                detector.pop();
            }
        } else if !chunk_buffer.is_empty() {
            let chunk_duration = chunk_buffer.len() as f64 / 16000.0;
            let chunk_start =
                total_samples_streamed.saturating_sub(chunk_buffer.len()) as f64 / 16000.0;
            if let Some(segment) = process_speech_slice(
                std::mem::take(&mut chunk_buffer),
                chunk_start,
                chunk_duration,
                Arc::clone(&recognizer),
                effective_punctuation.clone(),
                Arc::clone(&observer),
            )
            .await
            {
                results.push(segment);
            }
        }

        reader.finish();
        observer.on_progress(100.0);
        Ok(results)
    }
}
