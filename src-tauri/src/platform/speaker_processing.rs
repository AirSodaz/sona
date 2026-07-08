use crate::platform::paths::{PathKind, PathProvider};

pub use sona_core::transcription::speaker::{
    SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample,
};
pub use sona_core::transcription::transcript::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};
pub use sona_local_asr::speaker_processing::annotate_segments_with_speakers;

pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<sona_core::transcription::transcript::TranscriptSegment>,
    speaker_processing: Option<SpeakerProcessingConfig>,
) -> Result<Vec<sona_core::transcription::transcript::TranscriptSegment>, String> {
    sona_local_asr::speaker_processing::annotate_speaker_segments_from_file(
        file_path,
        segments,
        speaker_processing,
    )
    .await
}

pub async fn import_speaker_profile_sample(
    provider: &dyn PathProvider,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let app_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    sona_local_asr::speaker_processing::import_speaker_profile_sample(
        &app_data_dir,
        profile_id,
        source_path,
        source_name,
    )
    .await
}
