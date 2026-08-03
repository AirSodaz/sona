use super::types::{
    AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest, TranscriptSegment,
    TranscriptUpdate,
};
use super::{AsrBatchProcessor, AsrPortError, AsrProviderAdapter, AsrState};
use async_trait::async_trait;
use sona_core::export::ExportFormat;
use sona_core::ports::asr::{
    BatchTranscriberPort, BatchTranscriptionObserver, validate_local_asr_mode,
};
use sona_core::transcription::runtime::{BatchTranscribePlan, OutputTarget};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct LocalAsrAdapter;

#[async_trait]
impl AsrProviderAdapter for LocalAsrAdapter {
    fn provider_id(&self) -> &'static str {
        "local_sherpa"
    }

    fn create_batch_processor(
        &self,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<Arc<dyn AsrBatchProcessor>>, AsrPortError> {
        validate_local_asr_mode(request, AsrMode::Batch)?;
        Ok(Some(Arc::new(LocalAsrBatchProcessor)))
    }
}

pub struct LocalAsrBatchProcessor;

struct TauriBatchTranscriptionObserver {
    emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
    progress_path: String,
    instance_id: Option<String>,
}

impl BatchTranscriptionObserver for TauriBatchTranscriptionObserver {
    fn on_progress(&self, progress: f32) {
        let _ = self.emitter.emit(
            super::BATCH_PROGRESS_EVENT,
            serde_json::json!([&self.progress_path, progress, &self.instance_id]),
        );
    }

    fn on_transcript_update(&self, update: &TranscriptUpdate) {
        let Some(instance_id) = self.instance_id.as_deref() else {
            return;
        };
        let payload = match serde_json::to_value(update) {
            Ok(payload) => payload,
            Err(error) => {
                log::warn!("[ASR] failed to serialize batch transcript update: {error}");
                return;
            }
        };
        let _ = self
            .emitter
            .emit(&super::recognizer_output_event(instance_id), payload);
    }
}

#[async_trait]
impl AsrBatchProcessor for LocalAsrBatchProcessor {
    async fn process_file(
        &self,
        emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
        _state: &AsrState,
        file_path: std::path::PathBuf,
        save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let request = BatchTranscriptionRequest::from_local_asr_request(
            file_path,
            save_to_path,
            request,
            speaker_processing,
            instance_id,
        )?;
        let progress_path = request.file_path.to_string_lossy().into_owned();
        let observer = Arc::new(TauriBatchTranscriptionObserver {
            emitter,
            progress_path,
            instance_id: request.instance_id.clone(),
        });
        observer.on_progress(0.0);

        let normalization_options = request.normalization_options;
        let postprocessor = request.postprocessor.clone();
        let plan = BatchTranscribePlan {
            input_path: request.file_path,
            save_to_path: request.save_to_path,
            engine: request.engine,
            model_path: request.model_path,
            num_threads: request.num_threads,
            enable_itn: request.enable_itn,
            language: request.language,
            punctuation_model: request.punctuation_model,
            vad_model: request.vad_model,
            vad_buffer: request.vad_buffer,
            batch_segmentation_mode: request.batch_segmentation_mode,
            model_type: request.model_type,
            file_config: request.file_config,
            hotwords: request.hotwords,
            speaker_processing: request.speaker_processing,
            gpu_acceleration: request.gpu_acceleration,
            export_format: ExportFormat::Json,
            output_target: OutputTarget::Stdout,
            quiet: true,
        };
        let transcriber = sona_application::local_asr::LocalBatchTranscriberRouter::new(
            Arc::new(sona_local_asr::batch::LocalBatchAsrAdapter),
            Arc::new(sona_llama_asr::batch::LlamaBatchAsrAdapter),
        );
        let segments = transcriber
            .transcribe_with_observer(plan, observer.clone())
            .await?;
        let normalized =
            super::transcript::apply_timeline_normalization(segments, normalization_options);
        let output = postprocessor.process_segments(normalized);
        observer.on_transcript_update(&TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: output.clone(),
        });
        observer.on_progress(100.0);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::event::MockEventEmitter;
    use sona_core::ports::asr::{
        AsrEngineConfig, AsrPortErrorKind, LocalAsrEngine, OnlineAsrProviderRequest,
    };

    #[test]
    fn local_adapter_preserves_unsupported_engine_error_kind() {
        let request = AsrTranscriptionRequest {
            mode: AsrMode::Batch,
            language: "auto".to_string(),
            enable_itn: false,
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
            hotwords: None,
            speaker_processing: None,
            engine_config: AsrEngineConfig::Online {
                provider: OnlineAsrProviderRequest {
                    provider_id: "test".to_string(),
                    profile_id: "test".to_string(),
                    config: serde_json::Value::Null,
                },
            },
        };

        let error = match LocalAsrAdapter.create_batch_processor(&request) {
            Err(error) => error,
            Ok(_) => panic!("online request should be rejected"),
        };
        assert_eq!(error.kind, AsrPortErrorKind::Unsupported);
    }

    #[test]
    fn explicit_local_engine_selects_llama_without_model_heuristics() {
        let mut request = AsrTranscriptionRequest::local_sherpa(
            AsrMode::Batch,
            "C:/models/custom".to_string(),
            4,
            false,
            "auto".to_string(),
            None,
            None,
            5.0,
            "custom-model-type".to_string(),
            None,
            None,
            Default::default(),
            Default::default(),
            None,
            Some("cpu".to_string()),
        );
        let AsrEngineConfig::LocalSherpa { local_engine, .. } = &mut request.engine_config else {
            unreachable!();
        };
        *local_engine = LocalAsrEngine::LlamaCpp;

        let mapped = BatchTranscriptionRequest::from_local_asr_request(
            "input.wav".into(),
            None,
            request,
            None,
            None,
        )
        .unwrap();
        assert_eq!(mapped.engine, LocalAsrEngine::LlamaCpp);
    }

    #[test]
    fn batch_observer_correlates_progress_and_transcript_events() {
        let emitter = Arc::new(MockEventEmitter::new());
        let observer = TauriBatchTranscriptionObserver {
            emitter: emitter.clone(),
            progress_path: "C:/audio/demo.wav".to_string(),
            instance_id: Some("batch-1".to_string()),
        };
        let segment = TranscriptSegment {
            id: "segment-1".to_string(),
            text: "partial".to_string(),
            start: 0.0,
            end: 1.0,
            is_final: false,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        };

        observer.on_progress(42.0);
        observer.on_transcript_update(&TranscriptUpdate {
            remove_ids: Vec::new(),
            upsert_segments: vec![segment],
        });

        let emitted = emitter.emitted.lock().unwrap();
        assert_eq!(emitted[0].0, super::super::BATCH_PROGRESS_EVENT);
        assert_eq!(
            emitted[0].1,
            serde_json::json!(["C:/audio/demo.wav", 42.0, "batch-1"])
        );
        assert_eq!(emitted[1].0, "recognizer-output-batch-1");
        assert_eq!(emitted[1].1["upsertSegments"][0]["text"], "partial");
    }
}
