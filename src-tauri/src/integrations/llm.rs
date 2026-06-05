pub(crate) mod commands;
pub(crate) mod jobs;
#[path = "llm_usage.rs"]
pub(crate) mod llm_usage;
mod network;
mod providers;
mod streaming;
mod tasks;
#[cfg(test)]
mod tests;
mod types;

const DEFAULT_SEGMENT_CHUNK_SIZE: usize = 30;
const DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET: usize = 32_000;
const DEFAULT_SEGMENT_PROMPT_CHAR_BUDGET: usize = DEFAULT_SEGMENT_CONTEXT_CHAR_BUDGET / 2;
const GOOGLE_TRANSLATE_FREE_MAX_CONCURRENCY: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRIES: usize = 2;
const GOOGLE_TRANSLATE_FREE_MAX_RETRY_AFTER_SECS: u64 = 5;
const GOOGLE_TRANSLATE_FREE_RETRY_DELAYS_MS: [u64; GOOGLE_TRANSLATE_FREE_MAX_RETRIES] = [500, 1000];
const DEFAULT_SUMMARY_CHUNK_CHAR_BUDGET: usize = 6000;
const MIN_SUMMARY_CHUNK_CHAR_BUDGET: usize = 1200;
const LLM_TASK_PROGRESS_EVENT: &str = "llm-task-progress";
const LLM_TASK_CHUNK_EVENT: &str = "llm-task-chunk";
const LLM_TASK_TEXT_EVENT: &str = "llm-task-text";
const LLM_TRANSCRIPT_JOB_UPDATE_EVENT: &str = "llm-transcript-job-update";
const LLM_USAGE_RECORDED_EVENT: &str = "llm-usage-recorded";

pub(crate) use providers::*;
pub(crate) use streaming::*;
pub(crate) use tasks::*;
pub use types::*;
