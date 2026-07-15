use crate::domain::{BuiltinLlmProvider, LlmProvider};
use serde::{Deserialize, Serialize};

mod planning;
mod polish;
mod service;
mod structured;
mod summary;
mod translate;

pub use planning::*;
pub use polish::*;
pub use service::*;
pub use structured::*;
pub use summary::*;
pub use translate::*;

#[cfg(feature = "specta")]
use specta::Type;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmProviderStrategy {
    OpenAi,
    OpenAiResponses,
    #[serde(rename = "azure_openai")]
    AzureOpenAi,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    MoonshotAi,
    MoonshotCn,
    Xiaomi,
    Kimi,
    SiliconFlow,
    Qwen,
    QwenPortal,
    MinimaxGlobal,
    MinimaxCn,
    OpenRouter,
    LmStudio,
    Groq,
    XAi,
    MistralAi,
    Perplexity,
    Volcengine,
    Chatglm,
    Copilot,
    #[serde(rename = "google_translate")]
    GoogleTranslate,
    #[serde(rename = "google_translate_free")]
    GoogleTranslateFree,
    OpenAiCompatible,
    OpenAiCompatibleCustomPath,
}

impl LlmProviderStrategy {
    pub fn from_provider(provider: &LlmProvider) -> Self {
        match provider {
            LlmProvider::Custom(_) => Self::OpenAiCompatible,
            LlmProvider::Builtin(b) => match b {
                BuiltinLlmProvider::OpenAi => Self::OpenAi,
                BuiltinLlmProvider::OpenAiResponses => Self::OpenAiResponses,
                BuiltinLlmProvider::AzureOpenai => Self::AzureOpenAi,
                BuiltinLlmProvider::Anthropic => Self::Anthropic,
                BuiltinLlmProvider::Gemini => Self::Gemini,
                BuiltinLlmProvider::Ollama => Self::Ollama,
                BuiltinLlmProvider::MoonshotAi => Self::MoonshotAi,
                BuiltinLlmProvider::MoonshotCn => Self::MoonshotCn,
                BuiltinLlmProvider::Xiaomi => Self::Xiaomi,
                BuiltinLlmProvider::Perplexity => Self::Perplexity,
                BuiltinLlmProvider::Copilot => Self::Copilot,
                BuiltinLlmProvider::Volcengine => Self::OpenAiCompatibleCustomPath,
                BuiltinLlmProvider::GoogleTranslate => Self::GoogleTranslate,
                BuiltinLlmProvider::GoogleTranslateFree => Self::GoogleTranslateFree,
                _ => Self::OpenAiCompatible,
            },
        }
    }
}

impl<'de> Deserialize<'de> for LlmProviderStrategy {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Ok(match value.as_str() {
            "open_ai" => Self::OpenAi,
            "open_ai_responses" | "openai_responses" => Self::OpenAiResponses,
            "azure_openai" => Self::AzureOpenAi,
            "anthropic" => Self::Anthropic,
            "gemini" => Self::Gemini,
            "ollama" => Self::Ollama,
            "deep_seek" => Self::DeepSeek,
            "kimi" => Self::Kimi,
            "silicon_flow" => Self::SiliconFlow,
            "qwen" => Self::Qwen,
            "qwen_portal" => Self::QwenPortal,
            "minimax_global" => Self::MinimaxGlobal,
            "minimax_cn" => Self::MinimaxCn,
            "openrouter" | "open_router" => Self::OpenRouter,
            "lm_studio" => Self::LmStudio,
            "groq" => Self::Groq,
            "x_ai" => Self::XAi,
            "mistral_ai" => Self::MistralAi,
            "perplexity" => Self::Perplexity,
            "volcengine" => Self::Volcengine,
            "chatglm" => Self::Chatglm,
            "copilot" | "github_copilot" => Self::Copilot,
            "google_translate" => Self::GoogleTranslate,
            "google_translate_free" => Self::GoogleTranslateFree,
            "open_ai_compatible" | "openai_compatible" => Self::OpenAiCompatible,
            "open_ai_compatible_custom_path" | "openai_compatible_custom_path" => {
                Self::OpenAiCompatibleCustomPath
            }
            _ => Self::OpenAiCompatible,
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum LlmTaskType {
    Polish,
    Translate,
    Summary,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmSegmentInput {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct SummarySegmentInput {
    pub id: String,
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub is_final: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct PolishedSegment {
    pub id: String,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranslatedSegment {
    pub id: String,
    pub translation: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct TranscriptSummaryResult {
    pub template_id: String,
    pub content: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskProgressPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub completed_chunks: u32,
    pub total_chunks: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskChunkPayload<T> {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub chunk_index: u32,
    pub total_chunks: u32,
    pub items: Vec<T>,
}

#[derive(Serialize, Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmTaskTextPayload {
    pub task_id: String,
    pub task_type: LlmTaskType,
    pub text: String,
    pub delta: String,
    pub reset: bool,
}

fn task_label(task_type: LlmTaskType) -> &'static str {
    match task_type {
        LlmTaskType::Polish => "polish",
        LlmTaskType::Translate => "translate",
        LlmTaskType::Summary => "summary",
    }
}

pub fn chunk_error(
    task_type: LlmTaskType,
    chunk_number: usize,
    error: impl Into<String>,
) -> String {
    format!(
        "{} chunk {} failed: {}",
        task_label(task_type),
        chunk_number,
        error.into()
    )
}

fn validate_segment_ids<T, GetId>(
    parsed: &[T],
    expected: &[LlmSegmentInput],
    task_type: LlmTaskType,
    chunk_number: usize,
    get_id: GetId,
) -> Result<(), String>
where
    GetId: Fn(&T) -> &str,
{
    if parsed.len() != expected.len() {
        return Err(chunk_error(
            task_type,
            chunk_number,
            format!(
                "expected {} objects but received {}",
                expected.len(),
                parsed.len()
            ),
        ));
    }

    for (index, (actual, expected_segment)) in parsed.iter().zip(expected.iter()).enumerate() {
        let actual_id = get_id(actual);
        if actual_id != expected_segment.id {
            return Err(chunk_error(
                task_type,
                chunk_number,
                format!(
                    "segment {} expected id '{}' but received '{}'",
                    index + 1,
                    expected_segment.id,
                    actual_id
                ),
            ));
        }
    }

    Ok(())
}
