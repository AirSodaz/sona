use crate::audio::{extract_and_resample_audio_with_ffmpeg, save_wav_file};
use crate::gpu::{GpuFallbackNotice, is_int8_model, resolve_gpu_acceleration_plan};
use crate::recognizer::{
    SafeOfflineRecognizer, build_offline_model_config, create_offline_recognizer,
    decode_offline_samples,
};
use async_trait::async_trait;
use sona_core::models::config::ModelFileConfig;
use sona_core::ports::aligner::{AlignerEngineSet, load_configured_aligner};
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

#[derive(Clone)]
pub struct LocalBatchAsrAdapter {
    vad_engines: VadEngineSet,
    punctuation_engines: PunctuationEngineSet,
    aligner_engines: AlignerEngineSet,
}

impl Default for LocalBatchAsrAdapter {
    fn default() -> Self {
        Self {
            vad_engines: VadEngineSet::default(),
            punctuation_engines: PunctuationEngineSet::default(),
            aligner_engines: default_aligner_engines(),
        }
    }
}

pub fn default_aligner_engines() -> AlignerEngineSet {
    AlignerEngineSet::empty().register(Arc::new(crate::aligner::SherpaCtcAlignerEngine))
}

impl LocalBatchAsrAdapter {
    pub fn new(vad_engines: VadEngineSet, punctuation_engines: PunctuationEngineSet) -> Self {
        Self {
            vad_engines,
            punctuation_engines,
            aligner_engines: default_aligner_engines(),
        }
    }

    pub fn with_aligner_engines(mut self, aligner_engines: AlignerEngineSet) -> Self {
        self.aligner_engines = aligner_engines;
        self
    }
}

#[async_trait]
impl BatchTranscriberPort for LocalBatchAsrAdapter {
    async fn transcribe(
        &self,
        plan: BatchTranscribePlan,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job = BatchTranscriptionJob::from_plan(
            plan,
            &self.vad_engines,
            &self.punctuation_engines,
            &self.aligner_engines,
        )?;
        job.transcribe(Arc::new(NoopBatchTranscriptionObserver))
            .await
    }

