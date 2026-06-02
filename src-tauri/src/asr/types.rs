use super::model_config::ModelFileConfig;
use super::postprocess::TranscriptPostprocessor;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AsrEngine {
    LocalSherpa,
    Online,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum AsrMode {
    Streaming,
    Offline,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum BatchSegmentationMode {
    #[default]
    Vad,
    Whole,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AsrTranscriptionRequest {
    pub mode: AsrMode,
    pub language: String,
    pub enable_itn: bool,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocess_options: TranscriptPostprocessOptions,
    pub hotwords: Option<String>,

    #[serde(flatten)]
    pub engine_config: AsrEngineConfig,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "engine", rename_all = "kebab-case")]
pub enum AsrEngineConfig {
    LocalSherpa {
        #[serde(default)]
        model_id: Option<String>,
        model_path: String,
        num_threads: i32,
        #[serde(default)]
        punctuation_model: Option<String>,
        #[serde(default)]
        vad_model: Option<String>,
        vad_buffer: f32,
        #[serde(default)]
        batch_segmentation_mode: BatchSegmentationMode,
        model_type: String,
        #[serde(default)]
        file_config: Option<ModelFileConfig>,
        #[serde(default)]
        gpu_acceleration: Option<String>,
    },
    Online {
        #[serde(rename = "onlineProvider")]
        provider: OnlineAsrProviderRequest,
    }
}

impl AsrTranscriptionRequest {
    #[allow(clippy::too_many_arguments)]
    pub fn local_sherpa(
        mode: AsrMode,
        model_path: String,
        num_threads: i32,
        enable_itn: bool,
        language: String,
        punctuation_model: Option<String>,
        vad_model: Option<String>,
        vad_buffer: f32,
        model_type: String,
        file_config: Option<ModelFileConfig>,
        hotwords: Option<String>,
        normalization_options: TranscriptNormalizationOptions,
        postprocess_options: TranscriptPostprocessOptions,
        gpu_acceleration: Option<String>,
    ) -> Self {
        Self {
            mode,
            language,
            enable_itn,
            normalization_options,
            postprocess_options,
            hotwords,
            engine_config: AsrEngineConfig::LocalSherpa {
                model_id: None,
                model_path,
                num_threads,
                punctuation_model,
                vad_model,
                vad_buffer,
                batch_segmentation_mode: BatchSegmentationMode::Vad,
                model_type,
                file_config,
                gpu_acceleration,
            },
        }
    }

    pub fn engine(&self) -> AsrEngine {
        match &self.engine_config {
            AsrEngineConfig::LocalSherpa { .. } => AsrEngine::LocalSherpa,
            AsrEngineConfig::Online { .. } => AsrEngine::Online,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct OnlineAsrProviderRequest {
    pub provider_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub config: Value,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VolcengineDoubaoAsrConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub streaming_endpoint: String,
    #[serde(default)]
    pub streaming_resource_id: String,
    #[serde(default)]
    pub batch_endpoint: String,
    #[serde(default)]
    pub batch_resource_id: String,
}

#[derive(Debug, Clone)]
pub struct BatchTranscriptionRequest {
    pub file_path: String,
    pub save_to_path: Option<String>,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub batch_segmentation_mode: BatchSegmentationMode,
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    pub normalization_options: TranscriptNormalizationOptions,
    pub postprocessor: TranscriptPostprocessor,
    pub gpu_acceleration: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingLevel {
    Token,
    Segment,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TranscriptTimingSource {
    Model,
    Derived,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTimingUnit {
    pub text: String,
    pub start: f64,
    pub end: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTiming {
    pub level: TranscriptTimingLevel,
    pub source: TranscriptTimingSource,
    pub units: Vec<TranscriptTimingUnit>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[derive(Default)]
pub struct TranscriptNormalizationOptions {
    pub enable_timeline: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTextReplacementRule {
    #[serde(default)]
    pub from: String,
    #[serde(default)]
    pub to: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptTextReplacementRuleSet {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub ignore_case: bool,
    #[serde(default)]
    pub rules: Vec<TranscriptTextReplacementRule>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptPostprocessOptions {
    #[serde(default)]
    pub text_replacement_sets: Vec<TranscriptTextReplacementRuleSet>,
    #[serde(default = "default_drop_final_dot_segments")]
    pub drop_final_dot_segments: bool,
}

fn default_drop_final_dot_segments() -> bool {
    true
}

impl Default for TranscriptPostprocessOptions {
    fn default() -> Self {
        Self {
            text_replacement_sets: Vec::new(),
            drop_final_dot_segments: default_drop_final_dot_segments(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptUpdate {
    pub remove_ids: Vec<String>,
    pub upsert_segments: Vec<TranscriptSegment>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSegment {
    pub id: String,
    pub text: String,
    pub start: f64,
    pub end: f64,
    pub is_final: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timing: Option<TranscriptTiming>,
    // Legacy raw fields are still written for compatibility with older
    // persisted transcript records and upgrade paths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokens: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamps: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub durations: Option<Vec<f32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker: Option<crate::speaker::SpeakerTag>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub speaker_attribution: Option<crate::speaker::SpeakerAttribution>,
}
