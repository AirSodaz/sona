use super::network::validate_llm_api_host;
use super::*;

#[cfg(test)]
pub(crate) use sona_core::llm_tasks::plan_segment_task_chunks;
pub(crate) use sona_core::llm_tasks::{
    BufferedSegmentTaskConfig, SegmentTaskContext, StreamingSegmentTaskConfig, build_polish_prompt,
    build_translate_prompt, parse_polish_chunk, parse_translate_chunk, run_segment_task,
    run_streaming_segment_task, run_summary_task, validate_summary_strategy,
};
#[cfg(test)]
pub(crate) use sona_online_llm::{
    GoogleTranslateFreeAttemptError, parse_google_translate_free_retry_after,
};
pub(crate) use sona_online_llm::{
    execute_google_translate_free_request, fetch_google_translate_free_translation,
    run_google_translate_free_requests_in_order,
};

pub(crate) fn validate_llm_config(config: &LlmConfig) -> Result<(), String> {
    if config.model.trim().is_empty() {
        return Err("Model name cannot be empty".to_string());
    }

    validate_llm_api_host(&config.base_url)?;

    Ok(())
}

pub(crate) fn validate_task_request(task_id: &str, config: &LlmConfig) -> Result<(), String> {
    if task_id.trim().is_empty() {
        return Err("Task ID cannot be empty".to_string());
    }

    validate_llm_config(config)
}
