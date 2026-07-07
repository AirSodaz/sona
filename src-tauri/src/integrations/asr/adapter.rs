use super::SherpaError;
use super::state::AsrState;
use super::traits::{AsrBatchProcessor, AsrProviderAdapter, AsrStreamingSession};
use super::types::{
    AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest, TranscriptSegment,
};
use async_trait::async_trait;
use sona_core::ports::asr::validate_local_sherpa_mode;

#[derive(Debug, Clone, Copy)]
pub struct LocalSherpaAdapter;

#[async_trait]
impl AsrProviderAdapter for LocalSherpaAdapter {
    fn provider_id(&self) -> &'static str {
        "local_sherpa"
    }

    fn create_batch_processor(
        &self,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, SherpaError> {
        validate_local_sherpa_mode(request, AsrMode::Batch).map_err(SherpaError::Generic)?;
        Ok(Some(std::sync::Arc::new(LocalSherpaBatchProcessor)))
    }

    async fn create_streaming_session(
        &self,
        state: &AsrState,
        instance_id: &str,
        request: &AsrTranscriptionRequest,
    ) -> Result<Option<std::sync::Arc<dyn AsrStreamingSession>>, SherpaError> {
        validate_local_sherpa_mode(request, AsrMode::Streaming).map_err(SherpaError::Generic)?;

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
                *file_config,
                request.hotwords.clone(),
                Some(request.normalization_options),
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
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitter>,
        state: &AsrState,
        file_path: std::path::PathBuf,
        save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<sona_core::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, SherpaError> {
        let config = BatchTranscriptionRequest::from_local_sherpa_request(
            file_path,
            save_to_path,
            request,
            speaker_processing,
            instance_id,
        )
        .map_err(SherpaError::Generic)?;

        super::batch::process_batch_request_impl(emitter, state, config)
            .await
            .map_err(SherpaError::from)
    }
}
