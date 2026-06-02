use crate::integrations::asr::TranscriptSegment;
use crate::integrations::speaker::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpeakerReviewStatus {
    Pending,
    Auto,
    Reviewed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SpeakerReviewRiskReason {
    Suggested,
    Anonymous,
    LowConfidence,
    MediumConfidence,
    AutoIdentified,
    Reviewed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SpeakerReviewFilter {
    Pending,
    Suggested,
    Anonymous,
    Identified,
    Reviewed,
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerReviewSegmentPreview {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub display_start: String,
    pub display_duration: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerReviewCandidate {
    pub profile_id: String,
    pub profile_name: String,
    pub score: f32,
    pub rank: usize,
    pub display_score: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerReviewGroup {
    pub group_id: String,
    pub display_label: String,
    pub anonymous_label: String,
    pub state: String,
    pub source: String,
    pub confidence: String,
    pub review_status: SpeakerReviewStatus,
    pub risk_reason: SpeakerReviewRiskReason,
    pub priority: usize,
    pub candidates: Vec<SpeakerReviewCandidate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<SpeakerTag>,
    pub segment_count: usize,
    pub duration_seconds: f64,
    pub display_duration: String,
    pub first_segment_id: String,
    pub first_start: f64,
    pub display_start: String,
    pub preview_segments: Vec<SpeakerReviewSegmentPreview>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerReviewCounts {
    pub total: usize,
    pub pending: usize,
    pub suggested: usize,
    pub anonymous: usize,
    pub identified: usize,
    pub reviewed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerReviewFilterOption {
    pub id: SpeakerReviewFilter,
    pub label_key: String,
    pub count_key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerReviewSnapshot {
    pub groups: Vec<SpeakerReviewGroup>,
    pub counts: SpeakerReviewCounts,
    pub visible_groups: Vec<SpeakerReviewGroup>,
    pub filter_options: Vec<SpeakerReviewFilterOption>,
}

#[tauri::command]
pub fn build_speaker_review_snapshot(
    segments: Vec<TranscriptSegment>,
    active_filter: SpeakerReviewFilter,
) -> SpeakerReviewSnapshot {
    let groups = build_speaker_review_groups(&segments);
    let counts = build_speaker_review_counts(&groups);
    let visible_groups = filter_speaker_review_groups(&groups, active_filter);

    SpeakerReviewSnapshot {
        groups,
        counts,
        visible_groups,
        filter_options: speaker_review_filter_options(),
    }
}

fn fallback_attribution(segment: &TranscriptSegment) -> Option<SpeakerAttribution> {
    let speaker = segment.speaker.as_ref()?;
    let existing = segment.speaker_attribution.as_ref();
    let group_id = existing
        .map(|attribution| attribution.group_id.as_str())
        .filter(|group_id| !group_id.is_empty())
        .unwrap_or({
            if speaker.id.is_empty() {
                segment.id.as_str()
            } else {
                speaker.id.as_str()
            }
        })
        .to_string();
    let anonymous_label = existing
        .map(|attribution| attribution.anonymous_label.as_str())
        .filter(|label| !label.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| {
            if speaker.kind == "anonymous" {
                speaker.label.clone()
            } else {
                "Speaker".to_string()
            }
        });

    Some(SpeakerAttribution {
        group_id,
        anonymous_label,
        state: existing
            .map(|attribution| attribution.state.clone())
            .unwrap_or_else(|| {
                if speaker.kind == "identified" {
                    "identified".to_string()
                } else {
                    "anonymous".to_string()
                }
            }),
        source: existing
            .map(|attribution| attribution.source.clone())
            .unwrap_or_else(|| "auto".to_string()),
        confidence: existing
            .map(|attribution| attribution.confidence.clone())
            .unwrap_or_else(|| {
                if speaker.kind == "identified" {
                    "high".to_string()
                } else {
                    "low".to_string()
                }
            }),
        candidates: existing
            .map(|attribution| attribution.candidates.clone())
            .unwrap_or_default(),
    })
}

fn build_speaker_review_groups(segments: &[TranscriptSegment]) -> Vec<SpeakerReviewGroup> {
    let mut groups: HashMap<String, SpeakerReviewGroup> = HashMap::new();

    for segment in segments {
        let attribution = segment
            .speaker_attribution
            .clone()
            .or_else(|| fallback_attribution(segment));
        let Some(attribution) = attribution else {
            continue;
        };

        if let Some(existing) = groups.get_mut(&attribution.group_id) {
            existing.segment_count += 1;
            existing.duration_seconds += segment_duration(segment);

            if segment.start < existing.first_start {
                existing.first_start = segment.start;
                existing.first_segment_id = segment.id.clone();
            }

            if review_state_weight(&attribution.state) < review_state_weight(&existing.state) {
                existing.state = attribution.state.clone();
                existing.confidence = attribution.confidence.clone();
            }

            if attribution.source == "manual" {
                existing.source = "manual".to_string();
            }

            if attribution.candidates.len() > existing.candidates.len() {
                existing.candidates = review_candidates(&attribution.candidates);
            }

            if existing.speaker.is_none() {
                if let Some(speaker) = segment.speaker.clone() {
                    existing.display_label = speaker.label.clone();
                    existing.speaker = Some(speaker);
                }
            }

            existing.preview_segments.push(segment_preview(segment));
            continue;
        }

        let review_status = resolve_review_status(
            &attribution.source,
            &attribution.state,
            &attribution.confidence,
        );
        let risk_reason = resolve_risk_reason(
            &attribution.source,
            &attribution.state,
            &attribution.confidence,
        );

        groups.insert(
            attribution.group_id.clone(),
            SpeakerReviewGroup {
                group_id: attribution.group_id.clone(),
                display_label: segment
                    .speaker
                    .as_ref()
                    .map(|speaker| speaker.label.clone())
                    .unwrap_or_else(|| attribution.anonymous_label.clone()),
                anonymous_label: attribution.anonymous_label.clone(),
                state: attribution.state.clone(),
                source: attribution.source.clone(),
                confidence: attribution.confidence.clone(),
                review_status,
                risk_reason,
                priority: get_priority(risk_reason),
                candidates: review_candidates(&attribution.candidates),
                speaker: segment.speaker.clone(),
                segment_count: 1,
                duration_seconds: segment_duration(segment),
                display_duration: format_duration(segment_duration(segment)),
                first_segment_id: segment.id.clone(),
                first_start: segment.start,
                display_start: format_timestamp(segment.start),
                preview_segments: vec![segment_preview(segment)],
            },
        );
    }

    let mut groups: Vec<SpeakerReviewGroup> = groups
        .into_values()
        .map(|mut group| {
            group.review_status =
                resolve_review_status(&group.source, &group.state, &group.confidence);
            group.risk_reason = resolve_risk_reason(&group.source, &group.state, &group.confidence);
            group.priority = get_priority(group.risk_reason);
            group.display_duration = format_duration(group.duration_seconds);
            group.display_start = format_timestamp(group.first_start);
            group
                .preview_segments
                .sort_by(|left, right| compare_f64(left.start, right.start));
            group.preview_segments.truncate(3);
            group
        })
        .collect();

    groups.sort_by(|left, right| {
        left.priority
            .cmp(&right.priority)
            .then_with(|| compare_f64(left.first_start, right.first_start))
    });
    groups
}

fn build_speaker_review_counts(groups: &[SpeakerReviewGroup]) -> SpeakerReviewCounts {
    let mut counts = SpeakerReviewCounts {
        total: 0,
        pending: 0,
        suggested: 0,
        anonymous: 0,
        identified: 0,
        reviewed: 0,
    };

    for group in groups {
        counts.total += 1;
        counts.pending += usize::from(group.review_status == SpeakerReviewStatus::Pending);
        counts.suggested += usize::from(group.state == "suggested");
        counts.anonymous += usize::from(group.state == "anonymous");
        counts.identified += usize::from(group.state == "identified");
        counts.reviewed += usize::from(group.review_status == SpeakerReviewStatus::Reviewed);
    }

    counts
}

fn filter_speaker_review_groups(
    groups: &[SpeakerReviewGroup],
    filter: SpeakerReviewFilter,
) -> Vec<SpeakerReviewGroup> {
    groups
        .iter()
        .filter(|group| match filter {
            SpeakerReviewFilter::Pending => group.review_status == SpeakerReviewStatus::Pending,
            SpeakerReviewFilter::Suggested => group.state == "suggested",
            SpeakerReviewFilter::Anonymous => group.state == "anonymous",
            SpeakerReviewFilter::Identified => group.state == "identified",
            SpeakerReviewFilter::Reviewed => group.review_status == SpeakerReviewStatus::Reviewed,
            SpeakerReviewFilter::All => true,
        })
        .cloned()
        .collect()
}

fn speaker_review_filter_options() -> Vec<SpeakerReviewFilterOption> {
    vec![
        filter_option(
            SpeakerReviewFilter::Pending,
            "editor.speaker_review_filter_pending",
            "pending",
        ),
        filter_option(
            SpeakerReviewFilter::Suggested,
            "editor.speaker_review_filter_suggested",
            "suggested",
        ),
        filter_option(
            SpeakerReviewFilter::Anonymous,
            "editor.speaker_review_filter_anonymous",
            "anonymous",
        ),
        filter_option(
            SpeakerReviewFilter::Identified,
            "editor.speaker_review_filter_identified",
            "identified",
        ),
        filter_option(
            SpeakerReviewFilter::Reviewed,
            "editor.speaker_review_filter_reviewed",
            "reviewed",
        ),
        filter_option(
            SpeakerReviewFilter::All,
            "editor.speaker_review_filter_all",
            "total",
        ),
    ]
}

fn filter_option(
    id: SpeakerReviewFilter,
    label_key: &str,
    count_key: &str,
) -> SpeakerReviewFilterOption {
    SpeakerReviewFilterOption {
        id,
        label_key: label_key.to_string(),
        count_key: count_key.to_string(),
    }
}

fn segment_duration(segment: &TranscriptSegment) -> f64 {
    (segment.end - segment.start).max(0.0)
}

fn segment_preview(segment: &TranscriptSegment) -> SpeakerReviewSegmentPreview {
    SpeakerReviewSegmentPreview {
        id: segment.id.clone(),
        start: segment.start,
        end: segment.end,
        display_start: format_timestamp(segment.start),
        display_duration: format_duration(segment_duration(segment)),
        text: segment.text.clone(),
    }
}

fn review_candidates(candidates: &[SpeakerCandidate]) -> Vec<SpeakerReviewCandidate> {
    candidates
        .iter()
        .map(|candidate| SpeakerReviewCandidate {
            profile_id: candidate.profile_id.clone(),
            profile_name: candidate.profile_name.clone(),
            score: candidate.score,
            rank: candidate.rank,
            display_score: format_score(candidate.score),
        })
        .collect()
}

fn format_duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds <= 0.0 {
        return "0s".to_string();
    }

    let rounded_seconds = seconds.round() as u64;
    if rounded_seconds >= 60 {
        let minutes = rounded_seconds / 60;
        let remaining = rounded_seconds % 60;
        return format!("{minutes}m {remaining:02}s");
    }

    format!("{rounded_seconds}s")
}

fn format_timestamp(seconds: f64) -> String {
    let safe_seconds = seconds.max(0.0).floor() as u64;
    let minutes = safe_seconds / 60;
    let remaining = safe_seconds % 60;
    format!("{minutes}:{remaining:02}")
}

fn format_score(score: f32) -> String {
    format!("{score:.2}")
}

fn review_state_weight(state: &str) -> usize {
    match state {
        "suggested" => 0,
        "anonymous" => 1,
        "identified" => 2,
        _ => 2,
    }
}

fn resolve_review_status(source: &str, state: &str, confidence: &str) -> SpeakerReviewStatus {
    if source == "manual" {
        return SpeakerReviewStatus::Reviewed;
    }

    if state == "suggested" || state == "anonymous" || confidence != "high" {
        return SpeakerReviewStatus::Pending;
    }

    SpeakerReviewStatus::Auto
}

fn resolve_risk_reason(source: &str, state: &str, confidence: &str) -> SpeakerReviewRiskReason {
    if source == "manual" {
        return SpeakerReviewRiskReason::Reviewed;
    }

    if state == "suggested" {
        return SpeakerReviewRiskReason::Suggested;
    }

    if state == "anonymous" {
        return SpeakerReviewRiskReason::Anonymous;
    }

    if confidence == "low" {
        return SpeakerReviewRiskReason::LowConfidence;
    }

    if confidence == "medium" {
        return SpeakerReviewRiskReason::MediumConfidence;
    }

    SpeakerReviewRiskReason::AutoIdentified
}

fn get_priority(reason: SpeakerReviewRiskReason) -> usize {
    match reason {
        SpeakerReviewRiskReason::Suggested => 0,
        SpeakerReviewRiskReason::Anonymous => 1,
        SpeakerReviewRiskReason::LowConfidence => 2,
        SpeakerReviewRiskReason::MediumConfidence => 3,
        SpeakerReviewRiskReason::AutoIdentified => 4,
        SpeakerReviewRiskReason::Reviewed => 5,
    }
}

fn compare_f64(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integrations::asr::TranscriptSegment;
    use crate::integrations::speaker::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};

    fn segment(
        id: &str,
        group_id: &str,
        label: &str,
        state: &str,
        source: &str,
        confidence: &str,
        start: f64,
    ) -> TranscriptSegment {
        TranscriptSegment {
            id: id.to_string(),
            text: label.to_string(),
            start,
            end: start + 4.0,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: Some(SpeakerTag {
                id: if state == "identified" {
                    format!("profile-{group_id}")
                } else {
                    group_id.to_string()
                },
                label: label.to_string(),
                kind: if state == "identified" {
                    "identified".to_string()
                } else {
                    "anonymous".to_string()
                },
                score: None,
            }),
            speaker_attribution: Some(SpeakerAttribution {
                group_id: group_id.to_string(),
                anonymous_label: if label.starts_with("Speaker") {
                    label.to_string()
                } else {
                    format!("Speaker {group_id}")
                },
                state: state.to_string(),
                source: source.to_string(),
                confidence: confidence.to_string(),
                candidates: if state == "suggested" {
                    vec![SpeakerCandidate {
                        profile_id: "alice".to_string(),
                        profile_name: "Alice".to_string(),
                        score: 0.82,
                        rank: 1,
                    }]
                } else {
                    Vec::new()
                },
            }),
        }
    }

    #[test]
    fn sorts_groups_by_review_priority_then_first_start() {
        let snapshot = build_speaker_review_snapshot(
            vec![
                segment(
                    "seg-identified",
                    "anonymous-3",
                    "Alice",
                    "identified",
                    "auto",
                    "high",
                    30.0,
                ),
                segment(
                    "seg-anonymous",
                    "anonymous-2",
                    "Speaker 2",
                    "anonymous",
                    "auto",
                    "low",
                    10.0,
                ),
                segment(
                    "seg-suggested",
                    "anonymous-1",
                    "Speaker 1",
                    "suggested",
                    "auto",
                    "medium",
                    20.0,
                ),
            ],
            SpeakerReviewFilter::All,
        );

        let group_ids: Vec<&str> = snapshot
            .groups
            .iter()
            .map(|group| group.group_id.as_str())
            .collect();
        assert_eq!(group_ids, vec!["anonymous-1", "anonymous-2", "anonymous-3"]);
    }

    #[test]
    fn builds_counts_and_filters_visible_groups() {
        let snapshot = build_speaker_review_snapshot(
            vec![
                segment(
                    "seg-suggested",
                    "anonymous-1",
                    "Speaker 1",
                    "suggested",
                    "auto",
                    "medium",
                    20.0,
                ),
                segment(
                    "seg-anonymous",
                    "anonymous-2",
                    "Speaker 2",
                    "anonymous",
                    "auto",
                    "low",
                    10.0,
                ),
                segment(
                    "seg-auto",
                    "anonymous-3",
                    "Alice",
                    "identified",
                    "auto",
                    "high",
                    30.0,
                ),
                segment(
                    "seg-manual",
                    "anonymous-4",
                    "Bob",
                    "identified",
                    "manual",
                    "high",
                    40.0,
                ),
            ],
            SpeakerReviewFilter::Pending,
        );

        assert_eq!(snapshot.counts.total, 4);
        assert_eq!(snapshot.counts.pending, 2);
        assert_eq!(snapshot.counts.suggested, 1);
        assert_eq!(snapshot.counts.anonymous, 1);
        assert_eq!(snapshot.counts.identified, 2);
        assert_eq!(snapshot.counts.reviewed, 1);

        let visible_group_ids: Vec<&str> = snapshot
            .visible_groups
            .iter()
            .map(|group| group.group_id.as_str())
            .collect();
        assert_eq!(visible_group_ids, vec!["anonymous-1", "anonymous-2"]);
    }

    #[test]
    fn falls_back_to_segment_speaker_when_attribution_is_missing() {
        let mut fallback_segment = segment(
            "seg-fallback",
            "speaker-1",
            "Alice",
            "identified",
            "auto",
            "high",
            5.0,
        );
        fallback_segment.speaker_attribution = None;

        let snapshot =
            build_speaker_review_snapshot(vec![fallback_segment], SpeakerReviewFilter::All);

        assert_eq!(snapshot.groups.len(), 1);
        let group = &snapshot.groups[0];
        assert_eq!(group.group_id, "profile-speaker-1");
        assert_eq!(group.display_label, "Alice");
        assert_eq!(group.anonymous_label, "Speaker");
        assert_eq!(group.state, "identified");
        assert_eq!(group.review_status, SpeakerReviewStatus::Auto);
        assert_eq!(group.risk_reason, SpeakerReviewRiskReason::AutoIdentified);
    }

    #[test]
    fn truncates_preview_segments_after_sorting_by_start() {
        let snapshot = build_speaker_review_snapshot(
            vec![
                segment(
                    "seg-3",
                    "anonymous-1",
                    "Third line",
                    "anonymous",
                    "auto",
                    "low",
                    12.0,
                ),
                segment(
                    "seg-1",
                    "anonymous-1",
                    "First line",
                    "anonymous",
                    "auto",
                    "low",
                    2.0,
                ),
                segment(
                    "seg-4",
                    "anonymous-1",
                    "Fourth line",
                    "anonymous",
                    "auto",
                    "low",
                    20.0,
                ),
                segment(
                    "seg-2",
                    "anonymous-1",
                    "Second line",
                    "anonymous",
                    "auto",
                    "low",
                    8.0,
                ),
            ],
            SpeakerReviewFilter::All,
        );

        let preview_ids: Vec<&str> = snapshot.groups[0]
            .preview_segments
            .iter()
            .map(|preview| preview.id.as_str())
            .collect();
        assert_eq!(preview_ids, vec!["seg-1", "seg-2", "seg-3"]);
    }

    #[test]
    fn produces_view_ready_display_values_for_groups_previews_and_candidates() {
        let snapshot = build_speaker_review_snapshot(
            vec![
                segment(
                    "seg-1",
                    "anonymous-1",
                    "Speaker 1",
                    "suggested",
                    "auto",
                    "medium",
                    61.2,
                ),
                segment(
                    "seg-2",
                    "anonymous-1",
                    "Speaker 1",
                    "suggested",
                    "auto",
                    "medium",
                    70.0,
                ),
            ],
            SpeakerReviewFilter::All,
        );

        let group = &snapshot.groups[0];
        assert_eq!(group.display_start, "1:01");
        assert_eq!(group.display_duration, "8s");
        assert_eq!(group.preview_segments[0].display_start, "1:01");
        assert_eq!(group.preview_segments[0].display_duration, "4s");
        assert_eq!(group.candidates[0].display_score, "0.82");
    }
}
