use crate::domain::LlmProvider;
pub use crate::history::{HistorySummaryPayload, TranscriptSummaryRecordPayload};
use crate::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskType, SummarySegmentInput, SummaryTemplateConfig,
};
use crate::llm::usage::{LlmGenerateSource, LlmUsageCategory, TokenUsage};
use crate::transcription::transcript::TranscriptSegment;
use serde::{Deserialize, Serialize};

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Serialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmConfig {
    pub provider: LlmProvider,
    pub strategy: LlmProviderStrategy,
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub api_path: Option<String>,
    pub api_version: Option<String>,
    pub temperature: Option<f32>,
    pub reasoning_enabled: Option<bool>,
    pub reasoning_level: Option<String>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLlmConfig {
    provider: LlmProvider,
    strategy: Option<LlmProviderStrategy>,
    base_url: String,
    api_key: String,
    model: String,
    api_path: Option<String>,
    api_version: Option<String>,
    temperature: Option<f32>,
    reasoning_enabled: Option<bool>,
    reasoning_level: Option<String>,
    timeout_seconds: Option<u64>,
}

impl<'de> Deserialize<'de> for LlmConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let raw = RawLlmConfig::deserialize(deserializer)?;
        Ok(Self {
            strategy: raw
                .strategy
                .unwrap_or_else(|| LlmProviderStrategy::from_provider(&raw.provider)),
            provider: raw.provider,
            base_url: raw.base_url,
            api_key: raw.api_key,
            model: raw.model,
            api_path: raw.api_path,
            api_version: raw.api_version,
            temperature: raw.temperature,
            reasoning_enabled: raw.reasoning_enabled,
            reasoning_level: raw.reasoning_level,
            timeout_seconds: raw.timeout_seconds,
        })
    }
}

pub fn validate_llm_config(config: &LlmConfig) -> Result<(), String> {
    if config.model.trim().is_empty() {
        return Err("Model name cannot be empty".to_string());
    }

    Ok(())
}

pub fn validate_task_request(task_id: &str, config: &LlmConfig) -> Result<(), String> {
    if task_id.trim().is_empty() {
        return Err("Task ID cannot be empty".to_string());
    }

    validate_llm_config(config)
}

pub fn validate_llm_generate_request(request: &LlmGenerateRequest) -> Result<(), String> {
    validate_llm_config(&request.config)?;

    if request.input.trim().is_empty() {
        return Err("Input cannot be empty".to_string());
    }

    Ok(())
}

pub fn validate_polish_segments_request(request: &PolishSegmentsRequest) -> Result<(), String> {
    validate_task_request(&request.task_id, &request.config)?;

    if matches!(
        request.config.strategy,
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree
    ) {
        return Err("Google Translate does not support transcript polishing".to_string());
    }

    Ok(())
}

pub fn validate_translate_segments_request(
    request: &TranslateSegmentsRequest,
) -> Result<(), String> {
    validate_task_request(&request.task_id, &request.config)?;

    if request.target_language.trim().is_empty() {
        return Err("Target language cannot be empty".to_string());
    }

    Ok(())
}

pub fn validate_summarize_transcript_request(
    request: &SummarizeTranscriptRequest,
) -> Result<(), String> {
    validate_task_request(&request.task_id, &request.config)?;
    crate::llm::tasks::validate_summary_strategy(request.config.strategy)
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmGenerateRequest {
    pub config: LlmConfig,
    pub input: String,
    pub source: Option<LlmGenerateSource>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmUsageEventPayload {
    pub occurred_at: String,
    pub provider: LlmProvider,
    pub model: String,
    pub category: LlmUsageCategory,
    pub usage: Option<TokenUsage>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmModelsRequest {
    pub provider: LlmProvider,
    #[serde(default)]
    pub strategy: Option<LlmProviderStrategy>,
    pub base_url: String,
    pub api_key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct PolishSegmentsRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub segments: Vec<LlmSegmentInput>,
    pub chunk_size: Option<usize>,
    pub context: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranslateSegmentsRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub segments: Vec<LlmSegmentInput>,
    pub chunk_size: Option<usize>,
    pub target_language: String,
    pub target_language_name: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SummarizeTranscriptRequest {
    pub task_id: String,
    pub config: LlmConfig,
    pub template: SummaryTemplateConfig,
    pub segments: Vec<SummarySegmentInput>,
    pub chunk_char_budget: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptLlmJobRequest {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub job_history_id: Option<String>,
    pub config: LlmConfig,
    pub segments: Vec<TranscriptSegment>,
    pub target_language: Option<String>,
    pub target_language_name: Option<String>,
    pub context: Option<String>,
    pub keywords: Option<String>,
    pub template: Option<SummaryTemplateConfig>,
    pub chunk_size: Option<usize>,
    pub chunk_char_budget: Option<usize>,
}
