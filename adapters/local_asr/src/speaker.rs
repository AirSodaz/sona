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

    fn compute_embedding_for_samples(
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
