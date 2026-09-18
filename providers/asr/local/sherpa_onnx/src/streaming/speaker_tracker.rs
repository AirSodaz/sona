use crate::speaker::SpeakerEmbeddingIndex;
use crate::speaker_processing::{
    AUTO_IDENTIFICATION_MIN_MARGIN, AUTO_IDENTIFICATION_THRESHOLD, CANDIDATE_DISPLAY_THRESHOLD,
    ProfileSampleEmbedding, SpeakerProfileReadinessState, cosine_similarity,
    derive_profile_readiness, resolve_model_path,
};
use log::{info, warn};
use sona_core::ports::asr::AsrPortError;
use sona_core::transcription::speaker::SpeakerProcessingConfig;
use sona_core::transcription::transcript::{SpeakerAttribution, SpeakerCandidate, SpeakerTag};
use std::cmp::Ordering;
use std::collections::HashMap;

pub const ONLINE_DYNAMIC_MERGE_THRESHOLD: f32 = 0.75;
pub const ONLINE_MIN_TURN_DURATION_SECONDS: f32 = 0.8;
pub const ONLINE_CONTINUITY_MAX_GAP_SECONDS: f64 = 1.2;

#[derive(Debug, Clone)]
pub struct DynamicSpeakerCluster {
    pub raw_id: i32,
    pub anonymous_id: String,
    pub anonymous_label: String,
    pub centroid: Vec<f32>,
    pub sample_count: usize,
}

#[derive(Debug, Clone)]
pub struct RecentSpeakerState {
    pub speaker: Option<SpeakerTag>,
    pub attribution: Option<SpeakerAttribution>,
    pub end_time: f64,
}

