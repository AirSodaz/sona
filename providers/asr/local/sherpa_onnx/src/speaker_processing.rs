use log::{debug, info};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use sona_core::transcription::speaker::{
    SpeakerProcessingConfig, SpeakerProfile, SpeakerProfileSample,
};
use sona_core::transcription::text_alignment::AlignedTextUnit;
use sona_core::transcription::transcript::{
    SpeakerAttribution, SpeakerCandidate, SpeakerTag, TranscriptSegment, TranscriptTiming,
    TranscriptTimingLevel, TranscriptTimingUnit, ensure_transcript_segment_timing,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::speaker::SpeakerDiarizationSegment;

const SPEAKER_PROCESSING_LOG_TARGET: &str = "speaker_processing";
const SAMPLE_RATE: i32 = 16_000;
const IDENTIFICATION_MIN_DURATION_SECONDS: f32 = 1.5;
const IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER: usize = 3;
pub(crate) const CANDIDATE_DISPLAY_THRESHOLD: f32 = 0.6;
pub(crate) const AUTO_IDENTIFICATION_THRESHOLD: f32 = 0.72;
pub(crate) const AUTO_IDENTIFICATION_MIN_VOTES: usize = 2;
pub(crate) const AUTO_IDENTIFICATION_MIN_MARGIN: f32 = 0.08;
const CLUSTER_OVER_SEGMENTATION_MERGE_THRESHOLD: f32 = 0.85;
const PROFILE_SAMPLE_MIN_DURATION_SECONDS: f32 = 4.0;
const PROFILE_LIMITED_MIN_TOTAL_DURATION_SECONDS: f32 = 8.0;
const PROFILE_READY_MIN_TOTAL_DURATION_SECONDS: f32 = 20.0;
const PROFILE_READY_MIN_SAMPLE_COUNT: usize = 2;

#[derive(Debug, Clone)]
pub(crate) struct ProfileSampleEmbedding {
    pub(crate) embedding: Vec<f32>,
    pub(crate) duration_seconds: f32,
}

struct DisjointSet {
    parent: Vec<usize>,
}

impl DisjointSet {
    fn new(size: usize) -> Self {
        Self {
            parent: (0..size).collect(),
        }
    }

    fn find(&mut self, i: usize) -> usize {
        let mut root = i;
        while root != self.parent[root] {
            root = self.parent[root];
        }
        let mut curr = i;
        while curr != root {
            let next = self.parent[curr];
            self.parent[curr] = root;
            curr = next;
        }
        root
    }

    fn union(&mut self, i: usize, j: usize) {
        let root_i = self.find(i);
        let root_j = self.find(j);
        if root_i != root_j {
            self.parent[root_j] = root_i;
        }
    }
}

pub(crate) fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }
    if norm_a <= 1e-8 || norm_b <= 1e-8 {
        0.0
    } else {
        (dot / (norm_a.sqrt() * norm_b.sqrt())).clamp(-1.0, 1.0)
    }
}
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
pub(crate) enum SpeakerProfileReadinessState {
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
) -> Result<Vec<TranscriptSegment>, AsrPortError> {
    if segments.is_empty() {
        return Ok(segments);
    }

    let samples = crate::audio::extract_and_resample_audio(
        std::path::Path::new(&file_path),
        SAMPLE_RATE as u32,
    )
    .await?;
    annotate_segments_with_speakers(&samples, &segments, speaker_processing.as_ref())
}

pub async fn import_speaker_profile_sample(
    app_data_dir: &Path,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, AsrPortError> {
    let samples = crate::audio::extract_and_resample_audio(
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

    let profile_dir = app_data_dir.join("speaker-profiles").join(&profile_id);
    std::fs::create_dir_all(&profile_dir).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!(
                "Failed to create speaker profile directory {}: {error}",
                profile_dir.display()
            ),
        )
    })?;

    let output_path = profile_dir.join(format!("{sample_id}.wav"));
    crate::audio::save_wav_file(&samples, SAMPLE_RATE as u32, &output_path).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!(
                "Failed to save speaker profile sample {}: {error}",
                output_path.display()
            ),
        )
    })?;

    Ok(SpeakerProfileSample {
        id: sample_id,
        file_path: output_path.to_string_lossy().into_owned(),
        source_name: sample_name,
        duration_seconds,
    })
}

