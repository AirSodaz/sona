use sona_core::llm::requests::{
    LlmConfig, PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::llm::tasks::{
    LlmProviderStrategy, LlmSegmentInput, SummarySegmentInput, SummaryTemplateConfig,
};
use sona_core::ports::asr::{
    AsrEngine, AsrMode, BatchSegmentationMode, OnlineAsrProviderRequest, VolcengineDoubaoAsrConfig,
};
use sona_core::runtime::environment::{RuntimePathKind, RuntimePathStatus};

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRuntimePathKind {
    File,
    Directory,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiRuntimePathStatus {
    pub path: String,
    pub kind: FfiRuntimePathKind,
    pub error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiAsrEngine {
    LocalSherpa,
    Online,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiAsrMode {
    Streaming,
    Batch,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiBatchSegmentationMode {
    Vad,
    Whole,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiOnlineAsrProviderRequest {
    pub provider_id: String,
    pub profile_id: String,
    pub config_json: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiVolcengineDoubaoAsrConfig {
    pub api_key: String,
    pub streaming_endpoint: String,
    pub streaming_resource_id: String,
    pub batch_endpoint: String,
    pub batch_resource_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmProviderStrategy {
    OpenAi,
    OpenAiResponses,
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
    GoogleTranslate,
    GoogleTranslateFree,
    OpenAiCompatible,
    OpenAiCompatibleCustomPath,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiLlmConfig {
    pub provider_id: String,
    pub strategy: FfiLlmProviderStrategy,
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

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmSegmentInput {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSummarySegmentInput {
    pub id: String,
    pub text: String,
    pub start: f32,
    pub end: f32,
    pub is_final: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiSummaryTemplateConfig {
    pub id: String,
    pub name: String,
    pub instructions: String,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiPolishSegmentsRequest {
    pub task_id: String,
    pub config: FfiLlmConfig,
    pub segments: Vec<FfiLlmSegmentInput>,
    pub chunk_size: Option<u64>,
    pub context: Option<String>,
    pub keywords: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiTranslateSegmentsRequest {
    pub task_id: String,
    pub config: FfiLlmConfig,
    pub segments: Vec<FfiLlmSegmentInput>,
    pub chunk_size: Option<u64>,
    pub target_language: String,
    pub target_language_name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, uniffi::Record)]
pub struct FfiSummarizeTranscriptRequest {
    pub task_id: String,
    pub config: FfiLlmConfig,
    pub template: FfiSummaryTemplateConfig,
    pub segments: Vec<FfiSummarySegmentInput>,
    pub chunk_char_budget: Option<u64>,
}

pub fn runtime_path_status_to_ffi(status: RuntimePathStatus) -> FfiRuntimePathStatus {
    FfiRuntimePathStatus {
        path: status.path,
        kind: runtime_path_kind_to_ffi(status.kind),
        error: status.error,
    }
}

fn runtime_path_kind_to_ffi(kind: RuntimePathKind) -> FfiRuntimePathKind {
    match kind {
        RuntimePathKind::File => FfiRuntimePathKind::File,
        RuntimePathKind::Directory => FfiRuntimePathKind::Directory,
        RuntimePathKind::Missing => FfiRuntimePathKind::Missing,
        RuntimePathKind::Unknown => FfiRuntimePathKind::Unknown,
    }
}

#[expect(dead_code)]
pub fn asr_engine_to_ffi(engine: AsrEngine) -> FfiAsrEngine {
    match engine {
        AsrEngine::LocalSherpa => FfiAsrEngine::LocalSherpa,
        AsrEngine::Online => FfiAsrEngine::Online,
    }
}

#[expect(dead_code)]
pub fn asr_mode_to_ffi(mode: AsrMode) -> FfiAsrMode {
    match mode {
        AsrMode::Streaming => FfiAsrMode::Streaming,
        AsrMode::Batch => FfiAsrMode::Batch,
    }
}

pub fn batch_segmentation_mode_to_ffi(mode: BatchSegmentationMode) -> FfiBatchSegmentationMode {
    match mode {
        BatchSegmentationMode::Vad => FfiBatchSegmentationMode::Vad,
        BatchSegmentationMode::Whole => FfiBatchSegmentationMode::Whole,
    }
}

pub fn online_asr_provider_request_to_ffi(
    request: OnlineAsrProviderRequest,
) -> FfiOnlineAsrProviderRequest {
    FfiOnlineAsrProviderRequest {
        provider_id: request.provider_id,
        profile_id: request.profile_id,
        config_json: request.config.to_string(),
    }
}

pub fn volcengine_doubao_asr_config_to_ffi(
    config: VolcengineDoubaoAsrConfig,
) -> FfiVolcengineDoubaoAsrConfig {
    FfiVolcengineDoubaoAsrConfig {
        api_key: config.api_key,
        streaming_endpoint: config.streaming_endpoint,
        streaming_resource_id: config.streaming_resource_id,
        batch_endpoint: config.batch_endpoint,
        batch_resource_id: config.batch_resource_id,
    }
}

pub fn llm_provider_strategy_to_ffi(strategy: LlmProviderStrategy) -> FfiLlmProviderStrategy {
    match strategy {
        LlmProviderStrategy::OpenAi => FfiLlmProviderStrategy::OpenAi,
        LlmProviderStrategy::OpenAiResponses => FfiLlmProviderStrategy::OpenAiResponses,
        LlmProviderStrategy::AzureOpenAi => FfiLlmProviderStrategy::AzureOpenAi,
        LlmProviderStrategy::Anthropic => FfiLlmProviderStrategy::Anthropic,
        LlmProviderStrategy::Gemini => FfiLlmProviderStrategy::Gemini,
        LlmProviderStrategy::Ollama => FfiLlmProviderStrategy::Ollama,
        LlmProviderStrategy::DeepSeek => FfiLlmProviderStrategy::DeepSeek,
        LlmProviderStrategy::MoonshotAi => FfiLlmProviderStrategy::MoonshotAi,
        LlmProviderStrategy::MoonshotCn => FfiLlmProviderStrategy::MoonshotCn,
        LlmProviderStrategy::Xiaomi => FfiLlmProviderStrategy::Xiaomi,
        LlmProviderStrategy::Kimi => FfiLlmProviderStrategy::Kimi,
        LlmProviderStrategy::SiliconFlow => FfiLlmProviderStrategy::SiliconFlow,
        LlmProviderStrategy::Qwen => FfiLlmProviderStrategy::Qwen,
        LlmProviderStrategy::QwenPortal => FfiLlmProviderStrategy::QwenPortal,
        LlmProviderStrategy::MinimaxGlobal => FfiLlmProviderStrategy::MinimaxGlobal,
        LlmProviderStrategy::MinimaxCn => FfiLlmProviderStrategy::MinimaxCn,
        LlmProviderStrategy::OpenRouter => FfiLlmProviderStrategy::OpenRouter,
        LlmProviderStrategy::LmStudio => FfiLlmProviderStrategy::LmStudio,
        LlmProviderStrategy::Groq => FfiLlmProviderStrategy::Groq,
        LlmProviderStrategy::XAi => FfiLlmProviderStrategy::XAi,
        LlmProviderStrategy::MistralAi => FfiLlmProviderStrategy::MistralAi,
        LlmProviderStrategy::Perplexity => FfiLlmProviderStrategy::Perplexity,
        LlmProviderStrategy::Volcengine => FfiLlmProviderStrategy::Volcengine,
        LlmProviderStrategy::Chatglm => FfiLlmProviderStrategy::Chatglm,
        LlmProviderStrategy::Copilot => FfiLlmProviderStrategy::Copilot,
        LlmProviderStrategy::GoogleTranslate => FfiLlmProviderStrategy::GoogleTranslate,
        LlmProviderStrategy::GoogleTranslateFree => FfiLlmProviderStrategy::GoogleTranslateFree,
        LlmProviderStrategy::OpenAiCompatible => FfiLlmProviderStrategy::OpenAiCompatible,
        LlmProviderStrategy::OpenAiCompatibleCustomPath => {
            FfiLlmProviderStrategy::OpenAiCompatibleCustomPath
        }
    }
}

pub fn llm_config_to_ffi(config: LlmConfig) -> FfiLlmConfig {
    FfiLlmConfig {
        provider_id: config.provider.as_str(),
        strategy: llm_provider_strategy_to_ffi(config.strategy),
        base_url: config.base_url,
        api_key: config.api_key,
        model: config.model,
        api_path: config.api_path,
        api_version: config.api_version,
        temperature: config.temperature,
        reasoning_enabled: config.reasoning_enabled,
        reasoning_level: config.reasoning_level,
        timeout_seconds: config.timeout_seconds,
    }
}

fn llm_segment_input_to_ffi(segment: LlmSegmentInput) -> FfiLlmSegmentInput {
    FfiLlmSegmentInput {
        id: segment.id,
        text: segment.text,
    }
}

fn summary_segment_input_to_ffi(segment: SummarySegmentInput) -> FfiSummarySegmentInput {
    FfiSummarySegmentInput {
        id: segment.id,
        text: segment.text,
        start: segment.start,
        end: segment.end,
        is_final: segment.is_final,
    }
}

fn summary_template_config_to_ffi(template: SummaryTemplateConfig) -> FfiSummaryTemplateConfig {
    FfiSummaryTemplateConfig {
        id: template.id,
        name: template.name,
        instructions: template.instructions,
    }
}

pub fn polish_segments_request_to_ffi(request: PolishSegmentsRequest) -> FfiPolishSegmentsRequest {
    FfiPolishSegmentsRequest {
        task_id: request.task_id,
        config: llm_config_to_ffi(request.config),
        segments: request
            .segments
            .into_iter()
            .map(llm_segment_input_to_ffi)
            .collect(),
        chunk_size: request.chunk_size.map(|value| value as u64),
        context: request.context,
        keywords: request.keywords,
    }
}

pub fn translate_segments_request_to_ffi(
    request: TranslateSegmentsRequest,
) -> FfiTranslateSegmentsRequest {
    FfiTranslateSegmentsRequest {
        task_id: request.task_id,
        config: llm_config_to_ffi(request.config),
        segments: request
            .segments
            .into_iter()
            .map(llm_segment_input_to_ffi)
            .collect(),
        chunk_size: request.chunk_size.map(|value| value as u64),
        target_language: request.target_language,
        target_language_name: request.target_language_name,
    }
}

pub fn summarize_transcript_request_to_ffi(
    request: SummarizeTranscriptRequest,
) -> FfiSummarizeTranscriptRequest {
    FfiSummarizeTranscriptRequest {
        task_id: request.task_id,
        config: llm_config_to_ffi(request.config),
        template: summary_template_config_to_ffi(request.template),
        segments: request
            .segments
            .into_iter()
            .map(summary_segment_input_to_ffi)
            .collect(),
        chunk_char_budget: request.chunk_char_budget.map(|value| value as u64),
    }
}
