mod commands;
mod jobs;
#[cfg(test)]
pub(crate) use sona_core::llm::provider_protocol::{join_url, strategy_supports_model_listing};
#[cfg(test)]
pub(crate) use sona_core::llm::tasks::{
    build_summary_chunk_prompt, build_summary_finalize_prompt, clean_json_response,
    prompt_char_count, split_summary_segments,
};
pub(crate) use sona_core::llm::usage::UsageRecord;
#[cfg(test)]
mod network;
mod providers;
#[cfg(test)]
mod tasks;
#[cfg(test)]
mod tests;
mod types;

const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";
const LLM_TASK_TEXT_EVENT: &str = "llm-task-text";
const LLM_TRANSCRIPT_JOB_UPDATE_EVENT: &str = "llm-transcript-job-update";
const LLM_USAGE_RECORDED_EVENT: &str = "llm-usage-recorded";

pub(crate) use commands::{
    complete_llm_command, describe_llm_model_with_models_dir, generate_llm_text_command,
    list_llm_models_with_models_dir, polish_transcript_segments_command,
    summarize_transcript_command, translate_transcript_segments_command,
};
pub(crate) use jobs::run_transcript_llm_job_command;
pub(crate) use providers::*;
#[cfg(test)]
pub(crate) use tasks::*;
pub use types::*;
