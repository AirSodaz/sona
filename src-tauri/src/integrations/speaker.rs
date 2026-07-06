use crate::integrations::asr::{
    TranscriptSegment, TranscriptTiming, TranscriptTimingLevel, ensure_transcript_segment_timing,
};

pub use crate::core::speaker::{SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample};
pub use crate::core::transcript::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};

use crate::core::paths::{PathKind, PathProvider};
use crate::core::text_alignment::{AlignedTextUnit, align_text_units_to_tokens};
use log::{debug, info};
use serde::{Deserialize, Serialize};
use sona_local_asr::speaker::SpeakerDiarizationSegment;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

const SPEAKER_PROCESSING_LOG_TARGET: &str = "speaker_processing";
const SAMPLE_RATE: i32 = 16_000;
const IDENTIFICATION_MIN_DURATION_SECONDS: f32 = 1.5;
const IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER: usize = 3;
const CANDIDATE_DISPLAY_THRESHOLD: f32 = 0.6;
const AUTO_IDENTIFICATION_THRESHOLD: f32 = 0.72;
const AUTO_IDENTIFICATION_MIN_VOTES: usize = 2;
const AUTO_IDENTIFICATION_MIN_MARGIN: f32 = 0.08;
const PROFILE_SAMPLE_MIN_DURATION_SECONDS: f32 = 4.0;
const PROFILE_LIMITED_MIN_TOTAL_DURATION_SECONDS: f32 = 8.0;
const PROFILE_READY_MIN_TOTAL_DURATION_SECONDS: f32 = 20.0;
const PROFILE_READY_MIN_SAMPLE_COUNT: usize = 2;

#[derive(Debug, Clone)]
struct SpeakerSpan {
    start: f32,
    end: f32,
    raw_speaker: i32,
}

#[derive(Debug, Clone)]
struct ClusterInfo {
    raw_speaker: i32,
    spans: Vec<SpeakerSpan>,
    anonymous_tag: SpeakerTag,
}

#[derive(Debug, Clone)]
struct ClusterCandidate {
    profile_id: String,
    profile_name: String,
    votes: usize,
    average_score: f32,
}

