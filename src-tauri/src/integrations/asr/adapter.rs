use super::TranscriptPostprocessor;
use super::error::SherpaError;
use super::state::AsrState;
use super::traits::{AsrBatchProcessor, AsrProviderAdapter, AsrStreamingSession};
use super::types::{
    AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest, TranscriptSegment,
};
use async_trait::async_trait;

#[derive(Debug, Clone, Copy)]
pub struct LocalSherpaAdapter;

impl LocalSherpaAdapter {
    pub fn ensure_mode(request: &AsrTranscriptionRequest, expected: AsrMode) -> Result<(), String> {
        if request.engine() != super::types::AsrEngine::LocalSherpa {
            return Err("Unsupported ASR engine for local Sherpa adapter".to_string());
        }
        if request.mode != expected {
            return Err(format!(
                "ASR request mode mismatch: expected {:?}, got {:?}",
                expected, request.mode
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl AsrProviderAdapter for LocalSherpaAdapter {
    fn provider_id(&self) -> &'static str {
        "local_sherpa"
    }

    fn create_batch_processor(
        &self,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError> {
        Self::ensure_mode(request, AsrMode::Offline).map_err(SherpaError::Generic)?;
        Ok(Some(std::sync::Arc::new(LocalSherpaBatchProcessor)))
    }

    async fn create_streaming_session(
        &self,
        state: &AsrState,
        instance_id: &str,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrStreamingSession>>, SherpaError> {
        Self::ensure_mode(request, AsrMode::Streaming).map_err(SherpaError::Generic)?;

        if let crate::integrations::asr::types::AsrEngineConfig::LocalSherpa {
            model_path,
            num_threads,
            punctuation_model,
            vad_model,
            vad_buffer,
            model_type,
            file_config,
            gpu_acceleration,
            ..
        } = request.engine_config.clone()
        {
            let session = super::sherpa_onnx::init_recognizer_impl(
                state,
                instance_id,
                model_path,
                num_threads,
                request.enable_itn,
                request.language.clone(),
                punctuation_model,
                vad_model,
                vad_buffer,
                model_type,
                file_config,
                request.hotwords.clone(),
                Some(request.normalization_options.clone()),
                Some(request.postprocess_options.clone()),
                gpu_acceleration,
            )
            .await
            .map_err(SherpaError::Generic)?;
            Ok(Some(session))
        } else {
            Err(SherpaError::Generic(
                "Expected LocalSherpa engine config".to_string(),
            ))
        }
    }
}

pub struct LocalSherpaBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for LocalSherpaBatchProcessor {
    async fn process_file(
        &self,
        emitter: std::sync::Arc<dyn crate::core::event::EventEmitter>,
        state: &AsrState,
        file_path: String,
        save_to_path: Option<String>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<crate::integrations::speaker::SpeakerProcessingConfig>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        let config = match request.engine_config.clone() {
            crate::integrations::asr::types::AsrEngineConfig::LocalSherpa {
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config,
                gpu_acceleration,
                ..
            } => BatchTranscriptionRequest {
                file_path,
                save_to_path,
                model_path,
                num_threads,
                enable_itn: request.enable_itn,
                language: request.language,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode,
                model_type,
                file_config,
                hotwords: request.hotwords,
                speaker_processing,
                normalization_options: request.normalization_options.clone(),
                postprocessor: TranscriptPostprocessor::compile(
                    request.postprocess_options.clone(),
                )?,
                gpu_acceleration,
            },
            _ => {
                return Err(SherpaError::Generic(
                    "Expected LocalSherpa engine config".to_string(),
                ));
            }
        };

        super::batch::process_batch_request_impl(emitter, state, config)
            .await
            .map_err(SherpaError::from)
    }
}
