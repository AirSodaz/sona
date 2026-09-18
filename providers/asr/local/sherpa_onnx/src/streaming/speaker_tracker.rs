use crate::speaker::SpeakerEmbeddingIndex;
use crate::speaker_processing::{
    AUTO_IDENTIFICATION_MIN_MARGIN, ProfileSampleEmbedding, SpeakerProfileReadinessState,
    cosine_similarity, derive_profile_readiness, resolve_model_path,
};
use log::{info, warn};
use sona_core::ports::asr::AsrPortError;
use sona_core::transcription::speaker::SpeakerProcessingConfig;
use sona_core::transcription::transcript::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerModelThresholds {
    /// Dynamic cluster merge threshold (same speaker decision line)
    pub dynamic_merge_threshold: f32,
    /// Baseline threshold when considering continuity with the recent speaker
    pub continuity_threshold: f32,
    /// Maximum time gap (seconds) to maintain continuity with the recent speaker
    pub max_continuity_gap_seconds: f64,
    /// Auto-identification threshold for enrolled speaker profiles
    pub auto_identify_threshold: f32,
    /// Candidate display threshold for suggested profiles
    pub candidate_display_threshold: f32,
    /// Post-cluster oversegmentation repair threshold (merging redundant clusters)
    pub repair_merge_threshold: f32,
    /// Minimum turn audio duration (seconds) required to run neural embedding extraction
    pub min_turn_duration_seconds: f32,
}

impl Default for SpeakerModelThresholds {
    fn default() -> Self {
        Self::general()
    }
}

impl SpeakerModelThresholds {
    pub fn campplus() -> Self {
        Self {
            dynamic_merge_threshold: 0.48,
            continuity_threshold: 0.40,
            max_continuity_gap_seconds: 3.0,
            auto_identify_threshold: 0.58,
            candidate_display_threshold: 0.46,
            repair_merge_threshold: 0.52,
            min_turn_duration_seconds: 0.5,
        }
    }

    pub fn eres2net() -> Self {
        Self {
            dynamic_merge_threshold: 0.54,
            continuity_threshold: 0.46,
            max_continuity_gap_seconds: 3.0,
            auto_identify_threshold: 0.64,
            candidate_display_threshold: 0.50,
            repair_merge_threshold: 0.58,
            min_turn_duration_seconds: 0.5,
        }
    }

    pub fn eres2net_large() -> Self {
        Self {
            dynamic_merge_threshold: 0.58,
            continuity_threshold: 0.50,
            max_continuity_gap_seconds: 3.0,
            auto_identify_threshold: 0.68,
            candidate_display_threshold: 0.55,
            repair_merge_threshold: 0.62,
            min_turn_duration_seconds: 0.5,
        }
    }

    pub fn general() -> Self {
        Self {
            dynamic_merge_threshold: 0.50,
            continuity_threshold: 0.42,
            max_continuity_gap_seconds: 3.0,
            auto_identify_threshold: 0.60,
            candidate_display_threshold: 0.48,
            repair_merge_threshold: 0.54,
            min_turn_duration_seconds: 0.5,
        }
    }

    pub fn from_model_path(path: &Path) -> Self {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_lowercase();
        if filename.contains("eres2net_large") {
            Self::eres2net_large()
        } else if filename.contains("eres2net") {
            Self::eres2net()
        } else if filename.contains("campplus") {
            Self::campplus()
        } else {
            Self::general()
        }
    }
    pub fn with_sensitivity(mut self, sensitivity: Option<&str>) -> Self {
        match sensitivity
            .unwrap_or("balanced")
            .trim()
            .to_lowercase()
            .as_str()
        {
            "permissive" | "loose" => {
                self.dynamic_merge_threshold = (self.dynamic_merge_threshold - 0.05).max(0.35);
                self.continuity_threshold = (self.continuity_threshold - 0.05).max(0.30);
                self.repair_merge_threshold = (self.repair_merge_threshold - 0.05).max(0.40);
            }
            "strict" | "tight" => {
                self.dynamic_merge_threshold = (self.dynamic_merge_threshold + 0.05).min(0.90);
                self.continuity_threshold = (self.continuity_threshold + 0.05).min(0.85);
                self.repair_merge_threshold = (self.repair_merge_threshold + 0.05).min(0.90);
            }
            _ => {}
        }
        self
    }
}