    async fn transcribe_with_observer(
        &self,
        plan: BatchTranscribePlan,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let job = BatchTranscriptionJob::from_plan(
            plan,
            &self.vad_engines,
            &self.punctuation_engines,
            &self.aligner_engines,
        )?;
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
    alignment_model: Option<PathBuf>,
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
    aligner_engines: AlignerEngineSet,
    ffmpeg_path: Option<PathBuf>,
}

impl BatchTranscriptionJob {
    fn from_plan(
        plan: BatchTranscribePlan,
        vad_engines: &VadEngineSet,
        punct_engines: &PunctuationEngineSet,
        aligner_engines: &AlignerEngineSet,
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
            alignment_model: plan.alignment_model.map(PathBuf::from),
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
            aligner_engines: aligner_engines.clone(),
            ffmpeg_path: plan.ffmpeg_path.map(PathBuf::from),
        })
    }

    async fn transcribe(
        self,
        observer: Arc<dyn BatchTranscriptionObserver>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let is_int8 = is_int8_model(&self.model_path, self.file_config.as_ref());
        let gpu_plan =
            resolve_gpu_acceleration_plan(self.gpu_acceleration.as_deref(), is_int8).await;
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
        let is_same_model = self
            .alignment_model
            .as_deref()
            .is_some_and(|align_path| is_same_model_target(align_path, &self.model_path));

        let aligner = if is_same_model {
            log::info!(
                "ASR model and CTC alignment model target the same model ({}); skipping redundant CTC alignment pass.",
                self.model_path.display()
            );
            None
        } else {
            load_configured_aligner(&self.aligner_engines, self.alignment_model.as_deref())
                .map_err(|err| AsrPortError::new(AsrPortErrorKind::Model, err.to_string()))?
        };
        let punctuation =
            load_configured_punctuation(&self.punct_engines, self.punctuation_model.as_deref())?;

        let model_type = build_offline_model_config(
            &self.model_path,
            &self.model_type,
            &self.file_config,
            self.enable_itn,
            &self.language,
            self.hotwords.clone(),
        )?;

        let recognizer = create_offline_recognizer(model_type, self.num_threads, provider)?;
        let samples = extract_and_resample_audio_with_ffmpeg(
            &self.input_path,
            16000,
            self.ffmpeg_path.as_deref(),
        )
        .await?;
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
        let segments = if let Some(aligner) = aligner.as_ref() {
            observer.on_progress(92.0);
            match aligner.align_segments(&samples, 16000, &segments).await {
                Ok(aligned) => aligned,
                Err(err) => {
                    log::warn!(
                        "Forced alignment failed, falling back to unaligned segments: {err}"
                    );
                    segments
                }
            }
        } else {
            segments
        };
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
    let is_funasr_nano = recognizer.model_type() == "funasr-nano";
    let batch_segmentation_mode = if is_batch_vad_forced_model(recognizer.model_type()) {
        BatchSegmentationMode::Vad
    } else {
        batch_segmentation_mode
    };
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
    let effective_punctuation = if is_funasr_nano { None } else { punctuation };

    let total_duration = samples.len() as f32 / 16_000.0;
    let mut results = Vec::new();
    for segment in audio_segments {
        if let Some(result) = decode_offline_samples(recognizer, &segment.samples) {
            let cleaned_text = normalize_recognizer_text(&result.text);
            if cleaned_text.is_empty() {
                continue;
            }

            let text = finalize_transcript_text(&cleaned_text, effective_punctuation);
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

fn is_batch_vad_forced_model(model_type: &str) -> bool {
    !matches!(model_type, "qwen3-asr" | "parakeet-tdt")
}

fn is_same_model_target(path_a: &Path, path_b: &Path) -> bool {
    let can_a = std::fs::canonicalize(path_a).unwrap_or_else(|_| path_a.to_path_buf());
    let can_b = std::fs::canonicalize(path_b).unwrap_or_else(|_| path_b.to_path_buf());

    if can_a == can_b {
        return true;
    }

    let a_is_file = can_a.is_file();
    let b_is_file = can_b.is_file();

    // If both are files or both are directories, but not equal, they are distinct.
    if (a_is_file && b_is_file) || (!a_is_file && !b_is_file) {
        return false;
    }

    // Exactly one is a file and one is a directory.
    // The file is the same model target if it lives directly inside that directory
    // and is a recognized model file (e.g., model.onnx, model.int8.onnx).
    let (file_path, dir_path) = if a_is_file {
        (&can_a, &can_b)
    } else {
        (&can_b, &can_a)
    };

    if file_path.parent() == Some(dir_path)
        && let Some(file_name) = file_path.file_name().and_then(|n| n.to_str())
    {
        return matches!(
            file_name,
            "model.onnx" | "model.int8.onnx" | "model.fp16.onnx"
        );
    }

    false
}
#[cfg(test)]
mod tests {
    use super::*;
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
            alignment_model: None,
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
            ffmpeg_path: None,
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
            alignment_model: None,
            vad_buffer: 5.0,
            batch_segmentation_mode: sona_core::ports::asr::BatchSegmentationMode::Vad,
            model_type: "whisper".to_string(),
            file_config: None,
            hotwords: None,
            ffmpeg_path: None,
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

    #[test]
    fn adapter_configures_aligner_engine_set() {
        let adapter = LocalBatchAsrAdapter::default();
        assert_eq!(adapter.aligner_engines.engines().len(), 1);
        assert_eq!(
            adapter.aligner_engines.engines()[0].engine_kind(),
            sona_core::ports::aligner::AlignerEngineKind::CtcTrellisOnnx
        );
    }
    #[test]
    fn test_is_same_model_target() {
        let temp_dir = std::env::temp_dir().join("test_is_same_model_target");
        let _ = std::fs::create_dir_all(&temp_dir);
        let model_file = temp_dir.join("model.onnx");
        let _ = std::fs::write(&model_file, b"test");

        // Same path directly
        assert!(is_same_model_target(&temp_dir, &temp_dir));

        // Directory vs file inside directory
        assert!(is_same_model_target(&temp_dir, &model_file));
        assert!(is_same_model_target(&model_file, &temp_dir));

        // Distinct files in the same directory must NOT be considered the same target
        let other_file = temp_dir.join("other_model.onnx");
        let _ = std::fs::write(&other_file, b"test2");
        assert!(!is_same_model_target(&model_file, &other_file));
        assert!(!is_same_model_target(&temp_dir, &other_file));
        // Different paths
        let other_dir = std::env::temp_dir().join("test_other_model");
        assert!(!is_same_model_target(&temp_dir, &other_dir));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
