use hound::SampleFormat;
use sherpa_onnx::{
    FastClusteringConfig, OfflineSpeakerDiarization, OfflineSpeakerDiarizationConfig,
    OfflineSpeakerSegmentationModelConfig, OfflineSpeakerSegmentationPyannoteModelConfig,
    SpeakerEmbeddingExtractor, SpeakerEmbeddingExtractorConfig, SpeakerEmbeddingManager,
};
use sona_core::ports::asr::{AsrPortError, AsrPortErrorKind};
use std::path::Path;

const SAMPLE_RATE: i32 = 16_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerDiarizationSegment {
    pub start: f32,
    pub end: f32,
    pub speaker: i32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SpeakerEmbeddingMatch {
    pub name: String,
    pub score: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeakerModelThresholds {
    /// Dynamic clustering merge threshold for new/unknown speakers
    pub dynamic_merge_threshold: f32,
    /// Temporal continuity bonus threshold when recent speaker matches
    pub continuity_threshold: f32,
    /// Maximum gap (seconds) to consider consecutive speech from same speaker
    pub max_continuity_gap_seconds: f64,
    /// Minimum similarity score to auto-identify as an enrolled profile
    pub auto_identify_threshold: f32,
    /// Minimum similarity score to display enrolled profile as candidate
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
        let path_str = path.to_string_lossy().to_lowercase();
        if path_str.contains("eres2net_large") {
            Self::eres2net_large()
        } else if path_str.contains("eres2net") {
            Self::eres2net()
        } else if path_str.contains("campplus") || path_str.contains("cam++") {
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
                self.auto_identify_threshold = (self.auto_identify_threshold - 0.05).max(0.45);
                self.candidate_display_threshold =
                    (self.candidate_display_threshold - 0.05).max(0.35);
            }
            "strict" | "tight" => {
                self.dynamic_merge_threshold = (self.dynamic_merge_threshold + 0.05).min(0.90);
                self.continuity_threshold = (self.continuity_threshold + 0.05).min(0.85);
                self.repair_merge_threshold = (self.repair_merge_threshold + 0.05).min(0.90);
                self.auto_identify_threshold = (self.auto_identify_threshold + 0.05).min(0.90);
                self.candidate_display_threshold =
                    (self.candidate_display_threshold + 0.05).min(0.85);
            }
            _ => {}
        }
        self
    }

    /// Derives thresholds tailored for offline batch diarization.
    /// In batch diarization, cluster centroids are averaged over purified speech spans,
    /// so cosine similarity of the same speaker is typically slightly higher than single streaming turns.
    pub fn for_batch_diarization(model_path: &Path, sensitivity: Option<&str>) -> Self {
        let mut t = Self::from_model_path(model_path).with_sensitivity(sensitivity);
        t.repair_merge_threshold = (t.repair_merge_threshold + 0.10).clamp(0.55, 0.85);
        t
    }
}

pub struct SpeakerEmbeddingIndex {
    extractor: SpeakerEmbeddingExtractor,
    manager: SpeakerEmbeddingManager,
}