#[derive(Debug, Clone)]
struct SplitGroup {
    assignment: ResolvedSpeakerAssignment,
    text: String,
    token_start: usize,
    token_end_exclusive: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SpeakerProfileReadinessState {
    NotReady,
    Limited,
    Ready,
}

#[derive(Debug, Clone)]
struct ResolvedSpeakerAssignment {
    raw_speaker: i32,
    speaker: Option<SpeakerTag>,
    attribution: SpeakerAttribution,
    average_score: Option<f32>,
    votes: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SpeakerAssignmentSummary {
    identified: usize,
    suggested: usize,
    anonymous: usize,
    candidate_clusters: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct SpeakerProfileIndexSummary {
    enabled_profiles: usize,
    ready_profiles: usize,
    limited_profiles: usize,
    skipped_profiles: usize,
    usable_sample_embeddings: usize,
}

pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<TranscriptSegment>,
    speaker_processing: Option<SpeakerProcessingConfig>,
) -> Result<Vec<TranscriptSegment>, String> {
    if segments.is_empty() {
        return Ok(segments);
    }

    let samples = sona_local_asr::audio::extract_and_resample_audio(
        std::path::Path::new(&file_path),
        SAMPLE_RATE as u32,
    )
    .await?;
    annotate_segments_with_speakers(&samples, &segments, speaker_processing.as_ref())
}

pub async fn import_speaker_profile_sample(
    provider: &dyn PathProvider,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let samples = sona_local_asr::audio::extract_and_resample_audio(
        std::path::Path::new(&source_path),
        SAMPLE_RATE as u32,
    )
    .await?;
    let duration_seconds = samples.len() as f32 / SAMPLE_RATE as f32;
    let sample_id = uuid::Uuid::new_v4().to_string();
    let sample_name = source_name
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .map(|value| value.trim().to_string())
        .or_else(|| {
            Path::new(&source_path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string())
        })
        .unwrap_or_else(|| "Sample".to_string());

    let app_data_dir = provider.resolve_path(PathKind::AppLocalData)?;
    let profile_dir = app_data_dir.join("speaker-profiles").join(&profile_id);
    std::fs::create_dir_all(&profile_dir).map_err(|e| e.to_string())?;

    let output_path = profile_dir.join(format!("{sample_id}.wav"));
    sona_local_asr::audio::save_wav_file(&samples, SAMPLE_RATE as u32, &output_path)
        .map_err(|e| e.to_string())?;

    Ok(SpeakerProfileSample {
        id: sample_id,
        file_path: output_path.to_string_lossy().into_owned(),
        source_name: sample_name,
        duration_seconds,
    })
}

pub fn annotate_segments_with_speakers(
    samples: &[f32],
    segments: &[TranscriptSegment],
    speaker_processing: Option<&SpeakerProcessingConfig>,
) -> Result<Vec<TranscriptSegment>, String> {
    let total_started = Instant::now();
    let input_segment_count = segments.len();
    let audio_duration_ms = samples_to_duration_ms(samples.len());

    if segments.is_empty() {
        log_speaker_processing_skip("no_segments", "setup");
        return Ok(Vec::new());
    }

    let Some(config) = speaker_processing else {
        log_speaker_processing_skip("disabled", "setup");
        return Ok(segments.to_vec());
    };

    let segmentation_model = resolve_model_path(config.speaker_segmentation_model_path.as_deref())?;
    let embedding_model = resolve_model_path(config.speaker_embedding_model_path.as_deref())?;

    let diarization_started = Instant::now();
    let diarization_segments = run_diarization(samples, &segmentation_model, &embedding_model)?;
    let diarization_ms = elapsed_ms(diarization_started);
    let clusters = build_cluster_infos(&diarization_segments);
    info!(
        target: SPEAKER_PROCESSING_LOG_TARGET,
        "event=speaker_diarization_complete audio_duration_ms={:.1} input_segment_count={} diarization_segment_count={} cluster_count={} diarization_ms={:.1}",
        audio_duration_ms,
        input_segment_count,
        diarization_segments.len(),
        clusters.len(),
        diarization_ms,
    );

    if diarization_segments.is_empty() {
        log_speaker_processing_skip("no_diarization_segments", "diarization");
        log_speaker_processing_complete(
            total_started,
            input_segment_count,
            segments.len(),
            &SpeakerAssignmentSummary::default(),
        );
        return Ok(segments.to_vec());
    }

    log_cluster_debug_summary(&clusters);
    let speaker_assignments =
        build_cluster_speaker_assignments(samples, &clusters, config, &embedding_model)?;
    let annotated_segments =
        apply_speaker_tags_to_segments(segments, &clusters, &speaker_assignments);
    let assignment_summary = summarize_speaker_assignments(&speaker_assignments);
    log_speaker_processing_complete(
        total_started,
        input_segment_count,
        annotated_segments.len(),
        &assignment_summary,
    );
    Ok(annotated_segments)
}

fn resolve_model_path(input: Option<&str>) -> Result<PathBuf, String> {
    let raw = input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "Speaker processing models are not fully configured".to_string())?;
    let path = Path::new(raw);
    if !path.exists() {
        return Err(format!("Speaker model path does not exist: {raw}"));
    }

    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let mut onnx_files = std::fs::read_dir(path)
        .map_err(|e| e.to_string())?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let entry_path = entry.path();
            let extension = entry_path.extension()?.to_str()?;
            if extension.eq_ignore_ascii_case("onnx") {
                Some(entry_path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    onnx_files.sort();
    onnx_files
        .into_iter()
        .next()
        .ok_or_else(|| format!("No .onnx file found in {}", path.display()))
}

fn elapsed_ms(started: Instant) -> f64 {
    started.elapsed().as_secs_f64() * 1000.0
}

fn samples_to_duration_ms(sample_count: usize) -> f64 {
    sample_count as f64 / SAMPLE_RATE as f64 * 1000.0
}

fn count_added_speaker_segments(input_segment_count: usize, output_segment_count: usize) -> usize {
    output_segment_count.saturating_sub(input_segment_count)
}

fn summarize_speaker_assignments(
    assignments: &HashMap<i32, ResolvedSpeakerAssignment>,
) -> SpeakerAssignmentSummary {
    assignments.values().fold(
        SpeakerAssignmentSummary::default(),
        |mut summary, assignment| {
            match assignment.attribution.state.as_str() {
                "identified" => summary.identified += 1,
                "suggested" => summary.suggested += 1,
                _ => summary.anonymous += 1,
            }

            if !assignment.attribution.candidates.is_empty() {
                summary.candidate_clusters += 1;
            }

            summary
        },
    )
}

fn log_speaker_processing_skip(reason: &str, stage: &str) {
    info!(
        target: SPEAKER_PROCESSING_LOG_TARGET,
        "event=speaker_processing_skip reason={} stage={}",
        reason,
        stage,
    );
}

fn log_speaker_profile_index_complete(summary: &SpeakerProfileIndexSummary, index_ms: f64) {
    info!(
        target: SPEAKER_PROCESSING_LOG_TARGET,
        "event=speaker_profile_index_complete enabled_profile_count={} ready_profile_count={} limited_profile_count={} skipped_profile_count={} usable_sample_embedding_count={} index_ms={:.1}",
        summary.enabled_profiles,
        summary.ready_profiles,
        summary.limited_profiles,
        summary.skipped_profiles,
        summary.usable_sample_embeddings,
        index_ms,
    );
}

fn log_speaker_matching_complete(summary: &SpeakerAssignmentSummary, matching_ms: f64) {
    info!(
        target: SPEAKER_PROCESSING_LOG_TARGET,
        "event=speaker_matching_complete candidate_cluster_count={} identified_cluster_count={} suggested_cluster_count={} anonymous_cluster_count={} matching_ms={:.1}",
        summary.candidate_clusters,
        summary.identified,
        summary.suggested,
        summary.anonymous,
        matching_ms,
    );
}

fn log_speaker_processing_complete(
    started: Instant,
    input_segment_count: usize,
    output_segment_count: usize,
    summary: &SpeakerAssignmentSummary,
) {
    info!(
        target: SPEAKER_PROCESSING_LOG_TARGET,
        "event=speaker_processing_complete total_ms={:.1} input_segment_count={} output_segment_count={} added_segment_count={} identified_cluster_count={} suggested_cluster_count={} anonymous_cluster_count={} candidate_cluster_count={}",
        elapsed_ms(started),
        input_segment_count,
        output_segment_count,
        count_added_speaker_segments(input_segment_count, output_segment_count),
        summary.identified,
        summary.suggested,
        summary.anonymous,
        summary.candidate_clusters,
    );
}

fn log_cluster_debug_summary(clusters: &[ClusterInfo]) {
    for (index, cluster) in clusters.iter().enumerate() {
        let span_count = cluster.spans.len();
        let total_duration_ms = cluster
            .spans
            .iter()
            .map(|span| (span.end - span.start).max(0.0) as f64 * 1000.0)
            .sum::<f64>();
        let first_start_ms = cluster
            .spans
            .iter()
            .map(|span| span.start)
            .min_by(|left, right| left.partial_cmp(right).unwrap_or(Ordering::Equal))
            .unwrap_or_default() as f64
            * 1000.0;

        debug!(
            target: SPEAKER_PROCESSING_LOG_TARGET,
            "event=speaker_cluster_summary cluster_index={} raw_speaker={} span_count={} total_duration_ms={:.1} first_start_ms={:.1}",
            index,
            cluster.raw_speaker,
            span_count,
            total_duration_ms,
            first_start_ms,
        );
    }
}

fn run_diarization(
    samples: &[f32],
    segmentation_model: &Path,
    embedding_model: &Path,
) -> Result<Vec<SpeakerDiarizationSegment>, String> {
    sona_local_asr::speaker::run_speaker_diarization(samples, segmentation_model, embedding_model)
}

fn build_cluster_infos(diarization_segments: &[SpeakerDiarizationSegment]) -> Vec<ClusterInfo> {
    let mut spans_by_speaker: BTreeMap<i32, Vec<SpeakerSpan>> = BTreeMap::new();

    for segment in diarization_segments {
        spans_by_speaker
            .entry(segment.speaker)
            .or_default()
            .push(SpeakerSpan {
                start: segment.start,
                end: segment.end,
                raw_speaker: segment.speaker,
            });
    }

    let mut ordered = spans_by_speaker
        .into_iter()
        .map(|(raw_speaker, mut spans)| {
            spans.sort_by(|left, right| {
                left.start
                    .partial_cmp(&right.start)
                    .unwrap_or(Ordering::Equal)
            });
            let first_start = spans.first().map(|span| span.start).unwrap_or_default();
            (raw_speaker, first_start, spans)
        })
        .collect::<Vec<_>>();

    ordered.sort_by(|left, right| left.1.partial_cmp(&right.1).unwrap_or(Ordering::Equal));

    ordered
        .into_iter()
        .enumerate()
        .map(|(index, (raw_speaker, _, spans))| ClusterInfo {
            raw_speaker,
            spans,
            anonymous_tag: SpeakerTag {
                id: format!("anonymous-{}", index + 1),
                label: format!("Speaker {}", index + 1),
                kind: "anonymous".to_string(),
                score: None,
            },
        })
        .collect()
}

fn build_cluster_speaker_assignments(
    samples: &[f32],
    clusters: &[ClusterInfo],
    config: &SpeakerProcessingConfig,
    embedding_model: &Path,
) -> Result<HashMap<i32, ResolvedSpeakerAssignment>, String> {
    let default_assignments = clusters
        .iter()
        .map(|cluster| {
            (
                cluster.raw_speaker,
                build_anonymous_assignment(cluster, Vec::new(), "anonymous", "auto", "low"),
            )
        })
        .collect::<HashMap<_, _>>();

    let index_started = Instant::now();
    let enabled_profiles = config
        .speaker_profiles
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|profile| profile.enabled)
        .collect::<Vec<_>>();
    let mut index_summary = SpeakerProfileIndexSummary {
        enabled_profiles: enabled_profiles.len(),
        ..Default::default()
    };

    if enabled_profiles.is_empty() {
        log_speaker_profile_index_complete(&index_summary, elapsed_ms(index_started));
        log_speaker_processing_skip("no_enabled_profiles", "profile_index");
        log_speaker_matching_complete(&summarize_speaker_assignments(&default_assignments), 0.0);
        return Ok(default_assignments);
    }

    let embedding_index = sona_local_asr::speaker::SpeakerEmbeddingIndex::new(embedding_model)?;

    let mut loaded_profile_names = HashMap::new();
    let mut profile_readiness = HashMap::new();
    for profile in enabled_profiles {
        let readiness = derive_profile_readiness(&profile);
        if readiness == SpeakerProfileReadinessState::NotReady {
            index_summary.skipped_profiles += 1;
            continue;
        }

        let mut embeddings = Vec::new();
        for sample in &profile.samples {
            if sample.duration_seconds < PROFILE_SAMPLE_MIN_DURATION_SECONDS {
                continue;
            }
            if let Some(embedding) =
                embedding_index.compute_embedding_for_wav_file(&sample.file_path)?
            {
                embeddings.push(embedding);
            }
        }

        if embeddings.is_empty() {
            index_summary.skipped_profiles += 1;
            continue;
        }

        embedding_index.add_profile_embeddings(&profile.id, &profile.name, &embeddings)?;

        match readiness {
            SpeakerProfileReadinessState::Ready => index_summary.ready_profiles += 1,
            SpeakerProfileReadinessState::Limited => index_summary.limited_profiles += 1,
            SpeakerProfileReadinessState::NotReady => {}
        }
        index_summary.usable_sample_embeddings += embeddings.len();
        loaded_profile_names.insert(profile.id.clone(), profile.name.clone());
        profile_readiness.insert(profile.id.clone(), readiness);
    }

    log_speaker_profile_index_complete(&index_summary, elapsed_ms(index_started));
    if loaded_profile_names.is_empty() {
        log_speaker_processing_skip("no_usable_profiles", "profile_index");
        log_speaker_matching_complete(&summarize_speaker_assignments(&default_assignments), 0.0);
        return Ok(default_assignments);
    }

    let matching_started = Instant::now();
    let mut candidates = HashMap::new();
    for cluster in clusters {
        let cluster_candidates =
            identify_cluster_candidates(samples, cluster, &embedding_index, &loaded_profile_names)?;
        if !cluster_candidates.is_empty() {
            candidates.insert(cluster.raw_speaker, cluster_candidates);
        }
    }

    let mut assignments = resolve_cluster_assignments(clusters, &candidates, &profile_readiness);
    for (raw_speaker, assignment) in default_assignments {
        assignments.entry(raw_speaker).or_insert(assignment);
    }
    log_speaker_matching_complete(
        &summarize_speaker_assignments(&assignments),
        elapsed_ms(matching_started),
    );
    Ok(assignments)
}

fn identify_cluster_candidates(
    samples: &[f32],
    cluster: &ClusterInfo,
    embedding_index: &sona_local_asr::speaker::SpeakerEmbeddingIndex,
    profile_names: &HashMap<String, String>,
) -> Result<Vec<ClusterCandidate>, String> {
    let mut candidate_spans = cluster
        .spans
        .iter()
        .filter(|span| (span.end - span.start) >= IDENTIFICATION_MIN_DURATION_SECONDS)
        .cloned()
        .collect::<Vec<_>>();

    candidate_spans.sort_by(|left, right| {
        let left_duration = left.end - left.start;
        let right_duration = right.end - right.start;
        right_duration
            .partial_cmp(&left_duration)
            .unwrap_or(Ordering::Equal)
    });
    candidate_spans.truncate(IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER);

    if candidate_spans.is_empty() {
        return Ok(Vec::new());
    }

    let mut vote_counts: HashMap<String, usize> = HashMap::new();
    let mut score_sums: HashMap<String, f32> = HashMap::new();

    for span in candidate_spans {
        let Some(embedding) =
            embedding_index.compute_embedding_for_span(samples, span.start, span.end)?
        else {
            continue;
        };
        for best_match in embedding_index.best_matches(
            &embedding,
            CANDIDATE_DISPLAY_THRESHOLD,
            IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER as i32,
        ) {
            *vote_counts.entry(best_match.name.clone()).or_insert(0) += 1;
            *score_sums.entry(best_match.name).or_insert(0.0) += best_match.score;
        }
    }

    let mut candidates = vote_counts
        .into_iter()
        .filter_map(|(profile_id, votes)| {
            let profile_name = profile_names.get(profile_id.as_str())?.clone();
            let total_score = score_sums
                .get(profile_id.as_str())
                .copied()
                .unwrap_or_default();
            Some(ClusterCandidate {
                profile_id,
                profile_name,
                votes,
                average_score: total_score / votes as f32,
            })
        })
        .collect::<Vec<_>>();
    sort_cluster_candidates(&mut candidates);
    candidates.truncate(IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER);
    Ok(candidates)
}

fn sort_cluster_candidates(candidates: &mut [ClusterCandidate]) {
    candidates.sort_by(|left, right| {
        right
            .average_score
            .partial_cmp(&left.average_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.votes.cmp(&left.votes))
            .then_with(|| left.profile_name.cmp(&right.profile_name))
    });
}

fn derive_profile_readiness(profile: &SpeakerProfile) -> SpeakerProfileReadinessState {
    let usable_samples = profile
        .samples
        .iter()
        .filter(|sample| sample.duration_seconds >= PROFILE_SAMPLE_MIN_DURATION_SECONDS)
        .collect::<Vec<_>>();
    let usable_duration = usable_samples
        .iter()
        .map(|sample| sample.duration_seconds)
        .sum::<f32>();

    if usable_samples.len() >= PROFILE_READY_MIN_SAMPLE_COUNT
        && usable_duration >= PROFILE_READY_MIN_TOTAL_DURATION_SECONDS
    {
        return SpeakerProfileReadinessState::Ready;
    }

    if !usable_samples.is_empty() && usable_duration >= PROFILE_LIMITED_MIN_TOTAL_DURATION_SECONDS {
        return SpeakerProfileReadinessState::Limited;
    }

    SpeakerProfileReadinessState::NotReady
}

fn resolve_cluster_assignments(
    clusters: &[ClusterInfo],
    candidates_by_cluster: &HashMap<i32, Vec<ClusterCandidate>>,
    profile_readiness: &HashMap<String, SpeakerProfileReadinessState>,
) -> HashMap<i32, ResolvedSpeakerAssignment> {
    let clusters_by_id = clusters
        .iter()
        .map(|cluster| (cluster.raw_speaker, cluster))
        .collect::<HashMap<_, _>>();
    let mut assignments = clusters
        .iter()
        .map(|cluster| {
            let candidates = candidates_by_cluster
                .get(&cluster.raw_speaker)
                .cloned()
                .unwrap_or_default();
            (
                cluster.raw_speaker,
                resolve_single_cluster_assignment(cluster, candidates, profile_readiness),
            )
        })
        .collect::<HashMap<_, _>>();

    let mut identified = assignments
        .values()
        .filter_map(|assignment| {
            let profile_id = assignment.speaker.as_ref()?.id.clone();
            if assignment.attribution.state == "identified" {
                Some((
                    profile_id,
                    assignment.raw_speaker,
                    assignment.average_score.unwrap_or_default(),
                    assignment.votes,
                ))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    identified.sort_by(|left, right| {
        left.0.cmp(&right.0).then_with(|| {
            right
                .2
                .partial_cmp(&left.2)
                .unwrap_or(Ordering::Equal)
                .then_with(|| right.3.cmp(&left.3))
        })
    });

    let mut accepted_by_profile: HashMap<String, Vec<i32>> = HashMap::new();
    for (profile_id, raw_speaker, _, _) in identified {
        let Some(cluster) = clusters_by_id.get(&raw_speaker) else {
            continue;
        };

        let overlaps_existing = accepted_by_profile
            .get(profile_id.as_str())
            .into_iter()
            .flatten()
            .filter_map(|accepted_raw_speaker| clusters_by_id.get(accepted_raw_speaker))
            .any(|accepted_cluster| clusters_overlap(cluster, accepted_cluster));

        if overlaps_existing {
            if let Some(assignment) = assignments.get_mut(&raw_speaker) {
                downgrade_assignment_to_suggestion(assignment, cluster);
            }
            continue;
        }

        accepted_by_profile
            .entry(profile_id)
            .or_default()
            .push(raw_speaker);
    }

    assignments
}

fn resolve_single_cluster_assignment(
    cluster: &ClusterInfo,
    mut candidates: Vec<ClusterCandidate>,
    profile_readiness: &HashMap<String, SpeakerProfileReadinessState>,
) -> ResolvedSpeakerAssignment {
    if candidates.is_empty() {
        return build_anonymous_assignment(cluster, Vec::new(), "anonymous", "auto", "low");
    }

    sort_cluster_candidates(&mut candidates);
    candidates.truncate(IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER);

    let suggestion_candidates = build_speaker_candidates(&candidates);
    let top_candidate = &candidates[0];
    let second_score = candidates
        .get(1)
        .map(|candidate| candidate.average_score)
        .unwrap_or(0.0);
    let readiness = profile_readiness
        .get(top_candidate.profile_id.as_str())
        .copied()
        .unwrap_or(SpeakerProfileReadinessState::NotReady);
    let score_margin = top_candidate.average_score - second_score;

    if readiness == SpeakerProfileReadinessState::Ready
        && top_candidate.average_score >= AUTO_IDENTIFICATION_THRESHOLD
        && top_candidate.votes >= AUTO_IDENTIFICATION_MIN_VOTES
        && score_margin >= AUTO_IDENTIFICATION_MIN_MARGIN
    {
        return ResolvedSpeakerAssignment {
            raw_speaker: cluster.raw_speaker,
            speaker: Some(SpeakerTag {
                id: top_candidate.profile_id.clone(),
                label: top_candidate.profile_name.clone(),
                kind: "identified".to_string(),
                score: Some(top_candidate.average_score),
            }),
            attribution: SpeakerAttribution {
                group_id: cluster.anonymous_tag.id.clone(),
                anonymous_label: cluster.anonymous_tag.label.clone(),
                state: "identified".to_string(),
                source: "auto".to_string(),
                confidence: "high".to_string(),
                candidates: suggestion_candidates,
            },
            average_score: Some(top_candidate.average_score),
            votes: top_candidate.votes,
        };
    }

    if top_candidate.average_score >= CANDIDATE_DISPLAY_THRESHOLD {
        return build_anonymous_assignment(
            cluster,
            suggestion_candidates,
            "suggested",
            "auto",
            "medium",
        );
    }

    build_anonymous_assignment(cluster, Vec::new(), "anonymous", "auto", "low")
}

fn build_anonymous_assignment(
    cluster: &ClusterInfo,
    candidates: Vec<SpeakerCandidate>,
    state: &str,
    source: &str,
    confidence: &str,
) -> ResolvedSpeakerAssignment {
    ResolvedSpeakerAssignment {
        raw_speaker: cluster.raw_speaker,
        speaker: Some(cluster.anonymous_tag.clone()),
        attribution: SpeakerAttribution {
            group_id: cluster.anonymous_tag.id.clone(),
            anonymous_label: cluster.anonymous_tag.label.clone(),
            state: state.to_string(),
            source: source.to_string(),
            confidence: confidence.to_string(),
            candidates,
        },
        average_score: None,
        votes: 0,
    }
}

fn build_speaker_candidates(candidates: &[ClusterCandidate]) -> Vec<SpeakerCandidate> {
    candidates
        .iter()
        .take(IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER)
        .enumerate()
        .map(|(index, candidate)| SpeakerCandidate {
            profile_id: candidate.profile_id.clone(),
            profile_name: candidate.profile_name.clone(),
            score: candidate.average_score,
            rank: index + 1,
        })
        .collect()
}

fn clusters_overlap(left: &ClusterInfo, right: &ClusterInfo) -> bool {
    left.spans.iter().any(|left_span| {
        right.spans.iter().any(|right_span| {
            range_overlap(
                left_span.start,
                left_span.end,
                right_span.start,
                right_span.end,
            ) > 0.0
        })
    })
}

fn downgrade_assignment_to_suggestion(
    assignment: &mut ResolvedSpeakerAssignment,
    cluster: &ClusterInfo,
) {
    if assignment.attribution.candidates.is_empty() {
        assignment.speaker = Some(cluster.anonymous_tag.clone());
        assignment.attribution.state = "anonymous".to_string();
        assignment.attribution.confidence = "low".to_string();
        return;
    }

    assignment.speaker = Some(cluster.anonymous_tag.clone());
    assignment.attribution.state = "suggested".to_string();
    assignment.attribution.confidence = "medium".to_string();
}

fn apply_speaker_tags_to_segments(
    segments: &[TranscriptSegment],
    clusters: &[ClusterInfo],
    speaker_assignments: &HashMap<i32, ResolvedSpeakerAssignment>,
) -> Vec<TranscriptSegment> {
    let spans = clusters
        .iter()
        .flat_map(|cluster| cluster.spans.iter().cloned())
        .collect::<Vec<_>>();

    let mut annotated = Vec::new();
    for segment in segments {
        annotated.extend(assign_speakers_to_segment(
            segment,
            &spans,
            speaker_assignments,
        ));
    }

    annotated.sort_by(|left, right| {
        left.start
            .partial_cmp(&right.start)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.end.partial_cmp(&right.end).unwrap_or(Ordering::Equal))
    });
    annotated
}

fn assign_speakers_to_segment(
    segment: &TranscriptSegment,
    spans: &[SpeakerSpan],
    speaker_assignments: &HashMap<i32, ResolvedSpeakerAssignment>,
) -> Vec<TranscriptSegment> {
    let fallback_speaker = choose_speaker_for_range(
        segment.start as f32,
        segment.end as f32,
        spans,
        speaker_assignments,
    );

    if let Some(timing) = segment
        .timing
        .as_ref()
        .filter(|timing| timing.level == TranscriptTimingLevel::Token && !timing.units.is_empty())
    {
        let token_speakers = timing
            .units
            .iter()
            .map(|unit| {
                choose_speaker_for_range(
                    unit.start as f32,
                    unit.end as f32,
                    spans,
                    speaker_assignments,
                )
                .or_else(|| fallback_speaker.clone())
            })
            .collect::<Vec<_>>();

        if token_speakers.iter().all(|speaker| speaker.is_some()) {
            let aligned_units = timing
                .units
                .iter()
                .enumerate()
                .map(|(index, unit)| AlignedTextUnit {
                    text: unit.text.clone(),
                    token_index: index,
                })
                .collect::<Vec<_>>();

            if let Some(groups) = build_split_groups(&aligned_units, &token_speakers) {
                if groups.len() == 1 {
                    return vec![apply_assignment_to_segment(segment, &groups[0].assignment)];
                }

                let segments = groups
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, group)| {
                        let text = group.text.trim().to_string();
                        if text.is_empty() {
                            return None;
                        }

                        let timing_slice =
                            timing.units[group.token_start..group.token_end_exclusive].to_vec();
                        let start = timing_slice
                            .first()
                            .map(|unit| unit.start)
                            .unwrap_or(segment.start);
                        let end = timing_slice
                            .last()
                            .map(|unit| unit.end.max(unit.start))
                            .unwrap_or(segment.end)
                            .max(start);

                        let mut next_segment = TranscriptSegment {
                            id: if index == 0 {
                                segment.id.clone()
                            } else {
                                uuid::Uuid::new_v4().to_string()
                            },
                            text,
                            start,
                            end,
                            is_final: segment.is_final,
                            timing: Some(TranscriptTiming {
                                level: timing.level,
                                source: timing.source,
                                units: timing_slice,
                            }),
                            tokens: None,
                            timestamps: None,
                            durations: None,
                            translation: None,
                            speaker: group.assignment.speaker.clone(),
                            speaker_attribution: Some(group.assignment.attribution.clone()),
                        };
                        ensure_transcript_segment_timing(&mut next_segment);
                        Some(next_segment)
                    })
                    .collect::<Vec<_>>();

                if !segments.is_empty() {
                    return segments;
                }
            }
        }
    }

    let Some(tokens) = segment.tokens.as_ref() else {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    };
    let Some(timestamps) = segment.timestamps.as_ref() else {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    };

    if tokens.is_empty() || tokens.len() != timestamps.len() {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    }

    let durations = segment
        .durations
        .as_ref()
        .filter(|values| values.len() == tokens.len());
    let token_speakers = tokens
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let start = timestamps[index];
            let end = if let Some(duration_values) = durations {
                start + duration_values[index]
            } else if index + 1 < timestamps.len() {
                timestamps[index + 1]
            } else {
                segment.end as f32
            };
            choose_speaker_for_range(start, end, spans, speaker_assignments)
                .or_else(|| fallback_speaker.clone())
        })
        .collect::<Vec<_>>();