pub async fn enroll_speaker_profile_sample_from_audio(
    app_data_dir: &Path,
    profile_id: String,
    source_audio_path: String,
    start_seconds: f64,
    end_seconds: f64,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, AsrPortError> {
    if end_seconds <= start_seconds {
        return Err(AsrPortError::invalid_request(
            "Sample end time must be greater than start time",
        ));
    }
    let all_samples =
        crate::audio::extract_and_resample_audio(Path::new(&source_audio_path), SAMPLE_RATE as u32)
            .await?;

    let start_idx = ((start_seconds.max(0.0)) * SAMPLE_RATE as f64).floor() as usize;
    let end_idx = ((end_seconds.max(0.0)) * SAMPLE_RATE as f64).ceil() as usize;
    if start_idx >= all_samples.len() || end_idx <= start_idx {
        return Err(AsrPortError::invalid_request(
            "Selected sample range is out of audio bounds",
        ));
    }

    let bounded_end = end_idx.min(all_samples.len());
    let slice = &all_samples[start_idx..bounded_end];
    let duration_seconds = slice.len() as f32 / SAMPLE_RATE as f32;

    let sample_id = uuid::Uuid::new_v4().to_string();
    let sample_name = source_name
        .as_deref()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.trim().to_string())
        .unwrap_or_else(|| format!("Segment {:.1}s-{:.1}s", start_seconds, end_seconds));

    let profile_dir = app_data_dir.join("speaker-profiles").join(&profile_id);
    std::fs::create_dir_all(&profile_dir).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!(
                "Failed to create speaker profile directory {}: {error}",
                profile_dir.display()
            ),
        )
    })?;

    let output_path = profile_dir.join(format!("{sample_id}.wav"));
    crate::audio::save_wav_file(slice, SAMPLE_RATE as u32, &output_path).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!(
                "Failed to save speaker profile sample {}: {error}",
                output_path.display()
            ),
        )
    })?;

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
) -> Result<Vec<TranscriptSegment>, AsrPortError> {
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
    let embedding_index = crate::speaker::SpeakerEmbeddingIndex::new(&embedding_model)?;
    let refined =
        refine_clusters_and_repair_oversegmentation(samples, clusters, segments, &embedding_index)?;
    let mut speaker_assignments = build_cluster_speaker_assignments(
        samples,
        &refined.clusters,
        &refined.cluster_centroids,
        &refined.purified_spans_by_cluster,
        segments,
        config,
        &embedding_index,
    )?;

    for (old_spk, new_spk) in refined.merged_speaker_map {
        if let Some(canonical) = speaker_assignments.get(&new_spk).cloned() {
            speaker_assignments.insert(old_spk, canonical);
        }
    }

    let annotated_segments =
        apply_speaker_tags_to_segments(segments, &refined.clusters, &speaker_assignments);
    let assignment_summary = summarize_speaker_assignments(&speaker_assignments);
    log_speaker_processing_complete(
        total_started,
        input_segment_count,
        annotated_segments.len(),
        &assignment_summary,
    );
    Ok(annotated_segments)
}

pub(crate) fn resolve_model_path(input: Option<&str>) -> Result<PathBuf, AsrPortError> {
    let raw = input
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AsrPortError::invalid_request("Speaker processing models are not fully configured")
        })?;
    let path = Path::new(raw);
    if !path.exists() {
        return Err(AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("Speaker model path does not exist: {raw}"),
        ));
    }

    if path.is_file() {
        return Ok(path.to_path_buf());
    }

    let mut onnx_files = std::fs::read_dir(path)
        .map_err(|error| {
            AsrPortError::new(
                AsrPortErrorKind::FileSystem,
                format!(
                    "Failed to read speaker model directory {}: {error}",
                    path.display()
                ),
            )
        })?
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
    onnx_files.into_iter().next().ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            format!("No .onnx file found in {}", path.display()),
        )
    })
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
) -> Result<Vec<SpeakerDiarizationSegment>, AsrPortError> {
    crate::speaker::run_speaker_diarization(samples, segmentation_model, embedding_model)
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

fn extract_purified_spans_for_cluster(
    cluster: &ClusterInfo,
    all_clusters: &[ClusterInfo],
    segments: &[TranscriptSegment],
) -> Vec<SpeakerSpan> {
    let mut token_spans: Vec<(f32, f32)> = Vec::new();
    for segment in segments {
        if let Some(timing) = &segment.timing
            && timing.level == TranscriptTimingLevel::Token
        {
            for unit in &timing.units {
                if unit.end > unit.start {
                    token_spans.push((unit.start as f32, unit.end as f32));
                }
            }
        }
    }

    if token_spans.is_empty() {
        return cluster.spans.clone();
    }

    token_spans.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(Ordering::Equal));

    let other_spans: Vec<&SpeakerSpan> = all_clusters
        .iter()
        .filter(|c| c.raw_speaker != cluster.raw_speaker)
        .flat_map(|c| &c.spans)
        .collect();
    let mut purified = Vec::new();
    for span in &cluster.spans {
        if span.end <= span.start {
            continue;
        }

        let mut overlapping_tokens: Vec<(f32, f32)> = Vec::new();
        for (t_start, t_end) in &token_spans {
            let overlap_start = span.start.max(*t_start);
            let overlap_end = span.end.min(*t_end);
            if overlap_end > overlap_start {
                overlapping_tokens.push((overlap_start, overlap_end));
            }
        }

        if overlapping_tokens.is_empty() {
            continue;
        }

        let mut merged_blocks: Vec<(f32, f32)> = Vec::new();
        for (t_start, t_end) in overlapping_tokens {
            if let Some(last) = merged_blocks.last_mut()
                && t_start - last.1 < 0.25
            {
                last.1 = last.1.max(t_end);
                continue;
            }
            merged_blocks.push((t_start, t_end));
        }

        for (b_start, b_end) in merged_blocks {
            // Exclude blocks with cross-talk (overlap with another speaker)
            let has_cross_talk = other_spans
                .iter()
                .any(|other| range_overlap(b_start, b_end, other.start, other.end) > 0.1);
            if has_cross_talk {
                continue;
            }

            let dur = b_end - b_start;
            if dur >= 0.8 {
                let (final_start, final_end) = if dur >= 1.2 {
                    (b_start + 0.04, b_end - 0.04)
                } else {
                    (b_start, b_end)
                };
                purified.push(SpeakerSpan {
                    start: final_start,
                    end: final_end,
                    raw_speaker: span.raw_speaker,
                });
            }
        }
    }

    if purified.is_empty() {
        cluster.spans.clone()
    } else {
        purified.sort_by(|a, b| {
            let dur_a = a.end - a.start;
            let dur_b = b.end - b.start;
            dur_b.partial_cmp(&dur_a).unwrap_or(Ordering::Equal)
        });
        purified.truncate(IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER * 2);
        purified
    }
}

