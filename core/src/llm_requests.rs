use crate::domain::LlmProvider;
use crate::llm_tasks::{
    LlmProviderStrategy, LlmSegmentInput, LlmTaskType, SummarySegmentInput, SummaryTemplateConfig,
};
use crate::llm_usage::{LlmGenerateSource, LlmUsageCategory, TokenUsage};
use crate::transcript::TranscriptSegment;
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryRecordPayload {
    pub template_id: String,
    pub content: String,
    pub generated_at: String,
    pub source_fingerprint: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct HistorySummaryPayload {
    pub active_template_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record: Option<TranscriptSummaryRecordPayload>,
}
