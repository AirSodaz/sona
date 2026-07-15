use serde::Serialize;
use sona_core::llm::tasks::{
    LlmTaskChunkPayload, LlmTaskProgressPayload, LlmTaskResult, LlmTaskSummaryChunkPayload,
    LlmTaskTextPayload, LlmTaskType,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, uniffi::Enum)]
pub enum FfiLlmTaskType {
    Polish,
    Translate,
    Summary,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmTaskProgress {
    pub task_id: String,
    pub task_type: FfiLlmTaskType,
    pub completed_chunks: u64,
    pub total_chunks: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmTaskChunk {
    pub task_id: String,
    pub task_type: FfiLlmTaskType,
    pub chunk_index: u64,
    pub total_chunks: u64,
    pub items_json: Option<String>,
    pub text: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmTaskText {
    pub task_id: String,
    pub task_type: FfiLlmTaskType,
    pub text: String,
    pub delta: String,
    pub reset: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, uniffi::Record)]
pub struct FfiLlmTaskFinal {
    pub task_id: String,
    pub task_type: FfiLlmTaskType,
    pub result_json: String,
}

pub fn llm_task_type_to_ffi(task_type: LlmTaskType) -> FfiLlmTaskType {
    match task_type {
        LlmTaskType::Polish => FfiLlmTaskType::Polish,
        LlmTaskType::Translate => FfiLlmTaskType::Translate,
        LlmTaskType::Summary => FfiLlmTaskType::Summary,
    }
}

pub fn llm_task_progress_to_ffi(payload: LlmTaskProgressPayload) -> FfiLlmTaskProgress {
    FfiLlmTaskProgress {
        task_id: payload.task_id,
        task_type: llm_task_type_to_ffi(payload.task_type),
        completed_chunks: u64::from(payload.completed_chunks),
        total_chunks: u64::from(payload.total_chunks),
    }
}

pub fn llm_task_items_chunk_to_ffi<T>(
    payload: LlmTaskChunkPayload<T>,
) -> Result<FfiLlmTaskChunk, serde_json::Error>
where
    T: Serialize,
{
    Ok(FfiLlmTaskChunk {
        task_id: payload.task_id,
        task_type: llm_task_type_to_ffi(payload.task_type),
        chunk_index: u64::from(payload.chunk_index),
        total_chunks: u64::from(payload.total_chunks),
        items_json: Some(serde_json::to_string(&payload.items)?),
        text: None,
    })
}

pub fn llm_task_summary_chunk_to_ffi(payload: LlmTaskSummaryChunkPayload) -> FfiLlmTaskChunk {
    FfiLlmTaskChunk {
        task_id: payload.task_id,
        task_type: FfiLlmTaskType::Summary,
        chunk_index: u64::from(payload.chunk_index),
        total_chunks: u64::from(payload.total_chunks),
        items_json: None,
        text: Some(payload.text),
    }
}

pub fn llm_task_text_to_ffi(payload: LlmTaskTextPayload) -> FfiLlmTaskText {
    FfiLlmTaskText {
        task_id: payload.task_id,
        task_type: llm_task_type_to_ffi(payload.task_type),
        text: payload.text,
        delta: payload.delta,
        reset: payload.reset,
    }
}

pub fn llm_task_result_to_ffi(
    task_id: String,
    result: LlmTaskResult,
) -> Result<FfiLlmTaskFinal, serde_json::Error> {
    let (task_type, result_json) = match result {
        LlmTaskResult::Polish(items) => (FfiLlmTaskType::Polish, serde_json::to_string(&items)?),
        LlmTaskResult::Translate(items) => {
            (FfiLlmTaskType::Translate, serde_json::to_string(&items)?)
        }
        LlmTaskResult::Summary(summary) => {
            (FfiLlmTaskType::Summary, serde_json::to_string(&summary)?)
        }
    };
    Ok(FfiLlmTaskFinal {
        task_id,
        task_type,
        result_json,
    })
}