fn compute_cluster_centroid_embedding(
    samples: &[f32],
    purified_spans: &[SpeakerSpan],
    embedding_index: &crate::speaker::SpeakerEmbeddingIndex,
) -> Result<Option<Vec<f32>>, AsrPortError> {
    let mut embeddings = Vec::new();
    for span in purified_spans
        .iter()
        .take(IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER)
    {
        if let Some(embedding) =
            embedding_index.compute_embedding_for_span(samples, span.start, span.end)?
        {
            embeddings.push(embedding);
        }
    }

    if embeddings.is_empty() {
        return Ok(None);
    }

    let dim = embeddings[0].len();
    let mut mean = vec![0.0_f32; dim];
    for emb in &embeddings {
        for (i, val) in emb.iter().enumerate() {
            mean[i] += val;
        }
    }

    let norm = mean.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for val in &mut mean {
            *val /= norm;
        }
        Ok(Some(mean))
    } else {
        Ok(None)
    }
}

fn repair_cluster_oversegmentation(
    clusters: Vec<ClusterInfo>,
    cluster_centroids: &HashMap<i32, Vec<f32>>,
    threshold: f32,
) -> (Vec<ClusterInfo>, HashMap<i32, i32>) {
    if clusters.len() <= 1 {
        return (clusters, HashMap::new());
    }

    let n = clusters.len();
    let mut uf = DisjointSet::new(n);

    for i in 0..n {
        for j in (i + 1)..n {
            let spk_i = clusters[i].raw_speaker;
            let spk_j = clusters[j].raw_speaker;
            if let (Some(c_i), Some(c_j)) =
                (cluster_centroids.get(&spk_i), cluster_centroids.get(&spk_j))
            {
                let sim = cosine_similarity(c_i, c_j);
                if sim >= threshold {
                    uf.union(i, j);
                    info!(
                        target: SPEAKER_PROCESSING_LOG_TARGET,
                        "event=speaker_oversegmentation_merged cluster_a={} cluster_b={} similarity={:.3}",
                        spk_i,
                        spk_j,
                        sim,
                    );
                }
            }
        }
    }

    let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    for i in 0..n {
        let root = uf.find(i);
        groups.entry(root).or_default().push(i);
    }

    if groups.len() == n {
        return (clusters, HashMap::new());
    }

    let mut merged_mapping = HashMap::new();
    let mut merged_clusters = Vec::new();

    for (group_idx, (_root, indices)) in groups.into_iter().enumerate() {
        let canonical_idx = indices[0];
        let canonical_raw_speaker = clusters[canonical_idx].raw_speaker;

        let mut combined_spans = Vec::new();
        for &idx in &indices {
            let raw_spk = clusters[idx].raw_speaker;
            merged_mapping.insert(raw_spk, canonical_raw_speaker);
            for mut span in clusters[idx].spans.clone() {
                span.raw_speaker = canonical_raw_speaker;
                combined_spans.push(span);
            }
        }

        combined_spans.sort_by(|a, b| a.start.partial_cmp(&b.start).unwrap_or(Ordering::Equal));

        merged_clusters.push(ClusterInfo {
            raw_speaker: canonical_raw_speaker,
            spans: combined_spans,
            anonymous_tag: SpeakerTag {
                id: format!("anonymous-{}", group_idx + 1),
                label: format!("Speaker {}", group_idx + 1),
                kind: "anonymous".to_string(),
                score: None,
            },
        });
    }

    (merged_clusters, merged_mapping)
}
struct RefinedClusterOutcome {
    clusters: Vec<ClusterInfo>,
    merged_speaker_map: HashMap<i32, i32>,
    cluster_centroids: HashMap<i32, Vec<f32>>,
    purified_spans_by_cluster: HashMap<i32, Vec<SpeakerSpan>>,
}