    if token_speakers.iter().any(|speaker| speaker.is_none()) {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    }

    let Some(aligned_units) = align_text_units_to_tokens(&segment.text, tokens) else {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    };

    let Some(groups) = build_split_groups(&aligned_units, &token_speakers) else {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    };
    if groups.is_empty() {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    }

    if groups.len() == 1 {
        return vec![apply_assignment_to_segment(segment, &groups[0].assignment)];
    }

    let mut segments = groups
        .into_iter()
        .enumerate()
        .filter_map(|(index, group)| {
            let text = group.text.trim().to_string();
            if text.is_empty() {
                return None;
            }

            let start = timestamps
                .get(group.token_start)
                .copied()
                .unwrap_or(segment.start as f32);
            let end = if group.token_end_exclusive < timestamps.len() {
                timestamps[group.token_end_exclusive]
            } else {
                segment.end as f32
            };

            let token_slice = tokens[group.token_start..group.token_end_exclusive].to_vec();
            let timestamp_slice = timestamps[group.token_start..group.token_end_exclusive].to_vec();
            let duration_slice = durations
                .map(|values| values[group.token_start..group.token_end_exclusive].to_vec());

            Some(TranscriptSegment {
                id: if index == 0 {
                    segment.id.clone()
                } else {
                    uuid::Uuid::new_v4().to_string()
                },
                text,
                start: start as f64,
                end: end.max(start) as f64,
                is_final: segment.is_final,
                timing: None,
                tokens: Some(token_slice),
                timestamps: Some(timestamp_slice),
                durations: duration_slice,
                translation: None,
                speaker: group.assignment.speaker.clone(),
                speaker_attribution: Some(group.assignment.attribution.clone()),
            })
        })
        .collect::<Vec<_>>();

