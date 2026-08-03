use super::types::{
    AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest, TranscriptSegment,
};
use super::{AsrBatchProcessor, AsrPortError, AsrProviderAdapter, AsrState};
use async_trait::async_trait;
use sona_core::export::ExportFormat;
use sona_core::ports::asr::{BatchTranscriberPort, validate_local_asr_mode};
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
        let _ = emitter.emit(
            super::BATCH_PROGRESS_EVENT,
            serde_json::json!([&progress_path, 0.0]),
        );

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
        let segments = transcriber.transcribe(plan).await?;
        let normalized =
            super::transcript::apply_timeline_normalization(segments, normalization_options);
        let output = postprocessor.process_segments(normalized);

        let _ = emitter.emit(
            super::BATCH_PROGRESS_EVENT,
            serde_json::json!([&progress_path, 100.0]),
        );
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
}
