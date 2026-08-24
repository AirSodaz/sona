use sona_core::ports::asr::{AsrRuntimeObserver, AsrTranscriptUpdateEvent};
use sona_core::transcription::postprocess::TranscriptNormalizationOptions;
use sona_core::transcription::transcript::{TranscriptSegment, TranscriptUpdate};

fn new_segment_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(super) fn normalize_segments(
    segments: Vec<TranscriptSegment>,
    options: TranscriptNormalizationOptions,
) -> Vec<TranscriptSegment> {
    sona_core::transcription::transcript::apply_timeline_normalization_with_id_generator(
        segments,
        options,
        new_segment_id,
    )
}

pub(super) fn build_transcript_update(
    segment: TranscriptSegment,
    options: TranscriptNormalizationOptions,
) -> TranscriptUpdate {
    sona_core::transcription::transcript::build_transcript_update_with_id_generator(
        segment,
        options,
        new_segment_id,
    )
}

pub(super) fn observe_transcript_update(
    observer: &dyn AsrRuntimeObserver,
    instance_id: &str,
    update: &TranscriptUpdate,
) {
    observer.on_transcript_update(&AsrTranscriptUpdateEvent {
        instance_id: instance_id.to_string(),
        stage: "volcengine_streaming".to_string(),
        update: update.clone(),
    });
}
