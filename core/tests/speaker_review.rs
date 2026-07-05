use sona_core::speaker_review::{
    SpeakerReviewFilter, SpeakerReviewRiskReason, build_speaker_review_snapshot,
};
use sona_core::transcript::{SpeakerAttribution, SpeakerTag, TranscriptSegment};

fn segment(id: &str, group_id: &str, state: &str, confidence: &str) -> TranscriptSegment {
    TranscriptSegment {
        id: id.to_string(),
        text: id.to_string(),
        start: 0.0,
        end: 2.0,
        is_final: true,
        timing: None,
        tokens: None,
        timestamps: None,
        durations: None,
        translation: None,
        speaker: Some(SpeakerTag {
            id: group_id.to_string(),
            label: "Speaker".to_string(),
            kind: if state == "identified" {
                "identified".to_string()
            } else {
                "anonymous".to_string()
            },
            score: None,
        }),
        speaker_attribution: Some(SpeakerAttribution {
            group_id: group_id.to_string(),
            anonymous_label: "Speaker".to_string(),
            state: state.to_string(),
            source: "auto".to_string(),
            confidence: confidence.to_string(),
            candidates: Vec::new(),
        }),
    }
}

#[test]
fn filters_pending_groups_in_core_without_adapter_dependencies() {
    let snapshot = build_speaker_review_snapshot(
        vec![
            segment("pending", "group-a", "anonymous", "low"),
            segment("ready", "group-b", "identified", "high"),
        ],
        SpeakerReviewFilter::Pending,
    );

    assert_eq!(snapshot.counts.total, 2);
    assert_eq!(snapshot.visible_groups.len(), 1);
    assert_eq!(snapshot.visible_groups[0].group_id, "group-a");
    assert_eq!(
        snapshot.visible_groups[0].risk_reason,
        SpeakerReviewRiskReason::Anonymous
    );
}
