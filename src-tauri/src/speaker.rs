use crate::pipeline;
use crate::sherpa::{
    ensure_transcript_segment_timing, TranscriptSegment, TranscriptTiming, TranscriptTimingLevel,
};
use serde::{Deserialize, Serialize};
use sherpa_onnx::{
    FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
    OfflineSpeakerDiarizationSegment, OfflineSpeakerSegmentationModelConfig,
    OfflineSpeakerSegmentationPyannoteModelConfig, SpeakerEmbeddingExtractor,
    SpeakerEmbeddingExtractorConfig, SpeakerEmbeddingManager,
};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, Runtime};

const SAMPLE_RATE: i32 = 16_000;
const IDENTIFICATION_MIN_DURATION_SECONDS: f32 = 1.5;
const IDENTIFICATION_MAX_SEGMENTS_PER_CLUSTER: usize = 3;
const IDENTIFICATION_THRESHOLD: f32 = 0.6;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerTag {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub score: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProfileSample {
    pub id: String,
    pub file_path: String,
    pub source_name: String,
    pub duration_seconds: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProfile {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub samples: Vec<SpeakerProfileSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerProcessingConfig {
    pub speaker_segmentation_model_path: Option<String>,
    pub speaker_embedding_model_path: Option<String>,
    pub speaker_profiles: Option<Vec<SpeakerProfile>>,
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
    raw_speaker: i32,
    profile_id: String,
    profile_name: String,
    votes: usize,
    average_score: f32,
}

#[derive(Debug, Clone)]
struct TextUnit {
    text: String,
    normalized: String,
}

#[derive(Debug, Clone)]
struct AlignedTextUnit {
    text: String,
    token_index: usize,
}

#[derive(Debug, Clone)]
struct SplitGroup {
    speaker: SpeakerTag,
    text: String,
    token_start: usize,
    token_end_exclusive: usize,
}

#[tauri::command]
pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<TranscriptSegment>,
    speaker_processing: Option<SpeakerProcessingConfig>,
) -> Result<Vec<TranscriptSegment>, String> {
    if segments.is_empty() {
        return Ok(segments);
    }

    let samples = pipeline::extract_and_resample_audio(&file_path, SAMPLE_RATE as u32).await?;
    annotate_segments_with_speakers(&samples, &segments, speaker_processing.as_ref())
}

#[tauri::command]
pub async fn import_speaker_profile_sample<R: Runtime>(
    app: AppHandle<R>,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<SpeakerProfileSample, String> {
    let samples = pipeline::extract_and_resample_audio(&source_path, SAMPLE_RATE as u32).await?;
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

    let app_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let profile_dir = app_data_dir.join("speaker-profiles").join(&profile_id);
    std::fs::create_dir_all(&profile_dir).map_err(|e| e.to_string())?;

    let output_path = profile_dir.join(format!("{sample_id}.wav"));
    pipeline::save_wav_file(&samples, SAMPLE_RATE as u32, &output_path.to_string_lossy())
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
    let Some(config) = speaker_processing else {
        return Ok(segments.to_vec());
    };

    if segments.is_empty() {
        return Ok(Vec::new());
    }

    let segmentation_model = resolve_model_path(config.speaker_segmentation_model_path.as_deref())?;
    let embedding_model = resolve_model_path(config.speaker_embedding_model_path.as_deref())?;
    let diarization_segments = run_diarization(samples, &segmentation_model, &embedding_model)?;

    if diarization_segments.is_empty() {
        return Ok(segments.to_vec());
    }

    let clusters = build_cluster_infos(&diarization_segments);
    let speaker_tags = build_cluster_speaker_tags(samples, &clusters, config, &embedding_model)?;
    Ok(apply_speaker_tags_to_segments(segments, &clusters, &speaker_tags))
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

fn run_diarization(
    samples: &[f32],
    segmentation_model: &Path,
    embedding_model: &Path,
) -> Result<Vec<OfflineSpeakerDiarizationSegment>, String> {
    let diarization_config = OfflineSpeakerDiarizationConfig {
        segmentation: OfflineSpeakerSegmentationModelConfig {
            pyannote: OfflineSpeakerSegmentationPyannoteModelConfig {
                model: Some(segmentation_model.to_string_lossy().into_owned()),
            },
            num_threads: 1,
            debug: false,
            provider: Some("cpu".to_string()),
        },
        embedding: SpeakerEmbeddingExtractorConfig {
            model: Some(embedding_model.to_string_lossy().into_owned()),
            num_threads: 1,
            debug: false,
            provider: Some("cpu".to_string()),
        },
        clustering: FastClusteringConfig {
            num_clusters: -1,
            ..Default::default()
        },
        ..Default::default()
    };

    let diarizer = OfflineSpeakerDiarization::create(&diarization_config)
        .ok_or_else(|| "Failed to create offline speaker diarizer".to_string())?;
    let result = diarizer
        .process(samples)
        .ok_or_else(|| "Speaker diarization returned no result".to_string())?;
    Ok(result.sort_by_start_time())
}

fn build_cluster_infos(diarization_segments: &[OfflineSpeakerDiarizationSegment]) -> Vec<ClusterInfo> {
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
            spans.sort_by(|left, right| left.start.partial_cmp(&right.start).unwrap_or(Ordering::Equal));
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

fn build_cluster_speaker_tags(
    samples: &[f32],
    clusters: &[ClusterInfo],
    config: &SpeakerProcessingConfig,
    embedding_model: &Path,
) -> Result<HashMap<i32, SpeakerTag>, String> {
    let mut tag_map = clusters
        .iter()
        .map(|cluster| (cluster.raw_speaker, cluster.anonymous_tag.clone()))
        .collect::<HashMap<_, _>>();

    let enabled_profiles = config
        .speaker_profiles
        .clone()
        .unwrap_or_default()
        .into_iter()
        .filter(|profile| profile.enabled && !profile.samples.is_empty())
        .collect::<Vec<_>>();

    if enabled_profiles.is_empty() {
        return Ok(tag_map);
    }

    let extractor = SpeakerEmbeddingExtractor::create(&SpeakerEmbeddingExtractorConfig {
        model: Some(embedding_model.to_string_lossy().into_owned()),
        num_threads: 1,
        debug: false,
        provider: Some("cpu".to_string()),
    })
    .ok_or_else(|| "Failed to create speaker embedding extractor".to_string())?;

    let manager = SpeakerEmbeddingManager::create(extractor.dim())
        .ok_or_else(|| "Failed to create speaker embedding manager".to_string())?;

    let mut loaded_profile_names = HashMap::new();
    for profile in enabled_profiles {
        let mut embeddings = Vec::new();
        for sample in &profile.samples {
            if let Some(embedding) = compute_embedding_for_file(&extractor, &sample.file_path)? {
                embeddings.push(embedding);
            }
        }

        if embeddings.is_empty() {
            continue;
        }

        if !manager.add_list(&profile.id, &embeddings) {
            return Err(format!("Failed to index speaker profile {}", profile.name));
        }

        loaded_profile_names.insert(profile.id.clone(), profile.name.clone());
    }

    if loaded_profile_names.is_empty() {
        return Ok(tag_map);
    }

    let mut candidates = Vec::new();
    for cluster in clusters {
        if let Some(candidate) = identify_cluster_candidate(
            samples,
            cluster,
            &extractor,
            &manager,
            &loaded_profile_names,
        )? {
            candidates.push(candidate);
        }
    }

    for candidate in resolve_cluster_candidates(candidates).into_values() {
        tag_map.insert(
            candidate.raw_speaker,
            SpeakerTag {
                id: candidate.profile_id,
                label: candidate.profile_name,
                kind: "identified".to_string(),
                score: Some(candidate.average_score),
            },
        );
    }

    Ok(tag_map)
}

fn compute_embedding_for_file(
    extractor: &SpeakerEmbeddingExtractor,
    file_path: &str,
) -> Result<Option<Vec<f32>>, String> {
    let samples = load_profile_sample_wav(file_path)?;
    compute_embedding_for_samples(extractor, &samples)
}

fn compute_embedding_for_samples(
    extractor: &SpeakerEmbeddingExtractor,
    samples: &[f32],
) -> Result<Option<Vec<f32>>, String> {
    if samples.is_empty() {
        return Ok(None);
    }

    let stream = extractor
        .create_stream()
        .ok_or_else(|| "Failed to create speaker embedding stream".to_string())?;
    stream.accept_waveform(SAMPLE_RATE, samples);
    stream.input_finished();

    if !extractor.is_ready(&stream) {
        return Ok(None);
    }

    Ok(extractor.compute(&stream))
}

fn identify_cluster_candidate(
    samples: &[f32],
    cluster: &ClusterInfo,
    extractor: &SpeakerEmbeddingExtractor,
    manager: &SpeakerEmbeddingManager,
    profile_names: &HashMap<String, String>,
) -> Result<Option<ClusterCandidate>, String> {
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
        return Ok(None);
    }

    let mut vote_counts: HashMap<String, usize> = HashMap::new();
    let mut score_sums: HashMap<String, f32> = HashMap::new();

    for span in candidate_spans {
        let Some(embedding) = compute_embedding_for_span(extractor, samples, &span)? else {
            continue;
        };
        let Some(best_match) = manager
            .get_best_matches(&embedding, IDENTIFICATION_THRESHOLD, 1)
            .into_iter()
            .next()
        else {
            continue;
        };

        *vote_counts.entry(best_match.name.clone()).or_insert(0) += 1;
        *score_sums.entry(best_match.name).or_insert(0.0) += best_match.score;
    }

    let Some((profile_id, votes)) = vote_counts
        .iter()
        .max_by(|left, right| {
            let left_avg = score_sums.get(left.0.as_str()).copied().unwrap_or_default() / *left.1 as f32;
            let right_avg = score_sums.get(right.0.as_str()).copied().unwrap_or_default() / *right.1 as f32;

            left.1
                .cmp(right.1)
                .then_with(|| left_avg.partial_cmp(&right_avg).unwrap_or(Ordering::Equal))
        })
        .map(|(profile_id, votes)| (profile_id.clone(), *votes))
    else {
        return Ok(None);
    };

    let total_score = score_sums.get(profile_id.as_str()).copied().unwrap_or_default();
    let average_score = total_score / votes as f32;
    let Some(profile_name) = profile_names.get(profile_id.as_str()).cloned() else {
        return Ok(None);
    };

    Ok(Some(ClusterCandidate {
        raw_speaker: cluster.raw_speaker,
        profile_id,
        profile_name,
        votes,
        average_score,
    }))
}

fn compute_embedding_for_span(
    extractor: &SpeakerEmbeddingExtractor,
    samples: &[f32],
    span: &SpeakerSpan,
) -> Result<Option<Vec<f32>>, String> {
    let start_index = ((span.start.max(0.0)) * SAMPLE_RATE as f32).floor() as usize;
    let end_index = ((span.end.max(span.start)) * SAMPLE_RATE as f32).ceil() as usize;
    if start_index >= samples.len() || end_index <= start_index {
        return Ok(None);
    }

    let bounded_end = end_index.min(samples.len());
    compute_embedding_for_samples(extractor, &samples[start_index..bounded_end])
}

fn load_profile_sample_wav(file_path: &str) -> Result<Vec<f32>, String> {
    let mut reader = hound::WavReader::open(file_path).map_err(|e| e.to_string())?;
    let spec = reader.spec();
    if spec.channels != 1 {
        return Err(format!("Speaker sample must be mono wav: {file_path}"));
    }
    if spec.sample_rate != SAMPLE_RATE as u32 {
        return Err(format!(
            "Speaker sample must be 16k wav but got {} Hz: {file_path}",
            spec.sample_rate
        ));
    }

    match spec.sample_format {
        hound::SampleFormat::Int => {
            if spec.bits_per_sample != 16 {
                return Err(format!(
                    "Speaker sample must be 16-bit PCM wav: {file_path}"
                ));
            }
            reader
                .samples::<i16>()
                .map(|sample| sample.map(|value| value as f32 / i16::MAX as f32).map_err(|e| e.to_string()))
                .collect()
        }
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| sample.map_err(|e| e.to_string()))
            .collect(),
    }
}

fn resolve_cluster_candidates(candidates: Vec<ClusterCandidate>) -> HashMap<i32, ClusterCandidate> {
    let mut sorted = candidates;
    sorted.sort_by(|left, right| {
        right
            .average_score
            .partial_cmp(&left.average_score)
            .unwrap_or(Ordering::Equal)
            .then_with(|| right.votes.cmp(&left.votes))
    });

    let mut selected_by_profile = HashMap::new();
    let mut selected_by_cluster = HashMap::new();

    for candidate in sorted {
        if selected_by_profile.contains_key(candidate.profile_id.as_str()) {
            continue;
        }

        selected_by_profile.insert(candidate.profile_id.clone(), candidate.raw_speaker);
        selected_by_cluster.insert(candidate.raw_speaker, candidate);
    }

    selected_by_cluster
}

fn apply_speaker_tags_to_segments(
    segments: &[TranscriptSegment],
    clusters: &[ClusterInfo],
    speaker_tags: &HashMap<i32, SpeakerTag>,
) -> Vec<TranscriptSegment> {
    let spans = clusters
        .iter()
        .flat_map(|cluster| cluster.spans.iter().cloned())
        .collect::<Vec<_>>();

    let mut annotated = Vec::new();
    for segment in segments {
        annotated.extend(assign_speakers_to_segment(segment, &spans, speaker_tags));
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
    speaker_tags: &HashMap<i32, SpeakerTag>,
) -> Vec<TranscriptSegment> {
    let fallback_speaker = choose_speaker_for_range(segment.start as f32, segment.end as f32, spans, speaker_tags);

    if let Some(timing) = segment
        .timing
        .as_ref()
        .filter(|timing| timing.level == TranscriptTimingLevel::Token && !timing.units.is_empty())
    {
        let token_speakers = timing
            .units
            .iter()
            .map(|unit| {
                choose_speaker_for_range(unit.start as f32, unit.end as f32, spans, speaker_tags)
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
                    return vec![TranscriptSegment {
                        speaker: Some(groups[0].speaker.clone()),
                        ..segment.clone()
                    }];
                }

                let segments = groups
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, group)| {
                        let text = group.text.trim().to_string();
                        if text.is_empty() {
                            return None;
                        }

                        let timing_slice = timing.units[group.token_start..group.token_end_exclusive].to_vec();
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
                            speaker: Some(group.speaker),
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
            choose_speaker_for_range(start, end, spans, speaker_tags).or_else(|| fallback_speaker.clone())
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
        return vec![TranscriptSegment {
            speaker: Some(groups[0].speaker.clone()),
            ..segment.clone()
        }];
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
            let duration_slice = durations.map(|values| values[group.token_start..group.token_end_exclusive].to_vec());

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
                speaker: Some(group.speaker),
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
    speaker: Option<SpeakerTag>,
) -> TranscriptSegment {
    TranscriptSegment {
        speaker,
        ..segment.clone()
    }
}

fn build_split_groups(
    aligned_units: &[AlignedTextUnit],
    token_speakers: &[Option<SpeakerTag>],
) -> Option<Vec<SplitGroup>> {
    let mut groups: Vec<SplitGroup> = Vec::new();

    for unit in aligned_units {
        let speaker = token_speakers.get(unit.token_index)?.clone()?;
        if let Some(current) = groups.last_mut() {
            if speaker_tags_equal(&current.speaker, &speaker) {
                current.text.push_str(&unit.text);
                current.token_end_exclusive = current.token_end_exclusive.max(unit.token_index + 1);
                continue;
            }
        }

        groups.push(SplitGroup {
            speaker,
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
    speaker_tags: &HashMap<i32, SpeakerTag>,
) -> Option<SpeakerTag> {
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
        return speaker_tags.get(&span.raw_speaker).cloned();
    }

    let midpoint = (start + end) / 2.0;
    spans.iter()
        .min_by(|left, right| {
            let left_distance = distance_to_range(midpoint, left.start, left.end);
            let right_distance = distance_to_range(midpoint, right.start, right.end);
            left_distance
                .partial_cmp(&right_distance)
                .unwrap_or(Ordering::Equal)
        })
        .and_then(|span| speaker_tags.get(&span.raw_speaker).cloned())
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

fn align_text_units_to_tokens(text: &str, tokens: &[String]) -> Option<Vec<AlignedTextUnit>> {
    if tokens.is_empty() {
        return None;
    }

    let normalized_tokens = tokens
        .iter()
        .map(|token| normalize_search_text(token))
        .collect::<Vec<_>>();
    let units = lex_text_units(text);

    let mut joined_token_chars = Vec::new();
    let mut char_to_token_index = Vec::new();
    for (token_index, token) in normalized_tokens.iter().enumerate() {
        for ch in token.chars() {
            joined_token_chars.push(ch);
            char_to_token_index.push(token_index);
        }
    }

    if char_to_token_index.is_empty() {
        return None;
    }

    let mut char_pos = 0usize;
    let mut result = Vec::new();

    for unit in units {
        if unit.text.is_empty() {
            continue;
        }

        let token_index = if unit.normalized.is_empty() {
            fallback_token_index(char_pos, &char_to_token_index)
        } else {
            let needle = unit.normalized.chars().collect::<Vec<_>>();
            let search_limit = needle.len().saturating_mul(2).max(20);
            let window_end = (char_pos + search_limit).min(joined_token_chars.len());
            let local_index = find_subsequence(&joined_token_chars[char_pos..window_end], &needle);

            if let Some(local_index) = local_index {
                let match_pos = char_pos + local_index;
                char_pos = (match_pos + needle.len()).min(joined_token_chars.len());
                fallback_token_index(match_pos, &char_to_token_index)
            } else {
                let fallback = fallback_token_index(char_pos, &char_to_token_index);
                char_pos = (char_pos + needle.len().max(1)).min(joined_token_chars.len());
                fallback
            }
        };

        result.push(AlignedTextUnit {
            text: unit.text,
            token_index,
        });
    }

    Some(result)
}

fn fallback_token_index(char_pos: usize, char_to_token_index: &[usize]) -> usize {
    if char_to_token_index.is_empty() {
        return 0;
    }

    if char_pos >= char_to_token_index.len() {
        char_to_token_index[char_to_token_index.len() - 1]
    } else {
        char_to_token_index[char_pos]
    }
}

fn find_subsequence(haystack: &[char], needle: &[char]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    if needle.len() > haystack.len() {
        return None;
    }

    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

fn lex_text_units(text: &str) -> Vec<TextUnit> {
    let mut units = Vec::new();
    let chars = text.chars().collect::<Vec<_>>();
    let mut index = 0usize;

    while index < chars.len() {
        let ch = chars[index];

        if ch.is_whitespace() {
            let start = index;
            index += 1;
            while index < chars.len() && chars[index].is_whitespace() {
                index += 1;
            }
            units.push(TextUnit {
                text: chars[start..index].iter().collect(),
                normalized: String::new(),
            });
            continue;
        }

        if is_cjk_char(ch) {
            let text = ch.to_string();
            units.push(TextUnit {
                normalized: normalize_search_text(&text),
                text,
            });
            index += 1;
            continue;
        }

        let start = index;
        index += 1;
        while index < chars.len() && !chars[index].is_whitespace() && !is_cjk_char(chars[index]) {
            index += 1;
        }

        let text = chars[start..index].iter().collect::<String>();
        let normalized = normalize_search_text(&text);

        if normalized.is_empty() {
            if let Some(previous) = units.last_mut() {
                if !previous.normalized.is_empty() {
                    previous.text.push_str(&text);
                    continue;
                }
            }
        }

        units.push(TextUnit { text, normalized });
    }

    units
}

fn normalize_search_text(text: &str) -> String {
    text.chars()
        .flat_map(|ch| ch.to_lowercase())
        .filter(|ch| ch.is_alphanumeric())
        .collect()
}

fn is_cjk_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{3040}'..='\u{309F}'
            | '\u{30A0}'..='\u{30FF}'
            | '\u{AC00}'..='\u{D7AF}'
    )
}

fn speaker_tags_equal(left: &SpeakerTag, right: &SpeakerTag) -> bool {
    left.id == right.id && left.label == right.label && left.kind == right.kind
}

#[cfg(test)]
mod tests {
    use super::*;

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
        }
    }

    #[test]
    fn build_cluster_infos_orders_anonymous_labels_by_first_start_time() {
        let diarization_segments = vec![
            OfflineSpeakerDiarizationSegment {
                start: 6.0,
                end: 8.0,
                speaker: 10,
            },
            OfflineSpeakerDiarizationSegment {
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
        let resolved = resolve_cluster_candidates(vec![
            ClusterCandidate {
                raw_speaker: 1,
                profile_id: "alice".to_string(),
                profile_name: "Alice".to_string(),
                votes: 2,
                average_score: 0.91,
            },
            ClusterCandidate {
                raw_speaker: 2,
                profile_id: "alice".to_string(),
                profile_name: "Alice".to_string(),
                votes: 3,
                average_score: 0.77,
            },
        ]);

        assert!(resolved.contains_key(&1));
        assert!(!resolved.contains_key(&2));
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
            (1, speaker("anonymous-1", "Speaker 1", "anonymous", None)),
            (2, speaker("profile-bob", "Bob", "identified", Some(0.82))),
        ]);

        let result = assign_speakers_to_segment(&segment, &spans, &tags);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0].speaker.as_ref().map(|value| value.label.as_str()), Some("Bob"));
    }

    #[test]
    fn token_level_timing_allows_speaker_split_groups() {
        let mut segment = sample_segment(0.0, 2.0, "Hello there");
        segment.timing = Some(TranscriptTiming {
            level: TranscriptTimingLevel::Token,
            source: crate::sherpa::TranscriptTimingSource::Model,
            units: vec![
                crate::sherpa::TranscriptTimingUnit {
                    text: "Hello".to_string(),
                    start: 0.0,
                    end: 1.0,
                },
                crate::sherpa::TranscriptTimingUnit {
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
            (1, speaker("speaker-1", "Alice", "identified", Some(0.9))),
            (2, speaker("speaker-2", "Bob", "identified", Some(0.85))),
        ]);

        let result = assign_speakers_to_segment(&segment, &spans, &tags);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].speaker.as_ref().map(|value| value.label.as_str()), Some("Alice"));
        assert_eq!(result[1].speaker.as_ref().map(|value| value.label.as_str()), Some("Bob"));
        assert_eq!(result[0].timing.as_ref().map(|timing| timing.units.len()), Some(1));
        assert_eq!(result[1].timing.as_ref().map(|timing| timing.units.len()), Some(1));
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
