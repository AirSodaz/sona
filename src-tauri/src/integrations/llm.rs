pub(crate) mod commands;
pub(crate) mod jobs;
#[cfg(test)]
pub(crate) use sona_core::llm_provider_protocol::{
    GeminiModel, OpenAiModel, is_gemini_text_generation_model,
};
pub(crate) use sona_core::llm_provider_protocol::{
    build_standard_input, clean_gemini_base_url, extract_text_from_json_response,
    extract_usage_from_json_response, format_gemini_models_url, format_openai_models_urls,
    gemini_model_to_summary, join_url, openai_model_to_summary, strategy_supports_model_listing,
    strategy_uses_openai_chat_payload,
};
pub(crate) use sona_core::llm_tasks::{DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET, chunk_error};
#[cfg(test)]
pub(crate) use sona_core::llm_tasks::{
    DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET, build_summary_chunk_prompt, build_summary_finalize_prompt,
    clean_json_response, prompt_char_count, split_summary_segments,
};
pub(crate) use sona_core::llm_usage;
mod network;
mod providers;
mod streaming;
mod tasks;
#[cfg(test)]
mod tests;
mod types;

const GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRIES: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS: u64 = 5;
const GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS: [u64; GOOGLE_TRANSLATE_FREE_MAX_RETRIES] = [500, 1000];
const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";
const LLM_TASK_TEXT_EVENT: &str = "llm-task-text";
const LLM_TRANSCRIPT_JOB_UPDATE_EVENT: &str = "llm-transcript-job-update";
const LLM_USAGE_RECORDED_EVENT: &str = "llm-usage-recorded";

pub(crate) use providers::*;
pub(crate) use streaming::*;
pub(crate) use tasks::*;
pub use types::*;
