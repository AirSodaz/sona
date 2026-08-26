use crate::audio::{extract_and_resample_audio, save_wav_file};
use crate::gpu::{GpuFallbackNotice, resolve_gpu_acceleration_plan};
use crate::recognizer::{
    SafeOfflineRecognizer, build_offline_model_config, create_offline_recognizer,
    decode_offline_samples,
};
use async_trait::async_trait;
use sona_core::models::config::ModelFileConfig;
use sona_core::ports::asr::{
    AsrPortError, AsrPortErrorKind, BatchSegmentationMode, BatchTranscriberPort,
    BatchTranscriptionObserver, LocalAsrEngine, NoopBatchTranscriptionObserver,
    local_asr_engine_mismatch,
};
use sona_core::ports::punctuation::{
    PunctuationEngineSet, PunctuationModel, apply_optional_punctuation, load_configured_punctuation,
};
use sona_core::ports::vad::{VadDetectionOptions, VadEngineSet};
use sona_core::transcription::runtime::BatchTranscribePlan;
use sona_core::transcription::segmentation::{BATCH_SEGMENTATION_SAMPLE_RATE, segment_batch_audio};
use sona_core::transcription::transcript::{
    TranscriptSegment, TranscriptUpdate, ensure_transcript_segment_timing,
    normalize_recognizer_text, synthesize_durations,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct LocalBatchAsrAdapter {
    vad_engines: VadEngineSet,
    punctuation_engines: PunctuationEngineSet,
}

impl LocalBatchAsrAdapter {
    pub fn new(vad_engines: VadEngineSet, punctuation_engines: PunctuationEngineSet) -> Self {
        Self {
            vad_engines,
            punctuation_engines,
        }
    }
}

#[async_trait]
impl BatchTranscriberPort for LocalBatchAsrAdapter {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job =
            BatchTranscriptionJob::from_plan(plan, &self.vad_engines, &self.punctuation_engines)?;
        job.transcribe(Arc::new(NoopBatchTranscriptionObserver))
            .await
    }

    async fn transcribe_with_observer(
        &self,
        plan: BatchTranscribePlan,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job =
            BatchTranscriptionJob::from_plan(plan, &self.vad_engines, &self.punctuation_engines)?;
        job.transcribe(observer).await
    }
}

#[derive(Clone)]
struct BatchTranscriptionJob {
    input_path: PathBuf,
    save_to_path: Option<PathBuf>,
    model_path: PathBuf,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    punctuation_model: Option<PathBuf>,
    vad_model: Option<PathBuf>,
    vad_buffer: f32,
    batch_segmentation_mode: BatchSegmentationMode,
    model_type: String,
    file_config: Option<ModelFileConfig>,
    hotwords: Option<String>,
    speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
    gpu_acceleration: Option<String>,
    quiet: bool,
    vad_engines: VadEngineSet,
    punct_engines: PunctuationEngineSet,
}

impl BatchTranscriptionJob {
    fn from_plan(
        plan: BatchTranscribePlan,
        vad_engines: &VadEngineSet,
        punct_engines: &PunctuationEngineSet,
    ) -> Result<Self, AsrPortError> {
        if plan.engine != LocalAsrEngine::SherpaOnnx {
            return Err(local_asr_engine_mismatch(
                LocalAsrEngine::SherpaOnnx,
                plan.engine,
            ));
        }
        if !plan.input_path.is_file() {
            return Err(AsrPortError::new(
                AsrPortErrorKind::InvalidRequest,
                format!(
                    "Input file must be an existing file: {}",
                    plan.input_path.display()
                ),
            ));
        }
        Ok(Self {
            input_path: plan.input_path,
            save_to_path: plan.save_to_path,
            model_path: PathBuf::from(plan.model_path),
            num_threads: plan.num_threads,
            enable_itn: plan.enable_itn,
            language: plan.language,
            punctuation_model: plan.punctuation_model.map(PathBuf::from),
            vad_model: plan.vad_model.map(PathBuf::from),
            vad_buffer: plan.vad_buffer,
            batch_segmentation_mode: plan.batch_segmentation_mode,
            model_type: plan.model_type,
            file_config: plan.file_config,
            hotwords: plan.hotwords,
            speaker_processing: plan.speaker_processing,
            gpu_acceleration: plan.gpu_acceleration,
            quiet: plan.quiet,
            vad_engines: vad_engines.clone(),
            punct_engines: punct_engines.clone(),
        })
    }