    for segment in &mut segments {
        ensure_transcript_segment_timing(segment);
    }

    segments
}

fn apply_speaker_to_whole_segment(
    segment: &TranscriptSegment,
    assignment: Option<ResolvedSpeakerAssignment>,
) -> TranscriptSegment {
    let Some(assignment) = assignment else {
        return segment.clone();
    };
    apply_assignment_to_segment(segment, &assignment)
}

fn apply_assignment_to_segment(
    segment: &TranscriptSegment,
    assignment: &ResolvedSpeakerAssignment,
) -> TranscriptSegment {
    TranscriptSegment {
        speaker: assignment.speaker.clone(),
        speaker_attribution: Some(assignment.attribution.clone()),
        ..segment.clone()
    }
}

fn build_split_groups(
    aligned_units: &[AlignedTextUnit],
    token_speakers: &[Option<ResolvedSpeakerAssignment>],
) -> Option<Vec<SplitGroup>> {
    let mut groups: Vec<SplitGroup> = Vec::new();

    for unit in aligned_units {
        let assignment = token_speakers.get(unit.token_index)?.clone()?;
        if let Some(current) = groups.last_mut()
            && speaker_assignments_equal(&current.assignment, &assignment)
        {
            current.text.push_str(&unit.text);
            current.token_end_exclusive = current.token_end_exclusive.max(unit.token_index + 1);
            continue;
        }

        groups.push(SplitGroup {
            assignment,
            text: unit.text.clone(),
            token_start: unit.token_index,
            token_end_exclusive: unit.token_index + 1,
        });
    }

    Some(groups)
}

