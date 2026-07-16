use std::sync::{Arc, Mutex};

use sona_core::llm::jobs::{
    compute_summary_source_fingerprint, merge_polished_items_into_segments,
    merge_translated_items_into_segments, normalized_job_history_id,
    segment_inputs_from_transcript, summary_inputs_from_transcript,
};
use tauri::{AppHandle, Emitter, State};

use super::*;
use crate::integrations::asr::TranscriptSegment;
use crate::platform::history_repository::llm_helpers;
use crate::platform::history_repository::{HistoryRepositoryState, TranscriptSnapshotReason};

fn emit_transcript_job_update(
    app: &AppHandle,
    task_id: &str,
    task_type: LlmTaskType,
    job_history_id: Option<String>,
    segments: Option<Vec<TranscriptSegment>>,
    summary: Option<HistorySummaryPayload>,
    history_item: Option<crate::platform::history_repository::HistoryItemRecord>,
) -> Result<(), String> {
    app.emit(
        LLM_TRANSCRIPT_JOB_UPDATE_EVENT,
        TranscriptLlmJobResult {
            task_id: task_id.to_string(),
            task_type,
            job_history_id,
            segments,
            summary,
            history_item,
        },
    )
    .map_err(|error| error.to_string())
}

async fn create_transcript_job_snapshot(
    app: AppHandle,
    history_id: Option<&str>,
    reason: TranscriptSnapshotReason,
    segments: &[TranscriptSegment],
) -> Result<(), String> {
    if let Some(history_id) = history_id {
        let history_id = history_id.to_string();
        let segments = segments.to_vec();
        llm_helpers::run_llm_db_task_with_app(&app, move |store| {
            let repository = Arc::new(store);
            let mutation_service =
                sona_core::history::mutation_service::HistoryMutationService::new(
                    repository.clone(),
                );
            llm_helpers::create_llm_transcript_snapshot_record(
                repository.as_ref(),
                &mutation_service,
                &history_id,
                reason,
                segments,
            )
        })
        .await?;
    }
    Ok(())
}

async fn run_segment_rewrite_job<F, Fut, TItem>(
    app: AppHandle,
    request: TranscriptLlmJobRequest,
    task_type: LlmTaskType,
    snapshot_reason: TranscriptSnapshotReason,
    run_with_observer: F,
    merge_items_into_segments: fn(Vec<TranscriptSegment>, &[TItem]) -> Vec<TranscriptSegment>,
) -> Result<TranscriptLlmJobResult, String>
where
    F: FnOnce(
        AppHandle,
        TranscriptLlmJobRequest,
        Box<dyn FnMut(&[TItem]) -> Result<(), String> + Send + 'static>,
    ) -> Fut,
    Fut: std::future::Future<Output = Result<Vec<TItem>, String>>,
    TItem: Clone + Send + Sync + 'static,
{
    let history_id = normalized_job_history_id(request.job_history_id.as_deref());
    create_transcript_job_snapshot(
        app.clone(),
        history_id.as_deref(),
        snapshot_reason,
        &request.segments,
    )
    .await?;

    let merged_segments = Arc::new(Mutex::new(request.segments.clone()));
    let observer_segments = merged_segments.clone();
    let update_app = app.clone();
    let update_task_id = request.task_id.clone();
    let update_history_id = history_id.clone();

    let callback = move |items: &[TItem]| -> Result<(), String> {
        let next_segments = {
            let mut guard = observer_segments
                .lock()
                .map_err(|error| error.to_string())?;
            *guard = merge_items_into_segments(guard.clone(), items);
            guard.clone()
        };
        emit_transcript_job_update(
            &update_app,
            &update_task_id,
            task_type,
            update_history_id.clone(),
            Some(next_segments),
            None,
            None,
        )
    };

    let task_id = request.task_id.clone();
    run_with_observer(app.clone(), request, Box::new(callback)).await?;

    let final_segments = merged_segments
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let fs_for_update = final_segments.clone();
    let history_item = if let Some(history_id) = history_id.clone() {
        llm_helpers::run_llm_db_task_with_app(&app, move |store| {
            let repository = Arc::new(store);
            let mutation_service =
                sona_core::history::mutation_service::HistoryMutationService::new(repository);
            llm_helpers::update_llm_transcript_segments_record(
                &mutation_service,
                &history_id,
                fs_for_update,
            )
        })
        .await?
    } else {
        None
    };

    emit_transcript_job_update(
        &app,
        &task_id,
        task_type,
        history_id.clone(),
        Some(final_segments.clone()),
        None,
        history_item.clone(),
    )?;

    Ok(TranscriptLlmJobResult {
        task_id,
        task_type,
        job_history_id: history_id,
        segments: Some(final_segments),
        summary: None,
        history_item,
    })
}