fn refine_clusters_and_repair_oversegmentation(
    samples: &[f32],
    clusters: Vec<ClusterInfo>,
    segments: &[TranscriptSegment],
    embedding_index: &crate::speaker::SpeakerEmbeddingIndex,
) -> Result<RefinedClusterOutcome, AsrPortError> {
    let mut initial_purified_spans = HashMap::new();
    let mut cluster_centroids = HashMap::new();
    for cluster in &clusters {
        let purified_spans = extract_purified_spans_for_cluster(cluster, &clusters, segments);
        if let Some(centroid) =
            compute_cluster_centroid_embedding(samples, &purified_spans, embedding_index)?
        {
            cluster_centroids.insert(cluster.raw_speaker, centroid);
        }
        initial_purified_spans.insert(cluster.raw_speaker, purified_spans);
    }

    let (merged_clusters, merged_mapping) = repair_cluster_oversegmentation(
        clusters,
        &cluster_centroids,
        CLUSTER_OVER_SEGMENTATION_MERGE_THRESHOLD,
    );

    let mut refined_centroids = HashMap::new();
    let mut refined_purified_spans = HashMap::new();
    for cluster in &merged_clusters {
        if let Some(c) = cluster_centroids.get(&cluster.raw_speaker) {
            refined_centroids.insert(cluster.raw_speaker, c.clone());
        }
        let spans = if merged_mapping
            .values()
            .any(|&target| target == cluster.raw_speaker)
        {
            extract_purified_spans_for_cluster(cluster, &merged_clusters, segments)
        } else if let Some(cached) = initial_purified_spans.remove(&cluster.raw_speaker) {
            cached
        } else {
            extract_purified_spans_for_cluster(cluster, &merged_clusters, segments)
        };
        refined_purified_spans.insert(cluster.raw_speaker, spans);
    }

    Ok(RefinedClusterOutcome {
        clusters: merged_clusters,
        merged_speaker_map: merged_mapping,
        cluster_centroids: refined_centroids,
        purified_spans_by_cluster: refined_purified_spans,
    })
}

