use super::AsrPortError;
use super::types::{
    AsrMode, AsrTranscriptionRequest, BatchTranscriptionRequest, TranscriptSegment,
};
use super::{AsrBatchProcessor, AsrProviderAdapter, AsrState};
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
    ) -> Result<Option<std::sync::Arc<dyn AsrBatchProcessor>>, AsrPortError> {
        validate_local_sherpa_mode(request, AsrMode::Batch)
            .map_err(|error| AsrPortError::invalid_request(error.to_string()))?;
        Ok(Some(std::sync::Arc::new(LocalSherpaBatchProcessor)))
    }
}

pub struct LocalSherpaBatchProcessor;

#[async_trait]
impl AsrBatchProcessor for LocalSherpaBatchProcessor {
    async fn process_file(
        &self,
        emitter: std::sync::Arc<dyn crate::platform::event::EventEmitterPort>,
        state: &AsrState,
        file_path: std::path::PathBuf,
        save_to_path: Option<std::path::PathBuf>,
        request: AsrTranscriptionRequest,
        speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
        instance_id: Option<String>,
    ) -> Result<Vec<TranscriptSegment>, AsrPortError> {
        let config = BatchTranscriptionRequest::from_local_sherpa_request(
            file_path,
            save_to_path,
            request,
            speaker_processing,
            instance_id,
        )?;

        super::batch::process_batch_request_impl(emitter, state, config)
            .await
            .map_err(AsrPortError::from)
    }
}