impl SpeakerEmbeddingIndex {
    pub fn new(embedding_model: &Path) -> Result<Self, AsrPortError> {
        let extractor = SpeakerEmbeddingExtractor::create(&SpeakerEmbeddingExtractorConfig {
            model: Some(embedding_model.to_string_lossy().into_owned()),
            num_threads: 1,
            debug: false,
            provider: Some("cpu".to_string()),
        })
        .ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "Failed to create speaker embedding extractor",
            )
        })?;

        let manager = SpeakerEmbeddingManager::create(extractor.dim()).ok_or_else(|| {
            AsrPortError::new(
                AsrPortErrorKind::Model,
                "Failed to create speaker embedding manager",
            )
        })?;

        Ok(Self { extractor, manager })
    }

    pub fn add_profile_embeddings(
        &self,
        profile_id: &str,
        profile_name: &str,
        embeddings: &[Vec<f32>],
    ) -> Result<(), AsrPortError> {
        if self.manager.add_list(profile_id, embeddings) {
            Ok(())
        } else {
            Err(AsrPortError::runtime(format!(
                "Failed to index speaker profile {profile_name}"
            )))
        }
    }

    pub fn compute_embedding_for_wav_file(
        &self,
        file_path: &str,
    ) -> Result<Option<Vec<f32>>, AsrPortError> {
        let samples = load_profile_sample_wav(file_path)?;
        self.compute_embedding_for_samples(&samples)
    }

    pub fn compute_embedding_for_span(
        &self,
        samples: &[f32],
        start: f32,
        end: f32,
    ) -> Result<Option<Vec<f32>>, AsrPortError> {
        let start_index = ((start.max(0.0)) * SAMPLE_RATE as f32).floor() as usize;
        let end_index = ((end.max(start)) * SAMPLE_RATE as f32).ceil() as usize;
        if start_index >= samples.len() || end_index <= start_index {
            return Ok(None);
        }

        let bounded_end = end_index.min(samples.len());
        self.compute_embedding_for_samples(&samples[start_index..bounded_end])
    }

    pub fn best_matches(
        &self,
        embedding: &[f32],
        threshold: f32,
        max_matches: i32,
    ) -> Vec<SpeakerEmbeddingMatch> {
        self.manager
            .get_best_matches(embedding, threshold, max_matches)
            .into_iter()
            .map(|best_match| SpeakerEmbeddingMatch {
                name: best_match.name,
                score: best_match.score,
            })
            .collect()
    }

    pub fn compute_embedding_for_samples(
        &self,
        samples: &[f32],
    ) -> Result<Option<Vec<f32>>, AsrPortError> {
        if samples.is_empty() {
            return Ok(None);
        }

        let stream = self
            .extractor
            .create_stream()
            .ok_or_else(|| AsrPortError::runtime("Failed to create speaker embedding stream"))?;
        stream.accept_waveform(SAMPLE_RATE, samples);
        stream.input_finished();

        if !self.extractor.is_ready(&stream) {
            return Ok(None);
        }

        Ok(self.extractor.compute(&stream))
    }
}

pub fn run_speaker_diarization(
    samples: &[f32],
    segmentation_model: &Path,
    embedding_model: &Path,
) -> Result<Vec<SpeakerDiarizationSegment>, AsrPortError> {
    let diarization_config = OfflineSpeakerDiarizationConfig {
        segmentation: OfflineSpeakerSegmentationModelConfig {
            pyannote: OfflineSpeakerSegmentationPyannoteModelConfig {
                model: Some(segmentation_model.to_string_lossy().into_owned()),
                window_shift_ratio: 0.1,
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

    let diarizer = OfflineSpeakerDiarization::create(&diarization_config).ok_or_else(|| {
        AsrPortError::new(
            AsrPortErrorKind::Model,
            "Failed to create offline speaker diarizer",
        )
    })?;
    let result = diarizer
        .process(samples)
        .ok_or_else(|| AsrPortError::runtime("Speaker diarization returned no result"))?;

    Ok(result
        .sort_by_start_time()
        .into_iter()
        .map(|segment| SpeakerDiarizationSegment {
            start: segment.start,
            end: segment.end,
            speaker: segment.speaker,
        })
        .collect())
}

fn load_profile_sample_wav(file_path: &str) -> Result<Vec<f32>, AsrPortError> {
    let mut reader = hound::WavReader::open(file_path).map_err(|error| {
        AsrPortError::new(
            AsrPortErrorKind::FileSystem,
            format!("Failed to open speaker sample {file_path}: {error}"),
        )
    })?;
    let spec = reader.spec();
    if spec.channels != 1 {
        return Err(AsrPortError::invalid_request(format!(
            "Speaker sample must be mono wav: {file_path}"
        )));
    }
    if spec.sample_rate != SAMPLE_RATE as u32 {
        return Err(AsrPortError::invalid_request(format!(
            "Speaker sample must be 16k wav but got {} Hz: {file_path}",
            spec.sample_rate
        )));
    }

    match spec.sample_format {
        SampleFormat::Int => {
            if spec.bits_per_sample != 16 {
                return Err(AsrPortError::invalid_request(format!(
                    "Speaker sample must be 16-bit PCM wav: {file_path}"
                )));
            }
            reader
                .samples::<i16>()
                .map(|sample| {
                    sample
                        .map(|value| value as f32 / i16::MAX as f32)
                        .map_err(|error| {
                            AsrPortError::new(
                                AsrPortErrorKind::FileSystem,
                                format!("Failed to read speaker sample {file_path}: {error}"),
                            )
                        })
                })
                .collect()
        }
        SampleFormat::Float => reader
            .samples::<f32>()
            .map(|sample| {
                sample.map_err(|error| {
                    AsrPortError::new(
                        AsrPortErrorKind::FileSystem,
                        format!("Failed to read speaker sample {file_path}: {error}"),
                    )
                })
            })
            .collect(),
    }
}