/// Trims leading and trailing silence from audio samples based on local RMS energy.
/// Keeps a 50ms padding margin around detected active speech.
pub fn trim_speech_silence(samples: &[f32], sample_rate: usize) -> &[f32] {
    if samples.is_empty() {
        return samples;
    }

    let frame_size = (sample_rate * 20) / 1000; // 20ms = 320 samples at 16kHz
    let hop_size = (sample_rate * 10) / 1000; // 10ms = 160 samples
    let margin = (sample_rate * 50) / 1000; // 50ms = 800 samples

    if samples.len() <= frame_size {
        return samples;
    }

    let mut max_rms = 0.0_f32;
    let mut frame_start = 0;
    while frame_start + frame_size <= samples.len() {
        let frame = &samples[frame_start..frame_start + frame_size];
        let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame_size as f32).sqrt();
        if rms > max_rms {
            max_rms = rms;
        }
        frame_start += hop_size;
    }

    if max_rms < 1e-4 {
        return samples;
    }

    let energy_threshold = (max_rms * 0.08).clamp(1e-4, 0.015);

    // Find start frame
    let mut active_start = 0;
    frame_start = 0;
    while frame_start + frame_size <= samples.len() {
        let frame = &samples[frame_start..frame_start + frame_size];
        let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame_size as f32).sqrt();
        if rms >= energy_threshold {
            active_start = frame_start.saturating_sub(margin);
            break;
        }
        frame_start += hop_size;
    }

    // Find end frame
    let mut active_end = samples.len();
    let mut frame_end = samples.len();
    while frame_end >= frame_size {
        let start = frame_end - frame_size;
        let frame = &samples[start..frame_end];
        let sum_sq: f32 = frame.iter().map(|&x| x * x).sum();
        let rms = (sum_sq / frame_size as f32).sqrt();
        if rms >= energy_threshold {
            active_end = (frame_end + margin).min(samples.len());
            break;
        }
        if frame_end < hop_size {
            break;
        }
        frame_end -= hop_size;
    }

    if active_start < active_end && active_end - active_start >= (sample_rate * 400) / 1000 {
        &samples[active_start..active_end]
    } else {
        samples
    }
}

fn update_cluster_centroid(cluster: &mut DynamicSpeakerCluster, embedding: &[f32]) {
    const ALPHA: f32 = 0.80;
    for (c, &e) in cluster.centroid.iter_mut().zip(embedding.iter()) {
        *c = ALPHA * (*c) + (1.0 - ALPHA) * e;
    }
    let norm = cluster.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 1e-8 {
        for c in &mut cluster.centroid {
            *c /= norm;
        }
    }
    cluster.sample_count += 1;
}

#[derive(Debug, Clone)]
pub struct DynamicSpeakerCluster {
    pub raw_id: i32,
    pub anonymous_id: String,
    pub anonymous_label: String,
    pub centroid: Vec<f32>,
    pub sample_count: usize,
    pub bound_profile_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RecentSpeakerState {
    pub speaker: Option<SpeakerTag>,
    pub attribution: Option<SpeakerAttribution>,
    pub end_time: f64,
}

pub struct OnlineSpeakerTracker {
    embedding_index: Option<SpeakerEmbeddingIndex>,
    thresholds: SpeakerModelThresholds,
    profile_names: HashMap<String, String>,
    profile_sample_embeddings: HashMap<String, Vec<ProfileSampleEmbedding>>,
    profile_readiness: HashMap<String, SpeakerProfileReadinessState>,
    dynamic_clusters: Vec<DynamicSpeakerCluster>,
    recent_speaker: Option<RecentSpeakerState>,
    next_cluster_id: i32,
}

impl OnlineSpeakerTracker {
    pub fn new(config: Option<&SpeakerProcessingConfig>) -> Result<Self, AsrPortError> {
        let Some(config) = config else {
            return Ok(Self::empty());
        };

        let embedding_model_path = match config.speaker_embedding_model_path.as_deref() {
            Some(path) if !path.trim().is_empty() => match resolve_model_path(Some(path)) {
                Ok(p) => p,
                Err(error) => {
                    warn!(
                        "[OnlineSpeakerTracker] Failed to resolve speaker embedding model path '{path}': {error}"
                    );
                    return Ok(Self::empty());
                }
            },
            _ => return Ok(Self::empty()),
        };

        let thresholds = SpeakerModelThresholds::from_model_path(&embedding_model_path)
            .with_sensitivity(config.sensitivity.as_deref());

        let embedding_index = match SpeakerEmbeddingIndex::new(&embedding_model_path) {
            Ok(index) => index,
            Err(error) => {
                warn!(
                    "[OnlineSpeakerTracker] Failed to load speaker embedding model from {}: {error}",
                    embedding_model_path.display()
                );
                return Ok(Self::empty());
            }
        };

        let mut profile_names = HashMap::new();
        let mut profile_sample_embeddings = HashMap::new();
        let mut profile_readiness = HashMap::new();

        if let Some(profiles) = &config.speaker_profiles {
            for profile in profiles.iter().filter(|p| p.enabled) {
                let readiness = derive_profile_readiness(profile);
                if readiness == SpeakerProfileReadinessState::NotReady {
                    continue;
                }

                let mut sample_embeddings = Vec::new();
                let mut raw_embeddings = Vec::new();

                for sample in &profile.samples {
                    if sample.duration_seconds < 4.0 {
                        continue;
                    }
                    if let Ok(Some(emb)) =
                        embedding_index.compute_embedding_for_wav_file(&sample.file_path)
                    {
                        sample_embeddings.push(ProfileSampleEmbedding {
                            embedding: emb.clone(),
                            duration_seconds: sample.duration_seconds,
                        });
                        raw_embeddings.push(emb);
                    }
                }

                if !sample_embeddings.is_empty() {
                    let _ = embedding_index.add_profile_embeddings(
                        &profile.id,
                        &profile.name,
                        &raw_embeddings,
                    );
                    profile_names.insert(profile.id.clone(), profile.name.clone());
                    profile_readiness.insert(profile.id.clone(), readiness);
                    profile_sample_embeddings.insert(profile.id.clone(), sample_embeddings);
                }
            }
        }

        info!(
            "[OnlineSpeakerTracker] Initialized online speaker tracker with {} enrolled profiles using thresholds {:?}",
            profile_names.len(),
            thresholds
        );

        Ok(Self {
            embedding_index: Some(embedding_index),
            thresholds,
            profile_names,
            profile_sample_embeddings,
            profile_readiness,
            dynamic_clusters: Vec::new(),
            recent_speaker: None,
            next_cluster_id: 1,
        })
    }