async fn run_translate_job(
    app: AppHandle,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    run_segment_rewrite_job(
        app,
        request,
        LlmTaskType::Translate,
        TranscriptSnapshotReason::Translate,
        |app, req, mut callback| async move {
            let llm_request = TranslateSegmentsRequest {
                task_id: req.task_id,
                config: req.config,
                segments: segment_inputs_from_transcript(&req.segments),
                chunk_size: req.chunk_size,
                target_language: req.target_language.unwrap_or_else(|| "zh".to_string()),
                target_language_name: req.target_language_name,
            };
            commands::translate_transcript_segments_with_observer(app, llm_request, move |items| {
                callback(items)
            })
            .await
        },
        merge_translated_items_into_segments,
    )
    .await
}

async fn run_polish_job(
    app: AppHandle,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    run_segment_rewrite_job(
        app,
        request,
        LlmTaskType::Polish,
        TranscriptSnapshotReason::Polish,
        |app, req, mut callback| async move {
            let llm_request = PolishSegmentsRequest {
                task_id: req.task_id,
                config: req.config,
                segments: segment_inputs_from_transcript(&req.segments),
                chunk_size: req.chunk_size,
                context: req.context,
                keywords: req.keywords,
            };
            commands::polish_transcript_segments_with_observer(app, llm_request, move |items| {
                callback(items)
            })
            .await
        },
        merge_polished_items_into_segments,
    )
    .await
}

async fn run_summary_job(
    app: AppHandle,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    let history_id = normalized_job_history_id(request.job_history_id.as_deref());
    let template = request
        .template
        .clone()
        .ok_or_else(|| "Summary template is required.".to_string())?;
    let summary_request = SummarizeTranscriptRequest {
        task_id: request.task_id.clone(),
        config: request.config.clone(),
        template,
        segments: summary_inputs_from_transcript(&request.segments),
        chunk_char_budget: request.chunk_char_budget,
    };
    let result = commands::summarize_transcript_command(app.clone(), summary_request).await?;
    let summary = HistorySummaryPayload {
        active_template_id: result.template_id.clone(),
        record: Some(TranscriptSummaryRecordPayload {
            template_id: result.template_id,
            content: result.content.trim().to_string(),
            generated_at: crate::platform::time::utc_now_rfc3339_millis(),
            source_fingerprint: compute_summary_source_fingerprint(&request.segments),
        }),
    };

    if let Some(hid) = history_id.clone() {
        let persisted_summary = summary.clone();
        llm_helpers::run_llm_db_task_with_app(&app, move |store| {
            llm_helpers::save_llm_summary_payload(&store, &hid, persisted_summary)
        })
        .await?;
    }

    emit_transcript_job_update(
        &app,
        &request.task_id,
        LlmTaskType::Summary,
        history_id.clone(),
        None,
        Some(summary.clone()),
        None,
    )?;

    Ok(TranscriptLlmJobResult {
        task_id: request.task_id,
        task_type: LlmTaskType::Summary,
        job_history_id: history_id,
        segments: None,
        summary: Some(summary),
        history_item: None,
    })
}

pub(crate) async fn run_transcript_llm_job_command(
    app: AppHandle,
    _state: State<'_, HistoryRepositoryState>,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    match request.task_type {
        LlmTaskType::Translate => run_translate_job(app.clone(), request).await,
        LlmTaskType::Polish => run_polish_job(app.clone(), request).await,
        LlmTaskType::Summary => run_summary_job(app, request).await,
    }
}
