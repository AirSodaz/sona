use tauri::AppHandle;

#[tauri::command]
pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<crate::integrations::asr::TranscriptSegment>,
    speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
) -> Result<Vec<crate::integrations::asr::TranscriptSegment>, String> {
    crate::platform::speaker_processing::annotate_speaker_segments_from_file(
        file_path,
        segments,
        speaker_processing,
    )
    .await
}

#[tauri::command]
pub async fn import_speaker_profile_sample(
    app: AppHandle,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<sona_core::transcription::speaker::SpeakerProfileSample, String> {
    crate::platform::speaker_processing::import_speaker_profile_sample_for_app(
        &app,
        profile_id,
        source_path,
        source_name,
    )
    .await
}

#[tauri::command]
pub async fn enroll_speaker_profile_sample_from_audio(
    app: AppHandle,
    profile_id: String,
    source_audio_path: String,
    start_seconds: f64,
    end_seconds: f64,
    sample_name: Option<String>,
) -> Result<sona_core::transcription::speaker::SpeakerProfileSample, String> {
    crate::platform::speaker_processing::enroll_speaker_profile_sample_for_app(
        &app,
        profile_id,
        source_audio_path,
        start_seconds,
        end_seconds,
        sample_name,
    )
    .await
}

#[tauri::command]
pub fn build_speaker_review_snapshot(
    segments: Vec<crate::integrations::asr::TranscriptSegment>,
    active_filter: sona_core::transcription::speaker_review::SpeakerReviewFilter,
) -> sona_core::transcription::speaker_review::SpeakerReviewSnapshot {
    sona_core::transcription::speaker_review::build_speaker_review_snapshot(segments, active_filter)
}

#[tauri::command]
pub async fn apply_speaker_profile_to_group(
    request: sona_core::transcription::speaker_correction::ApplySpeakerProfileToGroupRequest,
) -> Result<sona_core::transcription::speaker_correction::SpeakerCorrectionResponse, String> {
    sona_core::transcription::speaker_correction::apply_speaker_profile_to_group_impl(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn reset_speaker_group_to_anonymous(
    request: sona_core::transcription::speaker_correction::SpeakerGroupRequest,
) -> Result<sona_core::transcription::speaker_correction::SpeakerCorrectionResponse, String> {
    sona_core::transcription::speaker_correction::reset_speaker_group_to_anonymous_impl(request)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn confirm_speaker_group_review(
    request: sona_core::transcription::speaker_correction::SpeakerGroupRequest,
) -> Result<sona_core::transcription::speaker_correction::SpeakerCorrectionResponse, String> {
    sona_core::transcription::speaker_correction::confirm_speaker_group_review_impl(request)
        .map_err(|error| error.to_string())
}