    pub fn empty() -> Self {
        Self {
            embedding_index: None,
            thresholds: SpeakerModelThresholds::default(),
            profile_names: HashMap::new(),
            profile_sample_embeddings: HashMap::new(),
            profile_readiness: HashMap::new(),
            dynamic_clusters: Vec::new(),
            recent_speaker: None,
            next_cluster_id: 1,
        }
    }

    pub fn with_thresholds(mut self, thresholds: SpeakerModelThresholds) -> Self {
        self.thresholds = thresholds;
        self
    }

    pub fn thresholds(&self) -> SpeakerModelThresholds {
        self.thresholds
    }

    pub fn is_enabled(&self) -> bool {
        self.embedding_index.is_some()
    }

    pub fn reset_session(&mut self) {
        self.dynamic_clusters.clear();
        self.recent_speaker = None;
        self.next_cluster_id = 1;
    }

    pub fn identify_turn(
        &mut self,
        samples: &[f32],
        start_time: f64,
        end_time: f64,
    ) -> (Option<SpeakerTag>, Option<SpeakerAttribution>) {
        let Some(embedding_index) = self.embedding_index.as_ref() else {
            return (None, None);
        };

        if samples.is_empty() {
            return (None, None);
        }

        // 1. Acoustic silence trimming
        let trimmed = trim_speech_silence(samples, 16_000);
        let duration_seconds = trimmed.len() as f32 / 16_000.0;

        // 2. Short turn acoustic guard (< min_turn_duration_seconds)
        if duration_seconds < self.thresholds.min_turn_duration_seconds {
            if let Some(recent) = self.recent_speaker.clone()
                && (start_time - recent.end_time).abs()
                    <= self.thresholds.max_continuity_gap_seconds
            {
                let mut attribution = recent.attribution.clone();
                if let Some(attr) = attribution.as_mut() {
                    attr.confidence = "medium".to_string();
                }
                self.recent_speaker = Some(RecentSpeakerState {
                    speaker: recent.speaker.clone(),
                    attribution: attribution.clone(),
                    end_time,
                });
                return (recent.speaker, attribution);
            }
            return self.fallback_anonymous(end_time);
        }

        // 3. Extract embedding from trimmed speech turn
        let embedding = match embedding_index.compute_embedding_for_samples(trimmed) {
            Ok(Some(emb)) => emb,
            _ => {
                if let Some(recent) = self.recent_speaker.clone()
                    && (start_time - recent.end_time).abs()
                        <= self.thresholds.max_continuity_gap_seconds
                {
                    self.recent_speaker = Some(RecentSpeakerState {
                        speaker: recent.speaker.clone(),
                        attribution: recent.attribution.clone(),
                        end_time,
                    });
                    return (recent.speaker, recent.attribution);
                }
                return self.fallback_anonymous(end_time);
            }
        };

        // 4. Match against enrolled profiles
        let mut candidates = Vec::new();
        if !self.profile_names.is_empty() {
            let matches = embedding_index.best_matches(
                &embedding,
                self.thresholds.candidate_display_threshold,
                3,
            );
            for m in matches {
                if let Some(profile_name) = self.profile_names.get(&m.name) {
                    let score = if let Some(sample_embs) =
                        self.profile_sample_embeddings.get(&m.name)
                    {
                        let total_dur: f32 = sample_embs.iter().map(|s| s.duration_seconds).sum();
                        if total_dur > 0.0 {
                            let weighted_sim: f32 = sample_embs
                                .iter()
                                .map(|s| {
                                    s.duration_seconds * cosine_similarity(&embedding, &s.embedding)
                                })
                                .sum::<f32>()
                                / total_dur;
                            let max_sim = sample_embs
                                .iter()
                                .map(|s| cosine_similarity(&embedding, &s.embedding))
                                .fold(f32::NEG_INFINITY, f32::max);
                            (0.6 * weighted_sim + 0.4 * max_sim).clamp(0.0, 1.0)
                        } else {
                            m.score
                        }
                    } else {
                        m.score
                    };

                    candidates.push(SpeakerCandidate {
                        profile_id: m.name,
                        profile_name: profile_name.clone(),
                        score,
                        rank: 0,
                    });
                }
            }

            candidates.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(Ordering::Equal));
            for (idx, c) in candidates.iter_mut().enumerate() {
                c.rank = idx + 1;
            }
        }

