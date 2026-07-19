use sona_core::transcription::speaker::{SpeakerProfile, SpeakerProfileSample};
use sona_core::transcription::speaker_correction::{
    ApplySpeakerProfileToGroupRequest, SpeakerGroupRequest, apply_speaker_profile_to_group_impl,
    reset_speaker_group_to_anonymous_impl,
};
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment,
};
use sona_core::transcription::{SpeakerCorrectionError, TranscriptPostprocessError};

fn sample_profile(id: &str, name: &str) -> SpeakerProfile {
    SpeakerProfile {
        id: id.to_string(),
        name: name.to_string(),
        enabled: true,
        samples: vec![SpeakerProfileSample {
            id: format!("{id}-sample"),
            file_path: format!("{id}.wav"),
            source_name: format!("{name}.wav"),
            duration_seconds: 1.5,
        }],
    }
}

fn segment(group_id: &str) -> TranscriptSegment {
    TranscriptSegment {
        id: "seg-1".to_string(),
        text: "hello".to_string(),
        start: 0.0,
        end: 1.0,
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: Some(SpeakerTag {
            id: group_id.to_string(),
            label: "Speaker 1".to_string(),
            kind: "anonymous".to_string(),
            score: None,
        }),
        speaker_attribution: Some(SpeakerAttribution {
            group_id: group_id.to_string(),
            anonymous_label: "Speaker 1".to_string(),
            state: "suggested".to_string(),
            source: "auto".to_string(),
            confidence: "medium".to_string(),
            candidates: vec![SpeakerCandidate {
                profile_id: "speaker-2".to_string(),
                profile_name: "Bob".to_string(),
                score: 0.91,
                rank: 1,
            }],
        }),
    }
}

#[test]
fn applies_speaker_profile_to_group_in_core() {
    let response = apply_speaker_profile_to_group_impl(ApplySpeakerProfileToGroupRequest {
        segments: vec![segment("anonymous-1")],
        group_id: "anonymous-1".to_string(),
        target_profile_id: "speaker-2".to_string(),
        speaker_profiles: vec![
            sample_profile("speaker-1", "Alice"),
            sample_profile("speaker-2", "Bob"),
        ],
        enabled_speaker_profile_ids: vec!["speaker-1".to_string()],
    })
    .unwrap();

    assert_eq!(
        response.segments[0].speaker,
        Some(SpeakerTag {
            id: "speaker-2".to_string(),
            label: "Bob".to_string(),
            kind: "identified".to_string(),
            score: None,
        })
    );
    let attribution = response.segments[0].speaker_attribution.as_ref().unwrap();
    assert_eq!(attribution.group_id, "anonymous-1");
    assert_eq!(attribution.anonymous_label, "Speaker 1");
    assert_eq!(attribution.state, "identified");
    assert_eq!(attribution.source, "manual");
    assert_eq!(attribution.candidates.len(), 1);
    assert_eq!(
        response.enabled_speaker_profile_ids,
        Some(vec!["speaker-1".to_string(), "speaker-2".to_string()])
    );
}

#[test]
fn speaker_correction_errors_preserve_specific_variants() {
    let missing_group = reset_speaker_group_to_anonymous_impl(SpeakerGroupRequest {
        segments: Vec::new(),
        group_id: "  ".to_string(),
    })
    .unwrap_err();
    assert_eq!(missing_group, SpeakerCorrectionError::MissingGroupId);

    let missing_profile = apply_speaker_profile_to_group_impl(ApplySpeakerProfileToGroupRequest {
        segments: Vec::new(),
        group_id: "anonymous-1".to_string(),
        target_profile_id: "missing".to_string(),
        speaker_profiles: Vec::new(),
        enabled_speaker_profile_ids: Vec::new(),
    })
    .unwrap_err();
    assert_eq!(
        missing_profile,
        SpeakerCorrectionError::ProfileNotFound {
            profile_id: "missing".to_string(),
        }
    );
}

#[test]
fn transcript_postprocess_error_preserves_rule_context() {
    let error = TranscriptPostprocessError::RuleCompilation {
        pattern: "speaker".to_string(),
        reason: "compiled regex exceeded size limit".to_string(),
    };

    assert_eq!(
        error,
        TranscriptPostprocessError::RuleCompilation {
            pattern: "speaker".to_string(),
            reason: "compiled regex exceeded size limit".to_string(),
        }
    );
}