    async fn transcribe(
        self,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let gpu_plan = resolve_gpu_acceleration_plan(self.gpu_acceleration.as_deref()).await;
        let mut last_error = None;
        let mut fallback_notice: Option<GpuFallbackNotice> = None;

        for provider in gpu_plan.provider_options() {
            match self
                .transcribe_with_provider(provider.as_deref(), Arc::clone(&observer))
                .await
            {
                Ok(segments) => {
                    if let Some(notice) = fallback_notice.take()
                        && !self.quiet
                    {
                        eprintln!(
                            "DirectML transcription failed, retrying with CPU: {}",
                            notice.error
                        );
                    }
                    return Ok(segments);
                }
                Err(error)
                    if provider
                        .as_deref()
                        .map(|provider| gpu_plan.should_retry_after_failure(provider))
                        .unwrap_or(false) =>
                {
                    fallback_notice = Some(GpuFallbackNotice::directml_retry(error.to_string()));
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
        }

        Err(last_error.unwrap_or_else(|| AsrPortError::runtime("Recognizer creation failed.")))
    }

    async fn transcribe_with_provider(
        &self,
        provider: Option<&str>,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let model_type = build_offline_model_config(
            &self.model_path,
            &self.model_type,
            &self.file_config,
            self.enable_itn,
            &self.language,
            self.hotwords.clone(),
        )?;

        let recognizer = create_offline_recognizer(model_type, self.num_threads, provider)?;
        let punctuation =
            load_configured_punctuation(&self.punct_engines, self.punctuation_model.as_deref())?;
        let samples = extract_and_resample_audio(&self.input_path, 16000).await?;
        observer.on_progress(5.0);
        if let Some(path) = self.save_to_path.as_ref() {
            save_wav_file(&samples, 16000, path).map_err(|error| {
                AsrPortError::new(
                    AsrPortErrorKind::FileSystem,
                    format!("Failed to save resampled audio {}: {error}", path.display()),
                )
            })?;
        }

        let segments = transcribe_samples(
            &samples,
            &recognizer,
            punctuation.as_deref(),
            &self.vad_engines,
            self.vad_model.as_deref(),
            self.vad_buffer,
            self.batch_segmentation_mode,
            observer.as_ref(),
        )?;
        let segments = crate::speaker_processing::annotate_segments_with_speakers(
            &samples,
            &segments,
            self.speaker_processing.as_ref(),
        )?;
        observer.on_transcript_update(&TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: segments.clone(),
        });
        observer.on_progress(98.0);
        Ok(segments)
    }
}

#[allow(clippy::too_many_arguments)]
fn transcribe_samples(
    samples: &[f32],
    recognizer: &SafeOfflineRecognizer,
    punctuation: Option<&dyn PunctuationModel>,
    vad_engines: &VadEngineSet,
    vad_model: Option<&Path>,
    vad_buffer: f32,
    batch_segmentation_mode: BatchSegmentationMode,
    observer: &dyn BatchTranscriptionObserver,
) -> Result<Vec<TranscriptSegment>, AsrPortError> {
    let vad_engine = vad_engines.resolve(vad_model);
    let mut vad_options = VadDetectionOptions::batch_defaults(vad_model.unwrap_or(Path::new("")));
    vad_options.buffer_seconds = vad_buffer;
    let audio_segments = segment_batch_audio(
        samples,
        BATCH_SEGMENTATION_SAMPLE_RATE,
        batch_segmentation_mode,
        vad_engine.as_deref(),
        &vad_options,
    );

    let total_duration = samples.len() as f32 / 16_000.0;
    let mut results = Vec::new();
    for segment in audio_segments {
        if let Some(result) = decode_offline_samples(recognizer, &segment.samples) {
            let cleaned_text = normalize_recognizer_text(&result.text);
            if cleaned_text.is_empty() {
                continue;
            }

            let text = finalize_transcript_text(&cleaned_text, punctuation);
            if text.is_empty() {
                continue;
            }

            let timestamps_abs = result.timestamps.as_ref().map(|timestamps| {
                timestamps
                    .iter()
                    .map(|timestamp| *timestamp + segment.start_time)
                    .collect::<Vec<_>>()
            });
            let durations = timestamps_abs
                .as_ref()
                .and_then(|timestamps| synthesize_durations(timestamps, segment.end_time()));

            let mut transcript_segment = TranscriptSegment {
                id: uuid::Uuid::new_v4().to_string(),
                text,
                start: segment.start_time as f64,
                end: segment.end_time() as f64,
                is_final: true,
                timing: None,
                tokens: Some(result.tokens),
                timestamps: timestamps_abs,
                durations,
                translation: None,
                speaker: None,
                speaker_attribution: None,
            };

            ensure_transcript_segment_timing(&mut transcript_segment);
            observer.on_transcript_update(&TranscriptUpdate {
                remove_ids: Vec::new(),
                upsert_segments: vec![transcript_segment.clone()],
            });
            results.push(transcript_segment);
        }
        let processed = if total_duration > 0.0 {
            (segment.end_time() / total_duration).clamp(0.0, 1.0)
        } else {
            1.0
        };
        observer.on_progress(5.0 + processed * 90.0);
    }

    Ok(results)
}

fn finalize_transcript_text(
    cleaned_text: &str,
    punctuation: Option<&dyn PunctuationModel>,
) -> String {
    let result = cleaned_text.trim().to_string();
    if result.is_empty() {
        return result;
    }

    apply_optional_punctuation(punctuation, &result)
}

#[cfg(test)]
mod tests {
    use super::LocalBatchAsrAdapter;
    use sona_core::export::ExportFormat;
    use sona_core::ports::asr::BatchTranscriberPort;
    use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
    use std::path::PathBuf;