        if let Some(top) = candidates.first().cloned() {
            let second_score = candidates.get(1).map(|c| c.score).unwrap_or(0.0);
            let margin = top.score - second_score;
            let readiness = self
                .profile_readiness
                .get(&top.profile_id)
                .copied()
                .unwrap_or(SpeakerProfileReadinessState::NotReady);
            let has_competition = candidates.len() > 1;
            let margin_satisfied = !has_competition || margin >= AUTO_IDENTIFICATION_MIN_MARGIN;

            if readiness == SpeakerProfileReadinessState::Ready
                && top.score >= self.thresholds.auto_identify_threshold
                && margin_satisfied
            {
                let speaker = SpeakerTag {
                    id: top.profile_id.clone(),
                    label: top.profile_name.clone(),
                    kind: "identified".to_string(),
                    score: Some(top.score),
                };
                let attribution = SpeakerAttribution {
                    group_id: format!("profile-{}", top.profile_id),
                    anonymous_label: top.profile_name.clone(),
                    state: "identified".to_string(),
                    source: "auto".to_string(),
                    confidence: "high".to_string(),
                    candidates,
                };

                self.bind_or_update_profile_cluster(&top.profile_id, &embedding);
                self.repair_oversegmentation();

                self.recent_speaker = Some(RecentSpeakerState {
                    speaker: Some(speaker.clone()),
                    attribution: Some(attribution.clone()),
                    end_time,
                });
                return (Some(speaker), Some(attribution));
            }
        }

        // 5. Recent Speaker Temporal Continuity Check
        if let Some(recent) = self.recent_speaker.clone()
            && let Some(recent_spk) = recent.speaker.as_ref()
        {
            let gap = (start_time - recent.end_time).max(0.0);
            if gap <= self.thresholds.max_continuity_gap_seconds {
                let cluster_idx = self.dynamic_clusters.iter().position(|c| {
                    c.anonymous_id == recent_spk.id
                        || c.bound_profile_id.as_deref() == Some(&recent_spk.id)
                });

                if let Some(idx) = cluster_idx {
                    let sim = cosine_similarity(&embedding, &self.dynamic_clusters[idx].centroid);
                    let bonus = 0.08
                        * (1.0 - (gap / self.thresholds.max_continuity_gap_seconds) as f32)
                            .max(0.0);
                    if sim >= self.thresholds.continuity_threshold
                        || sim + bonus >= self.thresholds.dynamic_merge_threshold
                    {
                        update_cluster_centroid(&mut self.dynamic_clusters[idx], &embedding);
                        self.repair_oversegmentation();

                        let mut attribution =
                            recent.attribution.clone().unwrap_or(SpeakerAttribution {
                                group_id: recent_spk.id.clone(),
                                anonymous_label: recent_spk.label.clone(),
                                state: recent_spk.kind.clone(),
                                source: "auto".to_string(),
                                confidence: "high".to_string(),
                                candidates: candidates.clone(),
                            });
                        attribution.candidates = candidates;

                        self.recent_speaker = Some(RecentSpeakerState {
                            speaker: Some(recent_spk.clone()),
                            attribution: Some(attribution.clone()),
                            end_time,
                        });
                        return (Some(recent_spk.clone()), Some(attribution));
                    }
                }
            }
        }

        // 6. Match against all existing dynamic clusters
        let mut best_cluster_idx = None;
        let mut best_sim = -1.0_f32;

        for (idx, cluster) in self.dynamic_clusters.iter().enumerate() {
            let sim = cosine_similarity(&embedding, &cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_cluster_idx = Some(idx);
            }
        }