fn choose_speaker_for_range(
    start: f32,
    end: f32,
    spans: &[SpeakerSpan],
    speaker_assignments: &HashMap<i32, ResolvedSpeakerAssignment>,
) -> Option<ResolvedSpeakerAssignment> {
    let mut best_span: Option<&SpeakerSpan> = None;
    let mut best_overlap = 0.0_f32;

    for span in spans {
        let overlap = range_overlap(start, end, span.start, span.end);
        if overlap > best_overlap {
            best_overlap = overlap;
            best_span = Some(span);
        }
    }

    if let Some(span) = best_span {
        return speaker_assignments.get(&span.raw_speaker).cloned();
    }

    let midpoint = (start + end) / 2.0;
    spans
        .iter()
        .min_by(|left, right| {
            let left_distance = distance_to_range(midpoint, left.start, left.end);
            let right_distance = distance_to_range(midpoint, right.start, right.end);
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(Ordering::Equal)
        })
        .and_then(|span| speaker_assignments.get(&span.raw_speaker).cloned())
}

fn range_overlap(start: f32, end: f32, other_start: f32, other_end: f32) -> f32 {
    (end.min(other_end) - start.max(other_start)).max(0.0)
}

fn distance_to_range(value: f32, start: f32, end: f32) -> f32 {
    if value < start {
        start - value
    } else if value > end {
        value - end
    } else {
        0.0
    }
}