    #[tokio::test]
    async fn batch_transcription_rejects_missing_input_file() {
        let plan = BatchTranscribePlan {
            input_path: PathBuf::from("missing.wav"),
            save_to_path: None,
            engine: sona_core::ports::asr::LocalAsrEngine::SherpaOnnx,
            model_path: "C:/models/demo".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
            model_type: "whisper".to_string(),
            file_config: None,
            hotwords: None,
            speaker_processing: None,
            gpu_acceleration: Some("cpu".to_string()),
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        };

        let error = LocalBatchAsrAdapter::default()
            .transcribe(plan)
            .await
            .unwrap_err();
        assert_eq!(
            error.kind,
            sona_core::ports::asr::AsrPortErrorKind::InvalidRequest
        );
        assert!(error.message.contains("existing file"));
    }

    #[tokio::test]
    async fn adapter_mismatch_uses_shared_error_contract() {
        let mut plan = BatchTranscribePlan {
            input_path: PathBuf::from("missing.wav"),
            save_to_path: None,
            engine: sona_core::ports::asr::LocalAsrEngine::SherpaOnnx,
            model_path: "C:/models/demo".to_string(),
            num_threads: 4,
            enable_itn: false,
            language: "auto".to_string(),
            punctuation_model: None,
            vad_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
            model_type: "whisper".to_string(),
            file_config: None,
            hotwords: None,
            speaker_processing: None,
            gpu_acceleration: Some("cpu".to_string()),
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        };
        plan.engine = sona_core::ports::asr::LocalAsrEngine::LlamaCpp;

        let error = LocalBatchAsrAdapter::default()
            .transcribe(plan)
            .await
            .unwrap_err();
        assert_eq!(
            error.kind,
            sona_core::ports::asr::AsrPortErrorKind::Unsupported
        );
        assert_eq!(
            error.message,
            "Local ASR adapter 'sherpa-onnx' cannot execute engine 'llama-cpp'."
        );
    }
}