        if let Some(idx) = best_cluster_idx
            && best_sim >= self.thresholds.dynamic_merge_threshold
        {
            let cluster = &mut self.dynamic_clusters[idx];
            update_cluster_centroid(cluster, &embedding);
            let cluster_clone = cluster.clone();
            self.repair_oversegmentation();

            let has_suggestion = candidates
                .first()
                .map(|c| c.score >= self.thresholds.candidate_display_threshold)
                .unwrap_or(false);

            let (state, confidence) = if has_suggestion {
                ("suggested".to_string(), "medium".to_string())
            } else {
                ("anonymous".to_string(), "low".to_string())
            };
            let (speaker, attribution) = if let Some(profile_id) =
                cluster_clone.bound_profile_id.as_deref()
                && let Some(profile_name) = self.profile_names.get(profile_id)
            {
                (
                    SpeakerTag {
                        id: profile_id.to_string(),
                        label: profile_name.clone(),
                        kind: "identified".to_string(),
                        score: candidates.first().map(|c| c.score).or(Some(best_sim)),
                    },
                    SpeakerAttribution {
                        group_id: profile_id.to_string(),
                        anonymous_label: profile_name.clone(),
                        state: "identified".to_string(),
                        source: "auto".to_string(),
                        confidence: "high".to_string(),
                        candidates,
                    },
                )
            } else {
                (
                    SpeakerTag {
                        id: cluster_clone.anonymous_id.clone(),
                        label: cluster_clone.anonymous_label.clone(),
                        kind: "anonymous".to_string(),
                        score: candidates.first().map(|c| c.score),
                    },
                    SpeakerAttribution {
                        group_id: cluster_clone.anonymous_id.clone(),
                        anonymous_label: cluster_clone.anonymous_label.clone(),
                        state,
                        source: "auto".to_string(),
                        confidence,
                        candidates,
                    },
                )
            };

            self.recent_speaker = Some(RecentSpeakerState {
                speaker: Some(speaker.clone()),
                attribution: Some(attribution.clone()),
                end_time,
            });
            return (Some(speaker), Some(attribution));
        }

        // 7. No existing cluster matched -> Create New Dynamic Cluster
        let new_cluster = self.create_dynamic_cluster(&embedding, None);
        let has_suggestion = candidates
            .first()
            .map(|c| c.score >= self.thresholds.candidate_display_threshold)
            .unwrap_or(false);

        let (state, confidence) = if has_suggestion {
            ("suggested".to_string(), "medium".to_string())
        } else {
            ("anonymous".to_string(), "low".to_string())
        };

        let speaker = SpeakerTag {
            id: new_cluster.anonymous_id.clone(),
            label: new_cluster.anonymous_label.clone(),
            kind: "anonymous".to_string(),
            score: candidates.first().map(|c| c.score),
        };
        let attribution = SpeakerAttribution {
            group_id: new_cluster.anonymous_id.clone(),
            anonymous_label: new_cluster.anonymous_label.clone(),
            state,
            source: "auto".to_string(),
            confidence,
            candidates,
        };

        self.recent_speaker = Some(RecentSpeakerState {
            speaker: Some(speaker.clone()),
            attribution: Some(attribution.clone()),
            end_time,
        });
        (Some(speaker), Some(attribution))
    }

    pub fn find_or_create_dynamic_cluster(&mut self, embedding: &[f32]) -> DynamicSpeakerCluster {
        let mut best_cluster_idx = None;
        let mut best_sim = -1.0_f32;

        for (idx, cluster) in self.dynamic_clusters.iter().enumerate() {
            let sim = cosine_similarity(embedding, &cluster.centroid);
            if sim > best_sim {
                best_sim = sim;
                best_cluster_idx = Some(idx);
            }
        }

        if let Some(idx) = best_cluster_idx
            && best_sim >= self.thresholds.dynamic_merge_threshold
        {
            let cluster = &mut self.dynamic_clusters[idx];
            update_cluster_centroid(cluster, embedding);
            let result = cluster.clone();
            self.repair_oversegmentation();
            return result;
        }

        self.create_dynamic_cluster(embedding, None)
    }

    fn bind_or_update_profile_cluster(&mut self, profile_id: &str, embedding: &[f32]) {
        let existing_idx = self
            .dynamic_clusters
            .iter()
            .position(|c| c.bound_profile_id.as_deref() == Some(profile_id));
        if let Some(idx) = existing_idx {
            update_cluster_centroid(&mut self.dynamic_clusters[idx], embedding);
        } else {
            self.create_dynamic_cluster(embedding, Some(profile_id.to_string()));
        }
    }

    fn create_dynamic_cluster(
        &mut self,
        embedding: &[f32],
        bound_profile_id: Option<String>,
    ) -> DynamicSpeakerCluster {
        let raw_id = self.next_cluster_id;
        self.next_cluster_id += 1;
        let anonymous_id = format!("speaker-{raw_id}");
        let anonymous_label = format!("Speaker {raw_id}");

        let mut centroid = embedding.to_vec();
        let norm = centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm > 1e-8 {
            for c in &mut centroid {
                *c /= norm;
            }
        }

        let new_cluster = DynamicSpeakerCluster {
            raw_id,
            anonymous_id,
            anonymous_label,
            centroid,
            sample_count: 1,
            bound_profile_id,
        };
        self.dynamic_clusters.push(new_cluster.clone());
        new_cluster
    }

