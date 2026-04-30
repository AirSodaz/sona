use super::model_config::ModelFileConfig;
use serde::{Deserialize, Serialize};

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
    pub model_type: String,
    pub file_config: Option<ModelFileConfig>,
    pub hotwords: Option<String>,
    pub speaker_processing: Option<crate::speaker::SpeakerProcessingConfig>,
    pub normalization_options: TranscriptNormalizationOptions,
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
pub struct TranscriptNormalizationOptions {
    pub enable_timeline: bool,
}

impl Default for TranscriptNormalizationOptions {
    fn default() -> Self {
        Self {
            enable_timeline: false,
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
}
