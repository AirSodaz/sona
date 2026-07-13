use sona_core::ports::asr::BatchTranscriber;
use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingSession};
use sona_core::transcription::runtime::LiveTranscribePlan;
use std::sync::Arc;

pub(crate) fn local_batch_transcriber() -> impl BatchTranscriber {
    sona_local_asr::batch::LocalBatchAsrAdapter
}

pub(crate) async fn local_streaming_session(
    plan: &LiveTranscribePlan,
    instance_id: &str,
    observer: Arc<dyn AsrRuntimeObserver>,
) -> Result<Arc<dyn AsrStreamingSession>, String> {
    let session = sona_local_asr::streaming::create_streaming_session(
        sona_local_asr::runtime::RecognizerPool::default(),
        plan.to_local_streaming_request(instance_id),
        observer,
    )
    .await?;
    Ok(session)
}