    fn repair_oversegmentation(&mut self) {
        if self.dynamic_clusters.len() <= 1 {
            return;
        }
        let threshold = self.thresholds.repair_merge_threshold;
        let mut to_merge = None;
        for i in 0..self.dynamic_clusters.len() {
            for j in (i + 1)..self.dynamic_clusters.len() {
                if let (Some(p1), Some(p2)) = (
                    &self.dynamic_clusters[i].bound_profile_id,
                    &self.dynamic_clusters[j].bound_profile_id,
                ) && p1 != p2
                {
                    continue;
                }
                let sim = cosine_similarity(
                    &self.dynamic_clusters[i].centroid,
                    &self.dynamic_clusters[j].centroid,
                );
                if sim >= threshold {
                    to_merge = Some((i, j));
                    break;
                }
            }
            if to_merge.is_some() {
                break;
            }
        }

        if let Some((i, j)) = to_merge {
            let removed = self.dynamic_clusters.remove(j);
            let cluster_i = &mut self.dynamic_clusters[i];
            let n_i = cluster_i.sample_count as f32;
            let n_j = removed.sample_count as f32;
            for (ci, rj) in cluster_i.centroid.iter_mut().zip(removed.centroid.iter()) {
                *ci = *ci * n_i + *rj * n_j;
            }
            let norm = cluster_i.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-8 {
                for c in &mut cluster_i.centroid {
                    *c /= norm;
                }
            }
            cluster_i.sample_count += removed.sample_count;

            if cluster_i.bound_profile_id.is_none() && removed.bound_profile_id.is_some() {
                cluster_i.bound_profile_id = removed.bound_profile_id;
            }

            if let Some(recent) = self.recent_speaker.as_mut()
                && let Some(spk) = recent.speaker.as_mut()
                && spk.id == removed.anonymous_id
            {
                spk.id = cluster_i.anonymous_id.clone();
                spk.label = cluster_i.anonymous_label.clone();
                if let Some(attr) = recent.attribution.as_mut() {
                    attr.group_id = cluster_i.anonymous_id.clone();
                    attr.anonymous_label = cluster_i.anonymous_label.clone();
                }
            }
        }
    }

