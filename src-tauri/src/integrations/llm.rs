pub(crate) mod commands;
pub(crate) mod jobs;
#[cfg(test)]
pub(crate) use sona_core::llm::provider_protocol::{
    GeminiModel, OpenAiModel, clean_gemini_base_url, extract_text_from_json_response,
    format_gemini_models_url, format_openai_models_urls, gemini_model_to_summary,
    is_gemini_text_generation_model, join_url, openai_model_to_summary,
    strategy_supports_model_listing,
};
pub(crate) use sona_core::llm::streaming_protocol::StreamTextAccumulator;
#[cfg(test)]
pub(crate) use sona_core::llm::streaming_protocol::{
    OpenAiChatPayloadConfig, build_openai_chat_payload,
};
pub(crate) use sona_core::llm::tasks::{DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET, chunk_error};
#[cfg(test)]
pub(crate) use sona_core::llm::tasks::{
    DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET, build_summary_chunk_prompt, build_summary_finalize_prompt,
    clean_json_response, prompt_char_count, split_summary_segments,
};
pub(crate) use sona_core::llm::usage;
mod network;
mod providers;
mod streaming;
mod tasks;
#[cfg(test)]
mod tests;
mod types;

const GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY: usize = 2;
const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";
const LLM_TASK_TEXT_EVENT: &str = "llm-task-text";
const LLM_TRANSCRIPT_JOB_UPDATE_EVENT: &str = "llm-transcript-job-update";
const LLM_USAGE_RECORDED_EVENT: &str = "llm-usage-recorded";

pub(crate) use providers::*;
pub(crate) use streaming::*;
pub(crate) use tasks::*;
pub use types::*;
