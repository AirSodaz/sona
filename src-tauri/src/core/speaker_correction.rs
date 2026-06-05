use crate::integrations::asr::TranscriptSegment;
use crate::integrations::speaker::{SpeakerAttribution, SpeakerProfile, SpeakerTag};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const EMPTY_GROUP_ERROR: &str = "Speaker correction requires a source speaker id.";

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ApplySpeakerProfileToGroupRequest {
    pub segments: Vec<TranscriptSegment>,
    pub group_id: String,
    pub target_profile_id: String,
    pub speaker_profiles: Vec<SpeakerProfile>,
    pub enabled_speaker_profile_ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerGroupRequest {
    pub segments: Vec<TranscriptSegment>,
    pub group_id: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerCorrectionResponse {
    pub segments: Vec<TranscriptSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled_speaker_profile_ids: Option<Vec<String>>,
}

fn resolve_segment_group_id(segment: &TranscriptSegment) -> &str {
    segment
        .speaker_attribution
        .as_ref()
        .map(|attribution| attribution.group_id.as_str())
        .filter(|group_id| !group_id.is_empty())
        .or_else(|| segment.speaker.as_ref().map(|speaker| speaker.id.as_str()))
        .unwrap_or("")
}

fn anonymous_label(segment: &TranscriptSegment) -> String {
    segment
        .speaker_attribution
        .as_ref()
        .map(|attribution| attribution.anonymous_label.clone())
        .filter(|label| !label.is_empty())
        .or_else(|| {
            segment
                .speaker
                .as_ref()
                .map(|speaker| speaker.label.clone())
        })
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "Speaker".to_string())
}

fn confirmed_anonymous_label(segment: &TranscriptSegment) -> String {
    segment
        .speaker_attribution
        .as_ref()
        .map(|attribution| attribution.anonymous_label.clone())
        .filter(|label| !label.is_empty())
        .or_else(|| {
            segment
                .speaker
                .as_ref()
                .filter(|speaker| speaker.kind == "anonymous")
                .map(|speaker| speaker.label.clone())
        })
        .filter(|label| !label.is_empty())
        .unwrap_or_else(|| "Speaker".to_string())
}

fn candidates(segment: &TranscriptSegment) -> Vec<crate::integrations::speaker::SpeakerCandidate> {
    segment
        .speaker_attribution
        .as_ref()
        .map(|attribution| attribution.candidates.clone())
        .unwrap_or_default()
}

fn ensure_group_id(group_id: &str) -> Result<(), String> {
    if group_id.trim().is_empty() {
        Err(EMPTY_GROUP_ERROR.to_string())
    } else {
        Ok(())
    }
}

fn ordered_enabled_speaker_profile_ids(
    profiles: &[SpeakerProfile],
    existing_ids: &[String],
    next_id: &str,
) -> Vec<String> {
    let enabled_ids: HashSet<&str> = existing_ids
        .iter()
        .map(String::as_str)
        .chain(std::iter::once(next_id))
        .collect();

    profiles
        .iter()
        .filter(|profile| enabled_ids.contains(profile.id.as_str()))
        .map(|profile| profile.id.clone())
        .collect()
}

pub fn apply_speaker_profile_to_group_impl(
    request: ApplySpeakerProfileToGroupRequest,
) -> Result<SpeakerCorrectionResponse, String> {
    ensure_group_id(&request.group_id)?;

    let target_profile = request
        .speaker_profiles
        .iter()
        .find(|profile| profile.id == request.target_profile_id)
        .ok_or_else(|| format!("Speaker profile not found: {}", request.target_profile_id))?;

    let next_speaker = SpeakerTag {
        id: target_profile.id.clone(),
        label: target_profile.name.clone(),
        kind: "identified".to_string(),
        score: None,
    };

    let segments = request
        .segments
        .into_iter()
        .map(|mut segment| {
            if resolve_segment_group_id(&segment) == request.group_id {
                let anonymous_label = anonymous_label(&segment);
                segment.speaker = Some(next_speaker.clone());
                segment.speaker_attribution = Some(SpeakerAttribution {
                    group_id: request.group_id.clone(),
                    anonymous_label,
                    state: "identified".to_string(),
                    source: "manual".to_string(),
                    confidence: "high".to_string(),
                    candidates: candidates(&segment),
                });
            }
            segment
        })
        .collect();

    let enabled_speaker_profile_ids = ordered_enabled_speaker_profile_ids(
        &request.speaker_profiles,
        &request.enabled_speaker_profile_ids,
        &target_profile.id,
    );

    Ok(SpeakerCorrectionResponse {
        segments,
        enabled_speaker_profile_ids: Some(enabled_speaker_profile_ids),
    })
}

pub fn reset_speaker_group_to_anonymous_impl(
    request: SpeakerGroupRequest,
) -> Result<SpeakerCorrectionResponse, String> {
    ensure_group_id(&request.group_id)?;

    let segments = request
        .segments
        .into_iter()
        .map(|mut segment| {
            if resolve_segment_group_id(&segment) == request.group_id {
                let anonymous_label = anonymous_label(&segment);
                segment.speaker = Some(SpeakerTag {
                    id: request.group_id.clone(),
                    label: anonymous_label.clone(),
                    kind: "anonymous".to_string(),
                    score: None,
                });
                segment.speaker_attribution = Some(SpeakerAttribution {
                    group_id: request.group_id.clone(),
                    anonymous_label,
                    state: "anonymous".to_string(),
                    source: "manual".to_string(),
                    confidence: "low".to_string(),
                    candidates: candidates(&segment),
                });
            }
            segment
        })
        .collect();

    Ok(SpeakerCorrectionResponse {
        segments,
        enabled_speaker_profile_ids: None,
    })
}

pub fn confirm_speaker_group_review_impl(
    request: SpeakerGroupRequest,
) -> Result<SpeakerCorrectionResponse, String> {
    ensure_group_id(&request.group_id)?;

    let segments = request
        .segments
        .into_iter()
        .map(|mut segment| {
            if resolve_segment_group_id(&segment) == request.group_id {
                let is_identified = segment
                    .speaker
                    .as_ref()
                    .map(|speaker| speaker.kind == "identified")
                    .unwrap_or(false);
                segment.speaker_attribution = Some(SpeakerAttribution {
                    group_id: request.group_id.clone(),
                    anonymous_label: confirmed_anonymous_label(&segment),
                    state: if is_identified {
                        "identified"
                    } else {
                        "anonymous"
                    }
                    .to_string(),
                    source: "manual".to_string(),
                    confidence: if is_identified { "high" } else { "low" }.to_string(),
                    candidates: candidates(&segment),
                });
            }
            segment
        })
        .collect();

    Ok(SpeakerCorrectionResponse {
        segments,
        enabled_speaker_profile_ids: None,
    })
}

pub async fn apply_speaker_profile_to_group(
    request: ApplySpeakerProfileToGroupRequest,
) -> Result<SpeakerCorrectionResponse, String> {
    apply_speaker_profile_to_group_impl(request)
}

pub async fn reset_speaker_group_to_anonymous(
    request: SpeakerGroupRequest,
) -> Result<SpeakerCorrectionResponse, String> {
    reset_speaker_group_to_anonymous_impl(request)
}

pub async fn confirm_speaker_group_review(
    request: SpeakerGroupRequest,
) -> Result<SpeakerCorrectionResponse, String> {
    confirm_speaker_group_review_impl(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(id: &str, name: &str) -> SpeakerProfile {
        SpeakerProfile {
            id: id.to_string(),
            name: name.to_string(),
            enabled: true,
            samples: vec![],
        }
    }

    fn tag(id: &str, label: &str, kind: &str) -> SpeakerTag {
        SpeakerTag {
            id: id.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            score: None,
        }
    }

    fn attribution(group_id: &str, anonymous_label: &str, state: &str) -> SpeakerAttribution {
        SpeakerAttribution {
            group_id: group_id.to_string(),
            anonymous_label: anonymous_label.to_string(),
            state: state.to_string(),
            source: "auto".to_string(),
            confidence: "medium".to_string(),
            candidates: vec![crate::integrations::speaker::SpeakerCandidate {
                profile_id: "speaker-2".to_string(),
                profile_name: "Bob".to_string(),
                score: 0.88,
                rank: 1,
            }],
        }
    }

    fn segment(
        id: &str,
        speaker: Option<SpeakerTag>,
        speaker_attribution: Option<SpeakerAttribution>,
    ) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: id.to_string(),
            start: 0.0,
            end: 1.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker,
            speaker_attribution,
        }
    }

    #[test]
    fn assigns_profile_to_matching_group_and_preserves_anonymous_origin() {
        let response = apply_speaker_profile_to_group_impl(ApplySpeakerProfileToGroupRequest {
            segments: vec![
                segment(
                    "seg-a",
                    Some(tag("anonymous-1", "Speaker 1", "anonymous")),
                    Some(attribution("anonymous-1", "Speaker 1", "anonymous")),
                ),
                segment(
                    "seg-b",
                    Some(tag("speaker-2", "Bob", "identified")),
                    Some(attribution("anonymous-2", "Speaker 2", "identified")),
                ),
            ],
            group_id: "anonymous-1".to_string(),
            target_profile_id: "speaker-2".to_string(),
            speaker_profiles: vec![profile("speaker-1", "Alice"), profile("speaker-2", "Bob")],
            enabled_speaker_profile_ids: vec!["speaker-1".to_string()],
        })
        .unwrap();

        assert_eq!(
            response.segments[0].speaker,
            Some(tag("speaker-2", "Bob", "identified"))
        );
        assert_eq!(
            response.segments[0].speaker_attribution,
            Some(SpeakerAttribution {
                group_id: "anonymous-1".to_string(),
                anonymous_label: "Speaker 1".to_string(),
                state: "identified".to_string(),
                source: "manual".to_string(),
                confidence: "high".to_string(),
                candidates: attribution("anonymous-1", "Speaker 1", "anonymous").candidates,
            })
        );
        assert_eq!(
            response.segments[1].speaker,
            Some(tag("speaker-2", "Bob", "identified"))
        );
    }

    #[test]
    fn resolves_group_from_visible_speaker_when_attribution_is_missing() {
        let response = reset_speaker_group_to_anonymous_impl(SpeakerGroupRequest {
            segments: vec![segment(
                "seg-a",
                Some(tag("anonymous-1", "Speaker 1", "anonymous")),
                None,
            )],
            group_id: "anonymous-1".to_string(),
        })
        .unwrap();

        assert_eq!(
            response.segments[0]
                .speaker_attribution
                .as_ref()
                .map(|item| item.group_id.as_str()),
            Some("anonymous-1")
        );
    }

    #[test]
    fn resets_group_to_anonymous_and_preserves_candidates() {
        let response = reset_speaker_group_to_anonymous_impl(SpeakerGroupRequest {
            segments: vec![segment(
                "seg-a",
                Some(tag("speaker-2", "Bob", "identified")),
                Some(attribution("anonymous-1", "Speaker 1", "identified")),
            )],
            group_id: "anonymous-1".to_string(),
        })
        .unwrap();

        assert_eq!(
            response.segments[0].speaker,
            Some(tag("anonymous-1", "Speaker 1", "anonymous"))
        );
        let next_attribution = response.segments[0].speaker_attribution.as_ref().unwrap();
        assert_eq!(next_attribution.state, "anonymous");
        assert_eq!(next_attribution.source, "manual");
        assert_eq!(next_attribution.confidence, "low");
        assert_eq!(next_attribution.candidates.len(), 1);
    }

    #[test]
    fn confirms_review_without_changing_visible_speaker() {
        let response = confirm_speaker_group_review_impl(SpeakerGroupRequest {
            segments: vec![segment(
                "seg-a",
                Some(tag("speaker-2", "Bob", "identified")),
                Some(attribution("anonymous-1", "Speaker 1", "suggested")),
            )],
            group_id: "anonymous-1".to_string(),
        })
        .unwrap();

        assert_eq!(
            response.segments[0].speaker,
            Some(tag("speaker-2", "Bob", "identified"))
        );
        let next_attribution = response.segments[0].speaker_attribution.as_ref().unwrap();
        assert_eq!(next_attribution.state, "identified");
        assert_eq!(next_attribution.source, "manual");
        assert_eq!(next_attribution.confidence, "high");
    }

    #[test]
    fn keeps_enabled_profile_order_from_normalized_profiles() {
        let response = apply_speaker_profile_to_group_impl(ApplySpeakerProfileToGroupRequest {
            segments: vec![],
            group_id: "anonymous-1".to_string(),
            target_profile_id: "speaker-2".to_string(),
            speaker_profiles: vec![
                profile("speaker-2", "Bob"),
                profile("speaker-1", "Alice"),
                profile("speaker-3", "Carol"),
            ],
            enabled_speaker_profile_ids: vec!["speaker-1".to_string()],
        })
        .unwrap();

        assert_eq!(
            response.enabled_speaker_profile_ids,
            Some(vec!["speaker-2".to_string(), "speaker-1".to_string()])
        );
    }

    #[test]
    fn rejects_empty_group_id_and_missing_profile() {
        let empty_group_error = reset_speaker_group_to_anonymous_impl(SpeakerGroupRequest {
            segments: vec![],
            group_id: "  ".to_string(),
        })
        .unwrap_err();
        assert_eq!(empty_group_error, EMPTY_GROUP_ERROR);

        let missing_profile_error =
            apply_speaker_profile_to_group_impl(ApplySpeakerProfileToGroupRequest {
                segments: vec![],
                group_id: "anonymous-1".to_string(),
                target_profile_id: "missing".to_string(),
                speaker_profiles: vec![],
                enabled_speaker_profile_ids: vec![],
            })
            .unwrap_err();
        assert_eq!(missing_profile_error, "Speaker profile not found: missing");
    }
}
