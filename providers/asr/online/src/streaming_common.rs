use sona_core::ports::asr::{AsrRuntimeObserver, AsrStreamingErrorEvent, AsrTranscriptUpdateEvent};
use sona_core::transcription::postprocess::TranscriptNormalizationOptions;
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};
use tokio::sync::{Mutex, Notify};
use tokio_tungstenite::tungstenite::http::header::{HeaderName, HeaderValue};

use crate::SherpaError;

pub fn new_segment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub fn normalize_segments(
    segments: Vec<TranscriptSegment>,
    options: TranscriptNormalizationOptions,
) -> Vec<TranscriptSegment> {
    sona_core::transcription::transcript::apply_timeline_normalization_with_id_generator(
        segments,
        options,
        new_segment_id,
    )
}

pub fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    sona_core::transcription::transcript::build_transcript_update_with_id_generator(
        segment,
        options,
        new_segment_id,
    )
}

pub fn observe_transcript_update(
    observer: &dyn AsrRuntimeObserver,
    instance_id: &str,
    stage: &str,
    update: &TranscriptUpdate,
) {
    observer.on_transcript_update(&AsrTranscriptUpdateEvent {
        instance_id: instance_id.to_string(),
        stage: stage.to_string(),
        update: update.clone(),
    });
}

pub fn observe_streaming_error(
    observer: &dyn AsrRuntimeObserver,
    instance_id: &str,
    error: &SherpaError,
) {
    observer.on_streaming_error(&AsrStreamingErrorEvent {
        instance_id: instance_id.to_string(),
        code: error.code().to_string(),
        message: error.to_string(),
    });
}

pub async fn publish_reader_outcome(
    outcome: &Mutex<Option<Result<(), SherpaError>>>,
    notification: &Notify,
    value: Result<(), SherpaError>,
) {
    let mut outcome = outcome.lock().await;
    if outcome.is_none() {
        *outcome = Some(value);
        notification.notify_waiters();
    }
}

pub fn insert_header(
    headers: &mut tokio_tungstenite::tungstenite::http::HeaderMap,
    name: &'static str,
    value: &str,
    provider: &'static str,
) -> Result<(), SherpaError> {
    let header_name = HeaderName::from_bytes(name.as_bytes()).map_err(|error| {
        SherpaError::StreamingEndpointInvalid {
            provider,
            error: error.to_string(),
        }
    })?;
    headers.insert(
        header_name,
        HeaderValue::from_str(value).map_err(|error| SherpaError::StreamingEndpointInvalid {
            provider,
            error: error.to_string(),
        })?,
    );
    Ok(())
}
