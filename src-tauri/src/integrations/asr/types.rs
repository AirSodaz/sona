use super::postprocess::TranscriptPostprocessor;

pub use sona_core::ports::asr::{
    AsrEngine, AsrEngineConfig, AsrMode, AsrTranscriptionRequest, BatchSegmentationMode,
    OnlineAsrProviderRequest, TranscriptNormalizationOptions, TranscriptPostprocessOptions,
    TranscriptTextReplacementRule, TranscriptTextReplacementRuleSet, VolcengineDoubaoAsrConfig,
};

pub use sona_core::transcript::{
    TranscriptSegment, TranscriptTiming, TranscriptTimingLevel, TranscriptTimingSource,
    TranscriptTimingUnit, TranscriptUpdate,
};

#[derive(Debug, Clone)]
pub struct BatchTranscriptionRequest {
    pub instance_id: Option<String>,
    pub file_path: std::path::PathBuf,
    pub save_to_path: Option<std::path::PathBuf>,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub batch_segmentation_mode: BatchSegmentationMode,
    pub model_type: String,
    pub file_config: Option<sona_core::model_config::ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<sona_core::speaker::SpeakerProcessingConfig>,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocessor: TranscriptPostprocessor,
    pub gpu_acceleration: Option<String>,
}