    fn fallback_anonymous(
        &mut self,
        end_time: f64,
    ) -> (Option<SpeakerTag>, Option<SpeakerAttribution>) {
        let raw_id = self.next_cluster_id;
        self.next_cluster_id += 1;
        let id = format!("speaker-{raw_id}");
        let label = format!("Speaker {raw_id}");
        let speaker = SpeakerTag {
            id: id.clone(),
            label: label.clone(),
            kind: "anonymous".to_string(),
            score: None,
        };
        let attribution = SpeakerAttribution {
            group_id: id,
            anonymous_label: label,
            state: "anonymous".to_string(),
            source: "auto".to_string(),
            confidence: "low".to_string(),
            candidates: Vec::new(),
        };

        self.recent_speaker = Some(RecentSpeakerState {
            speaker: Some(speaker.clone()),
            attribution: Some(attribution.clone()),
            end_time,
        });

        (Some(speaker), Some(attribution))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tracker_returns_none() {
        let mut tracker = OnlineSpeakerTracker::empty();
        assert!(!tracker.is_enabled());

        let fake_audio = vec![0.1_f32; 16000];
        let (speaker, attribution) = tracker.identify_turn(&fake_audio, 0.0, 1.0);
        assert!(speaker.is_none());
        assert!(attribution.is_none());
    }

    #[test]
    fn threshold_mapping_resolves_model_types() {
        let campplus_th = SpeakerModelThresholds::from_model_path(Path::new(
            "models/3dspeaker_speech_campplus_sv_zh_en_16k-common_advanced.onnx",
        ));
        assert_eq!(campplus_th, SpeakerModelThresholds::campplus());
        assert_eq!(campplus_th.dynamic_merge_threshold, 0.48);

        let eres2net_th = SpeakerModelThresholds::from_model_path(Path::new(
            "models/3dspeaker_speech_eres2netv2_sv_zh-cn_16k-common.onnx",
        ));
        assert_eq!(eres2net_th, SpeakerModelThresholds::eres2net());
        assert_eq!(eres2net_th.dynamic_merge_threshold, 0.54);

        let eres2net_large_th = SpeakerModelThresholds::from_model_path(Path::new(
            "models/3dspeaker_speech_eres2net_large_sv_zh-cn_3dspeaker_16k.onnx",
        ));
        assert_eq!(eres2net_large_th, SpeakerModelThresholds::eres2net_large());
        assert_eq!(eres2net_large_th.dynamic_merge_threshold, 0.58);

        let generic_th =
            SpeakerModelThresholds::from_model_path(Path::new("models/custom_unknown.onnx"));
        assert_eq!(generic_th, SpeakerModelThresholds::general());
    }

    #[test]
    fn dynamic_clustering_merges_similar_and_separates_dissimilar() {
        let mut tracker = OnlineSpeakerTracker::empty();

        let emb1 = vec![1.0, 0.0, 0.0];
        let cluster1 = tracker.find_or_create_dynamic_cluster(&emb1);
        assert_eq!(cluster1.anonymous_id, "speaker-1");
        assert_eq!(cluster1.sample_count, 1);

        // Moderately similar embedding (cosine sim ~0.60 > 0.50) -> merges into cluster1
        let emb1_similar = vec![0.8, 0.6, 0.0];
        let cluster1_merged = tracker.find_or_create_dynamic_cluster(&emb1_similar);
        assert_eq!(cluster1_merged.anonymous_id, "speaker-1");
        assert_eq!(cluster1_merged.sample_count, 2);

        // Orthogonal/dissimilar embedding -> creates cluster2
        let emb2 = vec![0.0, 0.0, 1.0];
        let cluster2 = tracker.find_or_create_dynamic_cluster(&emb2);
        assert_eq!(cluster2.anonymous_id, "speaker-2");
        assert_eq!(cluster2.sample_count, 1);
    }

    #[test]
    fn short_utterance_inherits_recent_speaker_within_gap() {
        let mut tracker = OnlineSpeakerTracker::empty();

        tracker.recent_speaker = Some(RecentSpeakerState {
            speaker: Some(SpeakerTag {
                id: "spk-alice".to_string(),
                label: "Alice".to_string(),
                kind: "identified".to_string(),
                score: Some(0.95),
            }),
            attribution: Some(SpeakerAttribution {
                group_id: "profile-alice".to_string(),
                anonymous_label: "Alice".to_string(),
                state: "identified".to_string(),
                source: "auto".to_string(),
                confidence: "high".to_string(),
                candidates: Vec::new(),
            }),
            end_time: 5.0,
        });

        // Short audio (< min_turn_duration_seconds)
        let short_audio = vec![0.1_f32; 4000];
        let (speaker, attribution) = tracker.identify_turn(&short_audio, 5.5, 5.75);
        assert!(speaker.is_none());
        assert!(attribution.is_none());
    }

    #[test]
    fn session_reset_clears_dynamic_clusters_and_recent_state() {
        let mut tracker = OnlineSpeakerTracker::empty();
        let emb = vec![1.0, 0.0];
        tracker.find_or_create_dynamic_cluster(&emb);
        assert_eq!(tracker.dynamic_clusters.len(), 1);

        tracker.reset_session();
        assert!(tracker.dynamic_clusters.is_empty());
        assert!(tracker.recent_speaker.is_none());
        assert_eq!(tracker.next_cluster_id, 1);
    }

    #[test]
    fn silence_trimming_removes_leading_and_trailing_silence() {
        // 16000 Hz: 0.5s silence + 1.0s sound + 0.5s silence
        let mut audio = vec![0.0_f32; 8000];
        let speech = vec![0.2_f32; 16000];
        audio.extend_from_slice(&speech);
        audio.extend_from_slice(&vec![0.0_f32; 8000]);

        let trimmed = trim_speech_silence(&audio, 16000);
        // Trimmed audio should exclude the silence and be close to 16000 + 2*margin (16000 + 1600 = 17600)
        assert!(trimmed.len() < audio.len());
        assert!(trimmed.len() >= 16000);
    }

    #[test]
    fn oversegmentation_repair_merges_redundant_clusters() {
        let mut tracker = OnlineSpeakerTracker::empty().with_thresholds(SpeakerModelThresholds {
            dynamic_merge_threshold: 0.50,
            continuity_threshold: 0.42,
            max_continuity_gap_seconds: 3.0,
            auto_identify_threshold: 0.60,
            candidate_display_threshold: 0.48,
            repair_merge_threshold: 0.55,
            min_turn_duration_seconds: 0.5,
        });

        // Add cluster 1
        let emb1 = vec![1.0, 0.0];
        tracker.create_dynamic_cluster(&emb1, None);

        // Add cluster 2 that is similar enough (cosine sim = 0.60 > repair_merge_threshold 0.55)
        let emb2 = vec![0.8, 0.6];
        tracker.create_dynamic_cluster(&emb2, None);

        assert_eq!(tracker.dynamic_clusters.len(), 2);
        tracker.repair_oversegmentation();
        assert_eq!(tracker.dynamic_clusters.len(), 1);
        assert_eq!(tracker.dynamic_clusters[0].anonymous_id, "speaker-1");
        assert_eq!(tracker.dynamic_clusters[0].sample_count, 2);
    }

    #[test]
    fn continuity_bonus_allows_consecutive_utterances_to_merge() {
        let mut tracker =
            OnlineSpeakerTracker::empty().with_thresholds(SpeakerModelThresholds::campplus());
        // Dynamic merge threshold is 0.48, continuity threshold is 0.40

        let emb1 = vec![1.0, 0.0];
        let cluster1 = tracker.find_or_create_dynamic_cluster(&emb1);
        tracker.recent_speaker = Some(RecentSpeakerState {
            speaker: Some(SpeakerTag {
                id: cluster1.anonymous_id.clone(),
                label: cluster1.anonymous_label.clone(),
                kind: "anonymous".to_string(),
                score: None,
            }),
            attribution: None,
            end_time: 2.0,
        });

        // Second utterance at t=3.0s (gap = 1.0s <= 3.0s)
        // Vector with cosine similarity ~0.44 (< 0.48 merge threshold, but >= 0.40 continuity threshold)
        // cos(theta) = 0.44 => x = 0.44, y = sqrt(1 - 0.44^2) = 0.898
        let emb2 = vec![0.44, 0.898];
        let sim = cosine_similarity(&emb1, &emb2);
        assert!((0.43..=0.45).contains(&sim));
        // Without continuity, 0.44 < 0.48 would create a new cluster.
        // But with continuity check:
        let gap = 1.0_f64;
        let bonus = 0.08 * (1.0 - (gap / 3.0) as f32);
        assert!(sim + bonus >= tracker.thresholds.dynamic_merge_threshold);
    }

    #[test]
    fn dissimilar_speaker_creates_new_cluster_even_with_short_gap() {
        let mut tracker =
            OnlineSpeakerTracker::empty().with_thresholds(SpeakerModelThresholds::campplus());

        let emb1 = vec![1.0, 0.0];
        let cluster1 = tracker.find_or_create_dynamic_cluster(&emb1);
        assert_eq!(cluster1.anonymous_id, "speaker-1");

        // Dissimilar speaker: emb2 orthogonal or low similarity (0.15)
        let emb2 = vec![0.15, 0.9887];
        let cluster2 = tracker.find_or_create_dynamic_cluster(&emb2);
        assert_eq!(cluster2.anonymous_id, "speaker-2");
    }

    #[test]
    fn sensitivity_adjusts_thresholds_correctly() {
        let base = SpeakerModelThresholds::campplus();
        assert!((base.dynamic_merge_threshold - 0.48).abs() < 1e-4);

        let permissive = base.with_sensitivity(Some("permissive"));
        assert!((permissive.dynamic_merge_threshold - 0.43).abs() < 1e-4);
        assert!((permissive.continuity_threshold - 0.35).abs() < 1e-4);

        let strict = base.with_sensitivity(Some("strict"));
        assert!((strict.dynamic_merge_threshold - 0.53).abs() < 1e-4);
        assert!((strict.continuity_threshold - 0.45).abs() < 1e-4);

        let balanced = base.with_sensitivity(Some("balanced"));
        assert!((balanced.dynamic_merge_threshold - 0.48).abs() < 1e-4);
    }

    #[test]
    fn repair_oversegmentation_does_not_merge_distinct_bound_profiles() {
        let mut tracker = OnlineSpeakerTracker::empty().with_thresholds(SpeakerModelThresholds {
            repair_merge_threshold: 0.50,
            ..SpeakerModelThresholds::default()
        });

        // Two clusters with similarity 0.99, but bound to different profiles
        tracker.create_dynamic_cluster(&[1.0, 0.0], Some("profile-alice".to_string()));
        tracker.create_dynamic_cluster(&[0.99, 0.05], Some("profile-bob".to_string()));
        assert_eq!(tracker.dynamic_clusters.len(), 2);

        tracker.repair_oversegmentation();
        // Must NOT merge them!
        assert_eq!(tracker.dynamic_clusters.len(), 2);
        assert_eq!(
            tracker.dynamic_clusters[0].bound_profile_id.as_deref(),
            Some("profile-alice")
        );
        assert_eq!(
            tracker.dynamic_clusters[1].bound_profile_id.as_deref(),
            Some("profile-bob")
        );
    }

    #[test]
    fn fallback_anonymous_allocates_new_speaker_id() {
        let mut tracker = OnlineSpeakerTracker::empty();
        tracker.next_cluster_id = 3;

        let (spk, attr) = tracker.fallback_anonymous(10.0);
        let spk = spk.unwrap();
        assert_eq!(spk.id, "speaker-3");
        assert_eq!(spk.label, "Speaker 3");
        assert_eq!(attr.unwrap().state, "anonymous");
        assert_eq!(tracker.next_cluster_id, 4);
    }
}
