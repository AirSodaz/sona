use sona_core::speaker::{SpeakerProfile, SpeakerProfileSample};
use sona_core::speaker_correction::{
    ApplySpeakerProfileToGroupRequest, apply_speaker_profile_to_group_impl,
};
use sona_core::transcript::{SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment};

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
