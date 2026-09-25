use crate::platform::paths::{PathKind, PathPort};

pub use sona_core::transcription::speaker::{
    SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample,
};
pub use sona_core::transcription::transcript::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};
pub use sona_sherpa_onnx::speaker_processing::{
    annotate_segments_with_speakers, match_cloud_speaker_segments_from_file,
    match_cloud_speakers_with_profiles,
};

pub async fn match_cloud_speakers_from_file(
    file_path: &std::path::Path,
    segments: Vec<sona_core::transcription::transcript::TranscriptSegment>,
    speaker_processing: Option<&SpeakerProcessingConfig>,
) -> Result<Vec<sona_core::transcription::transcript::TranscriptSegment>, String> {
    sona_sherpa_onnx::speaker_processing::match_cloud_speaker_segments_from_file(
        file_path,
        segments,
        speaker_processing,
    )
    .await
    .map_err(|error| error.to_string())
}
pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<sona_core::transcription::transcript::TranscriptSegment>,
    speaker_processing: Option<SpeakerProcessingConfig>,
) -> Result<Vec<sona_core::transcription::transcript::TranscriptSegment>, String> {
    sona_sherpa_onnx::speaker_processing::annotate_speaker_segments_from_file(
        file_path,
        segments,
        speaker_processing,
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn import_speaker_profile_sample(
    provider: &dyn PathPort,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let app_data_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    sona_sherpa_onnx::speaker_processing::import_speaker_profile_sample(
        &app_data_dir,
        profile_id,
        source_path,
        source_name,
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn import_speaker_profile_sample_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    import_speaker_profile_sample(&provider, profile_id, source_path, source_name).await
}

pub async fn enroll_speaker_profile_sample_from_audio(
    provider: &dyn PathPort,
    profile_id: String,
    source_audio_path: String,
    start_seconds: f64,
    end_seconds: f64,
    sample_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let app_data_dir = provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(|error| error.to_string())?;
    sona_sherpa_onnx::speaker_processing::enroll_speaker_profile_sample_from_audio(
        &app_data_dir,
        profile_id,
        source_audio_path,
        start_seconds,
        end_seconds,
        sample_name,
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn enroll_speaker_profile_sample_for_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    profile_id: String,
    source_audio_path: String,
    start_seconds: f64,
    end_seconds: f64,
    sample_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let provider = crate::platform::paths::TauriPathProvider::from_app(app);
    enroll_speaker_profile_sample_from_audio(
        &provider,
        profile_id,
        source_audio_path,
        start_seconds,
        end_seconds,
        sample_name,
    )
    .await
}
