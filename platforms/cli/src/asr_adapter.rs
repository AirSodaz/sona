use sona_core::ports::asr::BatchTranscriberPort;
use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingSession};
use sona_core::ports::asr::{AsrTranscriptionRequest, OnlineBatchTranscriptionRequest};
use sona_core::transcription::runtime::LiveTranscribePlan;
use sona_core::transcription::transcript::TranscriptSegment;
use std::path::PathBuf;
use std::sync::Arc;

pub(crate) fn local_batch_transcriber() -> impl BatchTranscriberPort {
    let vad_engines = sona_vad::built_in_engines();
    let punct_engines = sona_punct::built_in_engines();
    let registry = sona_application::local_asr::LocalAsrRegistry::empty()
        .register(Arc::new(sona_sherpa_onnx::SherpaOnnxAdapter::new(
            sona_sherpa_onnx::runtime::RecognizerPool::default(),
            vad_engines.clone(),
            punct_engines.clone(),
        )))
        .register(Arc::new(sona_llama_cpp::LlamaCppAdapter::new(
            vad_engines,
            punct_engines,
        )));
    sona_application::local_asr::LocalBatchTranscriberRouter::new(registry)
}

pub(crate) async fn local_streaming_session(
    plan: &LiveTranscribePlan,
    instance_id: &str,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, String> {
    let session = sona_sherpa_onnx::streaming::create_streaming_session(
        sona_sherpa_onnx::runtime::RecognizerPool::default(),
        plan.to_local_streaming_request(instance_id),
        observer,
    )
    .await
    .map_err(|error| error.to_string())?;
    Ok(session)
}

pub(crate) async fn online_batch_transcribe(
    file_path: PathBuf,
    request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, sona_core::ports::asr::AsrPortError> {
    sona_online_asr::OnlineAsrAdapter
        .transcribe_batch(OnlineBatchTranscriptionRequest { file_path, request })
        .await
        .map(|output| output.segments)
}

pub(crate) fn online_streaming_session(
    request: AsrTranscriptionRequest,
    instance_id: &str,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, sona_core::ports::asr::AsrPortError> {
    sona_online_asr::OnlineAsrAdapter.create_streaming_session(
        instance_id.to_string(),
        request,
        observer,
    )
}