fn speaker_tags_equal(left: &SpeakerTag, right: &SpeakerTag) -> bool {
    left.id == right.id && left.label == right.label && left.kind == right.kind
}

fn speaker_assignments_equal(
    left: &ResolvedSpeakerAssignment,
    right: &ResolvedSpeakerAssignment,
) -> bool {
    left.attribution.group_id == right.attribution.group_id
        && match (&left.speaker, &right.speaker) {
            (Some(left_speaker), Some(right_speaker)) => {
                speaker_tags_equal(left_speaker, right_speaker)
            }
            (None, None) => true,
            _ => false,
        }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::text_alignment::lex_text_units;

    fn speaker(id: &str, label: &str, kind: &str, score: Option<f32>) -> SpeakerTag {
        SpeakerTag {
            id: id.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            score,
        }
    }

    fn sample_segment(start: f64, end: f64, text: &str) -> TranscriptSegment {
        TranscriptSegment {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.to_string(),
            start,
            end,
            is_final: true,
            timing: None,
            tokens: None,
            timestamps: None,
            durations: None,
            translation: None,
            speaker: None,
            speaker_attribution: None,
        }
    }

    fn cluster(raw_speaker: i32, start: f32, end: f32, index: usize) -> ClusterInfo {
        ClusterInfo {
            raw_speaker,
            spans: vec![SpeakerSpan {
                start,
                end,
                raw_speaker,
            }],
            anonymous_tag: SpeakerTag {
                id: format!("anonymous-{}", index),
                label: format!("Speaker {}", index),
                kind: "anonymous".to_string(),
                score: None,
            },
        }
    }

    fn candidate(
        _raw_speaker: i32,
        profile_id: &str,
        profile_name: &str,
        votes: usize,
        average_score: f32,
    ) -> ClusterCandidate {
        ClusterCandidate {
            profile_id: profile_id.to_string(),
            profile_name: profile_name.to_string(),
            votes,
            average_score,
        }
    }

    fn resolved_assignment(
        raw_speaker: i32,
        speaker: SpeakerTag,
        group_id: &str,
        anonymous_label: &str,
    ) -> ResolvedSpeakerAssignment {
        ResolvedSpeakerAssignment {
            raw_speaker,
            average_score: speaker.score,
            votes: 1,
            speaker: Some(speaker),
            attribution: SpeakerAttribution {
                group_id: group_id.to_string(),
                anonymous_label: anonymous_label.to_string(),
                state: "identified".to_string(),
                source: "auto".to_string(),
                confidence: "high".to_string(),
                candidates: Vec::new(),
            },
        }
    }

    #[test]
    fn build_cluster_infos_orders_anonymous_labels_by_first_start_time() {
        let diarization_segments = vec![
            SpeakerDiarizationSegment {
                start: 6.0,
                end: 8.0,
                speaker: 10,
            },
            SpeakerDiarizationSegment {
                start: 0.5,
                end: 2.0,
                speaker: 42,
            },
        ];

        let clusters = build_cluster_infos(&diarization_segments);

        assert_eq!(clusters.len(), 2);
        assert_eq!(clusters[0].raw_speaker, 42);
        assert_eq!(clusters[0].anonymous_tag.label, "Speaker 1");
        assert_eq!(clusters[1].raw_speaker, 10);
        assert_eq!(clusters[1].anonymous_tag.label, "Speaker 2");
    }

    #[test]
    fn resolve_cluster_candidates_keeps_only_highest_score_per_profile() {
        let clusters = vec![cluster(1, 0.0, 3.0, 1)];
        let candidates = HashMap::from([(
            1,
            vec![
                candidate(1, "alice", "Alice", 2, 0.91),
                candidate(1, "bob", "Bob", 2, 0.8),
            ],
        )]);
        let readiness = HashMap::from([
            ("alice".to_string(), SpeakerProfileReadinessState::Ready),
            ("bob".to_string(), SpeakerProfileReadinessState::Ready),
        ]);

        let resolved = resolve_cluster_assignments(&clusters, &candidates, &readiness);

        assert_eq!(
            resolved
                .get(&1)
                .and_then(|value| value.speaker.as_ref())
                .map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            resolved
                .get(&1)
                .map(|value| value.attribution.state.as_str()),
            Some("identified")
        );
    }

    #[test]
    fn resolve_cluster_assignments_prioritize_higher_average_score_before_votes() {
        let clusters = vec![cluster(1, 0.0, 3.0, 1)];
        let candidates = HashMap::from([(
            1,
            vec![
                candidate(1, "alice", "Alice", 2, 0.91),
                candidate(1, "bob", "Bob", 3, 0.82),
            ],
        )]);
        let readiness = HashMap::from([
            ("alice".to_string(), SpeakerProfileReadinessState::Ready),
            ("bob".to_string(), SpeakerProfileReadinessState::Ready),
        ]);

        let resolved = resolve_cluster_assignments(&clusters, &candidates, &readiness);

        assert_eq!(
            resolved
                .get(&1)
                .and_then(|value| value.speaker.as_ref())
                .map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            resolved.get(&1).map(|value| {
                value
                    .attribution
                    .candidates
                    .iter()
                    .map(|candidate| candidate.profile_name.as_str())
                    .collect::<Vec<_>>()
            }),
            Some(vec!["Alice", "Bob"])
        );
    }

    #[test]
    fn resolve_cluster_assignments_keeps_limited_profiles_as_suggestions() {
        let clusters = vec![cluster(1, 0.0, 3.0, 1)];
        let candidates = HashMap::from([(1, vec![candidate(1, "alice", "Alice", 3, 0.87)])]);
        let readiness =
            HashMap::from([("alice".to_string(), SpeakerProfileReadinessState::Limited)]);

        let resolved = resolve_cluster_assignments(&clusters, &candidates, &readiness);
        let assignment = resolved.get(&1).expect("assignment");

        assert_eq!(
            assignment
                .speaker
                .as_ref()
                .map(|value| value.label.as_str()),
            Some("Speaker 1")
        );
        assert_eq!(assignment.attribution.state, "suggested");
        assert_eq!(assignment.attribution.confidence, "medium");
        assert_eq!(assignment.attribution.candidates.len(), 1);
        assert_eq!(assignment.attribution.candidates[0].profile_name, "Alice");
    }

    #[test]
    fn resolve_cluster_assignments_allow_same_profile_for_non_overlapping_clusters() {
        let clusters = vec![cluster(1, 0.0, 3.0, 1), cluster(2, 5.0, 8.0, 2)];
        let candidates = HashMap::from([
            (1, vec![candidate(1, "alice", "Alice", 3, 0.89)]),
            (2, vec![candidate(2, "alice", "Alice", 2, 0.82)]),
        ]);
        let readiness = HashMap::from([("alice".to_string(), SpeakerProfileReadinessState::Ready)]);

        let resolved = resolve_cluster_assignments(&clusters, &candidates, &readiness);

        assert_eq!(
            resolved
                .get(&1)
                .and_then(|value| value.speaker.as_ref())
                .map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            resolved
                .get(&2)
                .and_then(|value| value.speaker.as_ref())
                .map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            resolved
                .get(&1)
                .map(|value| value.attribution.state.as_str()),
            Some("identified")
        );
        assert_eq!(
            resolved
                .get(&2)
                .map(|value| value.attribution.state.as_str()),
            Some("identified")
        );
    }

    #[test]
    fn resolve_cluster_assignments_downgrades_weaker_overlapping_profile_claims() {
        let clusters = vec![cluster(1, 0.0, 4.0, 1), cluster(2, 2.0, 5.0, 2)];
        let candidates = HashMap::from([
            (1, vec![candidate(1, "alice", "Alice", 3, 0.92)]),
            (2, vec![candidate(2, "alice", "Alice", 2, 0.81)]),
        ]);
        let readiness = HashMap::from([("alice".to_string(), SpeakerProfileReadinessState::Ready)]);

        let resolved = resolve_cluster_assignments(&clusters, &candidates, &readiness);

        assert_eq!(
            resolved
                .get(&1)
                .and_then(|value| value.speaker.as_ref())
                .map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            resolved
                .get(&1)
                .map(|value| value.attribution.state.as_str()),
            Some("identified")
        );
        assert_eq!(
            resolved
                .get(&2)
                .and_then(|value| value.speaker.as_ref())
                .map(|value| value.label.as_str()),
            Some("Speaker 2")
        );
        assert_eq!(
            resolved
                .get(&2)
                .map(|value| value.attribution.state.as_str()),
            Some("suggested")
        );
    }

    #[test]
    fn speaker_assignment_summary_counts_states_and_candidate_clusters() {
        let assignments = HashMap::from([
            (
                1,
                ResolvedSpeakerAssignment {
                    raw_speaker: 1,
                    speaker: Some(speaker("profile-alice", "Alice", "identified", Some(0.91))),
                    attribution: SpeakerAttribution {
                        group_id: "anonymous-1".to_string(),
                        anonymous_label: "Speaker 1".to_string(),
                        state: "identified".to_string(),
                        source: "auto".to_string(),
                        confidence: "high".to_string(),
                        candidates: vec![SpeakerCandidate {
                            profile_id: "profile-alice".to_string(),
                            profile_name: "Alice".to_string(),
                            score: 0.91,
                            rank: 1,
                        }],
                    },
                    average_score: Some(0.91),
                    votes: 2,
                },
            ),
            (
                2,
                ResolvedSpeakerAssignment {
                    raw_speaker: 2,
                    speaker: Some(speaker("anonymous-2", "Speaker 2", "anonymous", None)),
                    attribution: SpeakerAttribution {
                        group_id: "anonymous-2".to_string(),
                        anonymous_label: "Speaker 2".to_string(),
                        state: "suggested".to_string(),
                        source: "auto".to_string(),
                        confidence: "medium".to_string(),
                        candidates: vec![SpeakerCandidate {
                            profile_id: "profile-bob".to_string(),
                            profile_name: "Bob".to_string(),
                            score: 0.74,
                            rank: 1,
                        }],
                    },
                    average_score: None,
                    votes: 0,
                },
            ),
            (
                3,
                ResolvedSpeakerAssignment {
                    raw_speaker: 3,
                    speaker: Some(speaker("anonymous-3", "Speaker 3", "anonymous", None)),
                    attribution: SpeakerAttribution {
                        group_id: "anonymous-3".to_string(),
                        anonymous_label: "Speaker 3".to_string(),
                        state: "anonymous".to_string(),
                        source: "auto".to_string(),
                        confidence: "low".to_string(),
                        candidates: Vec::new(),
                    },
                    average_score: None,
                    votes: 0,
                },
            ),
        ]);

        let summary = summarize_speaker_assignments(&assignments);

        assert_eq!(summary.identified, 1);
        assert_eq!(summary.suggested, 1);
        assert_eq!(summary.anonymous, 1);
        assert_eq!(summary.candidate_clusters, 2);
    }

    #[test]
    fn count_added_speaker_segments_never_reports_negative_values() {
        assert_eq!(count_added_speaker_segments(3, 7), 4);
        assert_eq!(count_added_speaker_segments(7, 3), 0);
        assert_eq!(count_added_speaker_segments(5, 5), 0);
    }

    #[test]
    fn whole_segment_fallback_uses_largest_overlap_when_no_token_timestamps() {
        let segment = sample_segment(0.0, 4.0, "Hello");
        let spans = vec![
            SpeakerSpan {
                start: 0.0,
                end: 1.0,
                raw_speaker: 1,
            },
            SpeakerSpan {
                start: 1.0,
                end: 4.0,
                raw_speaker: 2,
            },
        ];
        let tags = HashMap::from([
            (
                1,
                resolved_assignment(
                    1,
                    speaker("anonymous-1", "Speaker 1", "anonymous", None),
                    "anonymous-1",
                    "Speaker 1",
                ),
            ),
            (
                2,
                resolved_assignment(
                    2,
                    speaker("profile-bob", "Bob", "identified", Some(0.82)),
                    "anonymous-2",
                    "Speaker 2",
                ),
            ),
        ]);

        let result = assign_speakers_to_segment(&segment, &spans, &tags);

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].speaker.as_ref().map(|value| value.label.as_str()),
            Some("Bob")
        );
    }

    #[test]
    fn token_level_timing_allows_speaker_split_groups() {
        let mut segment = sample_segment(0.0, 2.0, "Hello there");
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: crate::integrations::asr::TranscriptTimingSource::Model,
            units: vec![
                crate::integrations::asr::TranscriptTimingUnit {
                    text: "Hello".to_string(),
                    start: 0.0,
                    end: 1.0,
                },
                crate::integrations::asr::TranscriptTimingUnit {
                    text: " there".to_string(),
                    start: 1.0,
                    end: 2.0,
                },
            ],
        });

        let spans = vec![
            SpeakerSpan {
                start: 0.0,
                end: 1.0,
                raw_speaker: 1,
            },
            SpeakerSpan {
                start: 1.0,
                end: 2.0,
                raw_speaker: 2,
            },
        ];
        let tags = HashMap::from([
            (
                1,
                resolved_assignment(
                    1,
                    speaker("speaker-1", "Alice", "identified", Some(0.9)),
                    "anonymous-1",
                    "Speaker 1",
                ),
            ),
            (
                2,
                resolved_assignment(
                    2,
                    speaker("speaker-2", "Bob", "identified", Some(0.85)),
                    "anonymous-2",
                    "Speaker 2",
                ),
            ),
        ]);

        let result = assign_speakers_to_segment(&segment, &spans, &tags);

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0].speaker.as_ref().map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            result[1].speaker.as_ref().map(|value| value.label.as_str()),
            Some("Bob")
        );
        assert_eq!(
            result[0].timing.as_ref().map(|timing| timing.units.len()),
            Some(1)
        );
        assert_eq!(
            result[1].timing.as_ref().map(|timing| timing.units.len()),
            Some(1)
        );
    }

    #[test]
    fn lex_text_units_keeps_whitespace_and_attaches_punctuation() {
        let units = lex_text_units("Hello, world!");
        assert_eq!(units.len(), 3);
        assert_eq!(units[0].text, "Hello,");
        assert_eq!(units[1].text, " ");
        assert_eq!(units[2].text, "world!");
    }
}