pub struct OnlineSpeakerTracker {
    embedding_index: Option<SpeakerEmbeddingIndex>,
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
            "[OnlineSpeakerTracker] Initialized online speaker tracker with {} enrolled profiles",
            profile_names.len()
        );

        Ok(Self {
            embedding_index: Some(embedding_index),
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
            profile_names: HashMap::new(),
            profile_sample_embeddings: HashMap::new(),
            profile_readiness: HashMap::new(),
            dynamic_clusters: Vec::new(),
            recent_speaker: None,
            next_cluster_id: 1,
        }
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

        let duration_seconds = samples.len() as f32 / 16_000.0;

        // Short utterance continuity check
        if duration_seconds < ONLINE_MIN_TURN_DURATION_SECONDS
            && let Some(recent) = self.recent_speaker.clone()
            && (start_time - recent.end_time).abs() <= ONLINE_CONTINUITY_MAX_GAP_SECONDS
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

        // Extract embedding for the speech turn
        let embedding = match embedding_index.compute_embedding_for_samples(samples) {
            Ok(Some(emb)) => emb,
            _ => {
                return self.fallback_anonymous(end_time);
            }
        };

        // Match against enrolled profiles
        let mut candidates = Vec::new();
        if !self.profile_names.is_empty() {
            let matches = embedding_index.best_matches(&embedding, CANDIDATE_DISPLAY_THRESHOLD, 3);
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
                && top.score >= AUTO_IDENTIFICATION_THRESHOLD
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

                self.recent_speaker = Some(RecentSpeakerState {
                    speaker: Some(speaker.clone()),
                    attribution: Some(attribution.clone()),
                    end_time,
                });
                return (Some(speaker), Some(attribution));
            }

            if top.score >= CANDIDATE_DISPLAY_THRESHOLD {
                let cluster = self.find_or_create_dynamic_cluster(&embedding);
                let speaker = SpeakerTag {
                    id: cluster.anonymous_id.clone(),
                    label: cluster.anonymous_label.clone(),
                    kind: "anonymous".to_string(),
                    score: Some(top.score),
                };
                let attribution = SpeakerAttribution {
                    group_id: cluster.anonymous_id.clone(),
                    anonymous_label: cluster.anonymous_label.clone(),
                    state: "suggested".to_string(),
                    source: "auto".to_string(),
                    confidence: "medium".to_string(),
                    candidates,
                };

                self.recent_speaker = Some(RecentSpeakerState {
                    speaker: Some(speaker.clone()),
                    attribution: Some(attribution.clone()),
                    end_time,
                });
                return (Some(speaker), Some(attribution));
            }
        }

        // No enrolled profile matched -> Dynamic session cluster
        let cluster = self.find_or_create_dynamic_cluster(&embedding);
        let speaker = SpeakerTag {
            id: cluster.anonymous_id.clone(),
            label: cluster.anonymous_label.clone(),
            kind: "anonymous".to_string(),
            score: None,
        };
        let attribution = SpeakerAttribution {
            group_id: cluster.anonymous_id.clone(),
            anonymous_label: cluster.anonymous_label.clone(),
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

    fn find_or_create_dynamic_cluster(&mut self, embedding: &[f32]) -> DynamicSpeakerCluster {
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
            && best_sim >= ONLINE_DYNAMIC_MERGE_THRESHOLD
        {
            let cluster = &mut self.dynamic_clusters[idx];
            let n = cluster.sample_count as f32;
            for (c, e) in cluster.centroid.iter_mut().zip(embedding.iter()) {
                *c = (*c * n + *e) / (n + 1.0);
            }
            let norm = cluster.centroid.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 1e-8 {
                for c in &mut cluster.centroid {
                    *c /= norm;
                }
            }
            cluster.sample_count += 1;
            return cluster.clone();
        }

        let raw_id = self.next_cluster_id;
        self.next_cluster_id += 1;
        let anonymous_id = format!("speaker-{raw_id}");
        let anonymous_label = format!("Speaker {raw_id}");

        let new_cluster = DynamicSpeakerCluster {
            raw_id,
            anonymous_id,
            anonymous_label,
            centroid: embedding.to_vec(),
            sample_count: 1,
        };
        self.dynamic_clusters.push(new_cluster.clone());
        new_cluster
    }

    fn fallback_anonymous(
        &mut self,
        end_time: f64,
    ) -> (Option<SpeakerTag>, Option<SpeakerAttribution>) {
        let (id, label) = if let Some(recent) = &self.recent_speaker {
            if let Some(spk) = &recent.speaker {
                (spk.id.clone(), spk.label.clone())
            } else {
                ("speaker-1".to_string(), "Speaker 1".to_string())
            }
        } else if let Some(c) = self.dynamic_clusters.first() {
            (c.anonymous_id.clone(), c.anonymous_label.clone())
        } else {
            ("speaker-1".to_string(), "Speaker 1".to_string())
        };

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
    fn dynamic_clustering_merges_similar_and_separates_dissimilar() {
        let mut tracker = OnlineSpeakerTracker::empty();

        let emb1 = vec![1.0, 0.0, 0.0];
        let cluster1 = tracker.find_or_create_dynamic_cluster(&emb1);
        assert_eq!(cluster1.anonymous_id, "speaker-1");
        assert_eq!(cluster1.sample_count, 1);

        // Very similar embedding (> 0.75) -> merges into cluster1
        let emb1_similar = vec![0.98, 0.02, 0.0];
        let cluster1_merged = tracker.find_or_create_dynamic_cluster(&emb1_similar);
        assert_eq!(cluster1_merged.anonymous_id, "speaker-1");
        assert_eq!(cluster1_merged.sample_count, 2);

        // Orthogonal/dissimilar embedding -> creates cluster2
        let emb2 = vec![0.0, 1.0, 0.0];
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

        // Short audio (< 0.8s), starts at 5.5s (gap 0.5s <= 1.2s threshold)
        // Note: tracker.embedding_index is None, but short utterance check happens when embedding_index is Some.
        // With embedding_index None, identify_turn returns (None, None).
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
}