fn build_cluster_speaker_assignments(
    samples: &[f32],
    clusters: &[ClusterInfo],
    cluster_centroids: &HashMap<i32, Vec<f32>>,
    purified_spans_by_cluster: &HashMap<i32, Vec<SpeakerSpan>>,
    segments: &[TranscriptSegment],
    config: &SpeakerProcessingConfig,
    embedding_index: &crate::speaker::SpeakerEmbeddingIndex,
) -> Result<HashMap<i32, ResolvedSpeakerAssignment>, AsrPortError> {
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

    let mut loaded_profile_names = HashMap::new();
    let mut profile_readiness = HashMap::new();
    let mut profile_sample_embeddings: HashMap<String, Vec<ProfileSampleEmbedding>> =
        HashMap::new();

    for profile in enabled_profiles {
        let readiness = derive_profile_readiness(&profile);
        if readiness == SpeakerProfileReadinessState::NotReady {
            index_summary.skipped_profiles += 1;
            continue;
        }

        let mut samples_list = Vec::new();
        let mut raw_embeddings = Vec::new();
        for sample in &profile.samples {
            if sample.duration_seconds < PROFILE_SAMPLE_MIN_DURATION_SECONDS {
                continue;
            }
            if let Some(embedding) =
                embedding_index.compute_embedding_for_wav_file(&sample.file_path)?
            {
                samples_list.push(ProfileSampleEmbedding {
                    embedding: embedding.clone(),
                    duration_seconds: sample.duration_seconds,
                });
                raw_embeddings.push(embedding);
            }
        }

        if samples_list.is_empty() {
            index_summary.skipped_profiles += 1;
            continue;
        }

        embedding_index.add_profile_embeddings(&profile.id, &profile.name, &raw_embeddings)?;

        match readiness {
            SpeakerProfileReadinessState::Ready => index_summary.ready_profiles += 1,
            SpeakerProfileReadinessState::Limited => index_summary.limited_profiles += 1,
            SpeakerProfileReadinessState::NotReady => {}
        }
        index_summary.usable_sample_embeddings += raw_embeddings.len();
        loaded_profile_names.insert(profile.id.clone(), profile.name.clone());
        profile_readiness.insert(profile.id.clone(), readiness);
        profile_sample_embeddings.insert(profile.id.clone(), samples_list);
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
        let centroid = cluster_centroids
            .get(&cluster.raw_speaker)
            .map(|v| v.as_slice());
        let fallback_spans;
        let purified_spans = match purified_spans_by_cluster.get(&cluster.raw_speaker) {
            Some(spans) => spans.as_slice(),
            None => {
                fallback_spans = extract_purified_spans_for_cluster(cluster, clusters, segments);
                fallback_spans.as_slice()
            }
        };
        let cluster_candidates = identify_cluster_candidates(
            samples,
            cluster,
            centroid,
            purified_spans,
            embedding_index,
            &loaded_profile_names,
            &profile_sample_embeddings,
        )?;
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
    cluster_centroid: Option<&[f32]>,
    purified_spans: &[SpeakerSpan],
    embedding_index: &crate::speaker::SpeakerEmbeddingIndex,
    profile_names: &HashMap<String, String>,
    profile_sample_embeddings: &HashMap<String, Vec<ProfileSampleEmbedding>>,
) -> Result<Vec<ClusterCandidate>, AsrPortError> {
    let mut candidate_spans = purified_spans
        .iter()
        .filter(|span| (span.end - span.start) >= IDENTIFICATION_MIN_DURATION_SECONDS)
        .cloned()
        .collect::<Vec<_>>();

    if candidate_spans.is_empty() {
        candidate_spans = cluster
            .spans
            .iter()
            .filter(|span| (span.end - span.start) >= IDENTIFICATION_MIN_DURATION_SECONDS)
            .cloned()
            .collect::<Vec<_>>();
    }

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
            let avg_match_score = score_sums
                .get(profile_id.as_str())
                .copied()
                .unwrap_or_default()
                / votes as f32;

            let final_score = if let (Some(centroid), Some(samples)) = (
                cluster_centroid,
                profile_sample_embeddings.get(profile_id.as_str()),
            ) {
                let total_dur: f32 = samples.iter().map(|s| s.duration_seconds).sum();
                if total_dur > 0.0 {
                    let weighted_sim: f32 = samples
                        .iter()
                        .map(|s| s.duration_seconds * cosine_similarity(centroid, &s.embedding))
                        .sum::<f32>()
                        / total_dur;
                    let max_sim = samples
                        .iter()
                        .map(|s| cosine_similarity(centroid, &s.embedding))
                        .fold(f32::NEG_INFINITY, f32::max);
                    (0.6 * weighted_sim + 0.4 * max_sim).clamp(0.0, 1.0)
                } else {
                    avg_match_score
                }
            } else {
                avg_match_score
            };

            Some(ClusterCandidate {
                profile_id,
                profile_name,
                votes,
                average_score: final_score,
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

pub(crate) fn derive_profile_readiness(profile: &SpeakerProfile) -> SpeakerProfileReadinessState {
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
    let has_competition = candidates.len() > 1;
    let margin_satisfied = !has_competition || score_margin >= AUTO_IDENTIFICATION_MIN_MARGIN;

    if readiness == SpeakerProfileReadinessState::Ready
        && top_candidate.average_score >= AUTO_IDENTIFICATION_THRESHOLD
        && top_candidate.votes >= AUTO_IDENTIFICATION_MIN_VOTES
        && margin_satisfied
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

    if annotated.is_empty() && !segments.is_empty() {
        return segments.to_vec();
    }

    annotated.sort_by(|left, right| {
        left.start
            .partial_cmp(&right.start)
            .unwrap_or(Ordering::Equal)
            .then_with(|| left.end.partial_cmp(&right.end).unwrap_or(Ordering::Equal))
    });
    annotated
}

fn choose_speaker_for_timing_unit(
    unit: &TranscriptTimingUnit,
    spans: &[SpeakerSpan],
    speaker_assignments: &HashMap<i32, ResolvedSpeakerAssignment>,
    fallback: Option<&ResolvedSpeakerAssignment>,
) -> Option<ResolvedSpeakerAssignment> {
    let unit_start = unit.start as f32;
    let unit_end = unit.end as f32;
    let unit_dur = (unit_end - unit_start).max(0.001);

    let mut best_assignment = None;
    let mut best_ratio = 0.0_f32;
    let mut best_overlap = 0.0_f32;

    for span in spans {
        let overlap = range_overlap(unit_start, unit_end, span.start, span.end);
        let ratio = overlap / unit_dur;
        if ratio > best_ratio {
            best_ratio = ratio;
            best_overlap = overlap;
            if let Some(assignment) = speaker_assignments.get(&span.raw_speaker) {
                best_assignment = Some(assignment.clone());
            }
        }
    }

    if best_ratio >= 0.35 && best_assignment.is_some() {
        return best_assignment;
    }

    if best_overlap > 0.0 && best_assignment.is_some() {
        return best_assignment;
    }

    let midpoint = (unit_start + unit_end) / 2.0;
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
        .or_else(|| fallback.cloned())
}

fn smooth_speaker_glitches(
    units: &[TranscriptTimingUnit],
    token_speakers: &mut [Option<ResolvedSpeakerAssignment>],
) {
    if token_speakers.len() < 3 {
        return;
    }

    let n = token_speakers.len();
    let mut i = 0;
    while i < n {
        let Some(current_spk) = token_speakers[i].as_ref() else {
            i += 1;
            continue;
        };

        let run_start = i;
        while i + 1 < n
            && let Some(next_spk) = token_speakers[i + 1].as_ref()
            && speaker_assignments_equal(current_spk, next_spk)
        {
            i += 1;
        }
        let run_end = i + 1;
        let run_len = run_end - run_start;

        if run_start > 0
            && run_end < n
            && let (Some(left_spk), Some(right_spk)) = (
                token_speakers[run_start - 1].as_ref(),
                token_speakers[run_end].as_ref(),
            )
            && speaker_assignments_equal(left_spk, right_spk)
            && !speaker_assignments_equal(current_spk, left_spk)
        {
            let run_duration = units[run_end - 1].end - units[run_start].start;
            if run_duration < 0.40 && run_len <= 2 {
                let replacement = left_spk.clone();
                for slot in token_speakers.iter_mut().take(run_end).skip(run_start) {
                    *slot = Some(replacement.clone());
                }
            }
        }

        i += 1;
    }
}

fn rebuild_speaker_segments(
    segment: &TranscriptSegment,
    timing: &TranscriptTiming,
    groups: &[SplitGroup],
) -> Vec<TranscriptSegment> {
    let group_count = groups.len();
    let mut split_segments = Vec::with_capacity(group_count);

    for (group_idx, group) in groups.iter().enumerate() {
        let text = group.text.trim().to_string();
        if text.is_empty() {
            continue;
        }

        let timing_slice = timing.units[group.token_start..group.token_end_exclusive].to_vec();
        if timing_slice.is_empty() {
            continue;
        }

        let first_unit_start = timing_slice.first().unwrap().start;
        let last_unit_end = timing_slice.last().unwrap().end;

        let start = if group_idx == 0 {
            segment.start.min(first_unit_start)
        } else {
            let prev_group = &groups[group_idx - 1];
            let prev_last_unit_end = timing.units[prev_group.token_end_exclusive - 1].end;
            if first_unit_start > prev_last_unit_end {
                (prev_last_unit_end + first_unit_start) / 2.0
            } else {
                first_unit_start
            }
        };

        let end = if group_idx + 1 == group_count {
            segment.end.max(last_unit_end).max(start)
        } else {
            let next_group = &groups[group_idx + 1];
            let next_first_unit_start = timing.units[next_group.token_start].start;
            if next_first_unit_start > last_unit_end {
                ((last_unit_end + next_first_unit_start) / 2.0).max(start)
            } else {
                last_unit_end.max(start)
            }
        };

        let mut next_segment = TranscriptSegment {
            id: if split_segments.is_empty() {
                segment.id.clone()
            } else {
                uuid::Uuid::new_v4().to_string()
            },
            text,
            start,
            end,
            is_final: segment.is_final,
            timing: Some(TranscriptTiming {
                level: TranscriptTimingLevel::Token,
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
        split_segments.push(next_segment);
    }

    split_segments
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

    let mut effective_segment = segment.clone();
    sona_core::transcription::forced_alignment::apply_fallback_timing_to_transcript_segment(
        &mut effective_segment,
    );
    let Some(timing) = effective_segment
        .timing
        .as_ref()
        .filter(|t| !t.units.is_empty())
    else {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    };

    let mut token_speakers: Vec<Option<ResolvedSpeakerAssignment>> = timing
        .units
        .iter()
        .map(|unit| {
            choose_speaker_for_timing_unit(
                unit,
                spans,
                speaker_assignments,
                fallback_speaker.as_ref(),
            )
        })
        .collect();

    smooth_speaker_glitches(&timing.units, &mut token_speakers);

    let aligned_units = timing
        .units
        .iter()
        .enumerate()
        .map(|(index, unit)| AlignedTextUnit {
            text: unit.text.clone(),
            token_index: index,
            token_end_exclusive: index + 1,
        })
        .collect::<Vec<_>>();

    let Some(groups) = build_split_groups(&aligned_units, &token_speakers) else {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    };

    if groups.is_empty() {
        return vec![apply_speaker_to_whole_segment(segment, fallback_speaker)];
    }

    if groups.len() == 1 {
        return vec![apply_assignment_to_segment(
            &effective_segment,
            &groups[0].assignment,
        )];
    }

    let split_segments = rebuild_speaker_segments(segment, timing, &groups);
    if !split_segments.is_empty() {
        split_segments
    } else {
        vec![apply_speaker_to_whole_segment(segment, fallback_speaker)]
    }
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
            if should_insert_space_between(&current.text, &unit.text) {
                current.text.push(' ');
            }
            current.text.push_str(&unit.text);
            current.token_end_exclusive = current.token_end_exclusive.max(unit.token_end_exclusive);
            continue;
        }

        groups.push(SplitGroup {
            assignment,
            text: unit.text.clone(),
            token_start: unit.token_index,
            token_end_exclusive: unit.token_end_exclusive,
        });
    }

    Some(groups)
}
fn should_insert_space_between(prev_text: &str, next_text: &str) -> bool {
    let Some(last_char) = prev_text.chars().last() else {
        return false;
    };
    let Some(first_char) = next_text.chars().next() else {
        return false;
    };
    if last_char.is_whitespace() || first_char.is_whitespace() {
        return false;
    }
    // No space if either character is CJK
    if sona_core::transcription::text_alignment::is_cjk_char(last_char)
        || sona_core::transcription::text_alignment::is_cjk_char(first_char)
    {
        return false;
    }
    // No space before punctuation like ',', '.', '!', '?', etc.
    if matches!(
        first_char,
        '.' | ',' | '!' | '?' | ':' | ';' | '\'' | ')' | ']' | '}'
    ) {
        return false;
    }
    // No space after opening brackets or quotes
    if matches!(last_char, '(' | '[' | '{' | '\'') {
        return false;
    }
    true
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
    use sona_core::ports::asr::AsrPortErrorKind;
    use sona_core::transcription::text_alignment::lex_text_units;
    use sona_core::transcription::transcript::TranscriptTimingSource;

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
    fn configured_speaker_processing_requires_model_paths() {
        let segments = vec![sample_segment(0.0, 1.0, "hello")];
        let config = SpeakerProcessingConfig {
            speaker_segmentation_model_path: None,
            speaker_embedding_model_path: None,
            speaker_profiles: None,
        };

        let error = annotate_segments_with_speakers(&[], &segments, Some(&config)).unwrap_err();

        assert_eq!(error.kind, AsrPortErrorKind::InvalidRequest);
        assert!(error.message.contains("not fully configured"));
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
            source: sona_core::transcription::transcript::TranscriptTimingSource::Model,
            units: vec![
                sona_core::transcription::transcript::TranscriptTimingUnit {
                    text: "Hello".to_string(),
                    start: 0.0,
                    end: 1.0,
                },
                sona_core::transcription::transcript::TranscriptTimingUnit {
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

    #[test]
    fn apply_speaker_tags_to_segments_never_drops_original_segments() {
        let segment = sample_segment(0.0, 1.0, "preserved text");
        let clusters = Vec::new();
        let assignments = HashMap::new();

        let annotated = apply_speaker_tags_to_segments(&[segment], &clusters, &assignments);
        assert_eq!(annotated.len(), 1);
        assert_eq!(annotated[0].text, "preserved text");
    }

    #[test]
    fn anti_glitch_smoothing_eliminates_short_speaker_noise_blips() {
        let mut segment = sample_segment(0.0, 3.0, "Welcome to the world");
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: sona_core::transcription::transcript::TranscriptTimingSource::Model,
            units: vec![
                TranscriptTimingUnit {
                    text: "Welcome".to_string(),
                    start: 0.0,
                    end: 1.0,
                },
                TranscriptTimingUnit {
                    text: " to".to_string(),
                    start: 1.0,
                    end: 1.15,
                },
                TranscriptTimingUnit {
                    text: " the".to_string(),
                    start: 1.15,
                    end: 1.30,
                },
                TranscriptTimingUnit {
                    text: " world".to_string(),
                    start: 1.30,
                    end: 3.0,
                },
            ],
        });

        // Speaker 2 has an acoustic blip for only 0.3s (1.0..1.30)
        let spans = vec![
            SpeakerSpan {
                start: 0.0,
                end: 1.0,
                raw_speaker: 1,
            },
            SpeakerSpan {
                start: 1.0,
                end: 1.30,
                raw_speaker: 2,
            },
            SpeakerSpan {
                start: 1.30,
                end: 3.0,
                raw_speaker: 1,
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

        // Glitch is absorbed: returns 1 segment assigned to Alice instead of splitting
        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0].speaker.as_ref().map(|value| value.label.as_str()),
            Some("Alice")
        );
        assert_eq!(
            result[0].timing.as_ref().map(|timing| timing.units.len()),
            Some(4)
        );
    }

    #[test]
    fn natural_resegmentation_snaps_boundaries_at_gap_midpoint() {
        let mut segment = sample_segment(0.0, 3.0, "Hello World");
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: sona_core::transcription::transcript::TranscriptTimingSource::Model,
            units: vec![
                TranscriptTimingUnit {
                    text: "Hello".to_string(),
                    start: 0.0,
                    end: 1.0,
                },
                TranscriptTimingUnit {
                    text: " World".to_string(),
                    start: 1.4, // 0.4s gap between 1.0 and 1.4
                    end: 3.0,
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
                start: 1.4,
                end: 3.0,
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
        // First segment starts at 0.0, ends at midpoint 1.2s
        assert!((result[0].start - 0.0).abs() < 1e-4);
        assert!((result[0].end - 1.2).abs() < 1e-4);
        assert_eq!(
            result[0].speaker.as_ref().map(|s| s.label.as_str()),
            Some("Alice")
        );

        // Second segment starts at midpoint 1.2s, ends at 3.0s
        assert!((result[1].start - 1.2).abs() < 1e-4);
        assert!((result[1].end - 3.0).abs() < 1e-4);
        assert_eq!(
            result[1].speaker.as_ref().map(|s| s.label.as_str()),
            Some("Bob")
        );
    }

    #[test]
    fn test_cosine_similarity_basic() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &b) - 1.0).abs() < 1e-5);

        let c = vec![0.0, 1.0, 0.0];
        assert!((cosine_similarity(&a, &c) - 0.0).abs() < 1e-5);

        let d = vec![-1.0, 0.0, 0.0];
        assert!((cosine_similarity(&a, &d) - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn test_build_split_groups_inserts_spaces_for_non_cjk_words() {
        let aligned_units = vec![
            AlignedTextUnit {
                text: "hello".to_string(),
                token_index: 0,
                token_end_exclusive: 1,
            },
            AlignedTextUnit {
                text: "world".to_string(),
                token_index: 1,
                token_end_exclusive: 2,
            },
        ];
        let assignment = Some(ResolvedSpeakerAssignment {
            raw_speaker: 1,
            speaker: Some(SpeakerTag {
                id: "speaker-1".to_string(),
                label: "Speaker 1".to_string(),
                kind: "identified".to_string(),
                score: None,
            }),
            attribution: SpeakerAttribution {
                group_id: "anonymous-1".to_string(),
                anonymous_label: "Speaker 1".to_string(),
                state: "identified".to_string(),
                source: "auto".to_string(),
                confidence: "high".to_string(),
                candidates: Vec::new(),
            },
            average_score: Some(0.95),
            votes: 1,
        });
        let token_speakers = vec![assignment.clone(), assignment];
        let groups = build_split_groups(&aligned_units, &token_speakers).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].text, "hello world");
    }
    #[test]
    fn test_repair_cluster_oversegmentation_merges_high_similarity() {
        let clusters = vec![
            ClusterInfo {
                raw_speaker: 1,
                spans: vec![SpeakerSpan {
                    start: 0.0,
                    end: 2.0,
                    raw_speaker: 1,
                }],
                anonymous_tag: SpeakerTag {
                    id: "anonymous-1".to_string(),
                    label: "Speaker 1".to_string(),
                    kind: "anonymous".to_string(),
                    score: None,
                },
            },
            ClusterInfo {
                raw_speaker: 2,
                spans: vec![SpeakerSpan {
                    start: 3.0,
                    end: 5.0,
                    raw_speaker: 2,
                }],
                anonymous_tag: SpeakerTag {
                    id: "anonymous-2".to_string(),
                    label: "Speaker 2".to_string(),
                    kind: "anonymous".to_string(),
                    score: None,
                },
            },
        ];

        let centroids = HashMap::from([
            (1, vec![0.8, 0.6]),
            (2, vec![0.81, 0.59]), // Cosine sim ~ 0.999
        ]);

        let (merged, mapping) = repair_cluster_oversegmentation(clusters, &centroids, 0.85);
        assert_eq!(merged.len(), 1);
        assert_eq!(mapping.get(&2), Some(&1));
        assert_eq!(merged[0].spans.len(), 2);
        assert_eq!(merged[0].spans[0].raw_speaker, 1);
        assert_eq!(merged[0].spans[1].raw_speaker, 1);
        assert_eq!(merged[0].anonymous_tag.label, "Speaker 1");
    }

    #[test]
    fn test_extract_purified_spans_for_cluster_trims_silence() {
        let cluster = ClusterInfo {
            raw_speaker: 1,
            spans: vec![SpeakerSpan {
                start: 0.0,
                end: 5.0,
                raw_speaker: 1,
            }],
            anonymous_tag: SpeakerTag {
                id: "anonymous-1".to_string(),
                label: "Speaker 1".to_string(),
                kind: "anonymous".to_string(),
                score: None,
            },
        };

        let mut segment = sample_segment(1.0, 3.5, "Some spoken words");
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: TranscriptTimingSource::Model,
            units: vec![
                TranscriptTimingUnit {
                    text: "Some".to_string(),
                    start: 1.0,
                    end: 1.8,
                },
                TranscriptTimingUnit {
                    text: "spoken".to_string(),
                    start: 1.9,
                    end: 2.7,
                },
                TranscriptTimingUnit {
                    text: "words".to_string(),
                    start: 2.8,
                    end: 3.5,
                },
            ],
        });

        let purified = extract_purified_spans_for_cluster(&cluster, &[], &[segment]);
        assert_eq!(purified.len(), 1);
        // 1.0 to 3.5 is duration 2.5s >= 1.2s -> insets 0.04 applied: 1.04 to 3.46
        assert!((purified[0].start - 1.04).abs() < 1e-3);
        assert!((purified[0].end - 3.46).abs() < 1e-3);
    }

    #[test]
    fn test_competitor_margin_downgrades_to_suggested_on_close_scores() {
        let cluster = ClusterInfo {
            raw_speaker: 1,
            spans: vec![SpeakerSpan {
                start: 0.0,
                end: 3.0,
                raw_speaker: 1,
            }],
            anonymous_tag: SpeakerTag {
                id: "anonymous-1".to_string(),
                label: "Speaker 1".to_string(),
                kind: "anonymous".to_string(),
                score: None,
            },
        };

        let candidates = vec![
            ClusterCandidate {
                profile_id: "profile-alice".to_string(),
                profile_name: "Alice".to_string(),
                votes: 3,
                average_score: 0.75,
            },
            ClusterCandidate {
                profile_id: "profile-bob".to_string(),
                profile_name: "Bob".to_string(),
                votes: 3,
                average_score: 0.73, // Margin is 0.02 < 0.08
            },
        ];

        let readiness = HashMap::from([
            (
                "profile-alice".to_string(),
                SpeakerProfileReadinessState::Ready,
            ),
            (
                "profile-bob".to_string(),
                SpeakerProfileReadinessState::Ready,
            ),
        ]);

        let assignment = resolve_single_cluster_assignment(&cluster, candidates, &readiness);
        // Due to close competition, state must be "suggested" rather than "identified"
        assert_eq!(assignment.attribution.state, "suggested");
        assert_eq!(assignment.attribution.candidates.len(), 2);
        assert_eq!(assignment.attribution.candidates[0].profile_name, "Alice");
        assert_eq!(assignment.attribution.candidates[1].profile_name, "Bob");
    }
}
