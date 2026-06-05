use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chrono::SecondsFormat;
use serde_json::to_value;
use tauri::{AppHandle, Emitter, State};

use super::*;
use crate::integrations::asr::TranscriptSegment;
use crate::repositories::history::{HistoryRepositoryState, TranscriptSnapshotReason};
use crate::commands::history::{
    history_create_llm_transcript_snapshot, history_save_llm_summary,
    history_update_llm_transcript_segments,
};

fn normalized_job_history_id(job_history_id: Option<&str>) -> Option<String> {
    job_history_id
        .map(str::trim)
        .filter(|value| !value.is_empty() && *value != "current")
        .map(str::to_string)
}

fn segment_inputs_from_transcript(segments: &[TranscriptSegment]) -> Vec<LlmSegmentInput> {
    segments
        .iter()
        .map(|segment| LlmSegmentInput {
            id: segment.id.clone(),
            text: segment.text.clone(),
        })
        .collect()
}

fn summary_inputs_from_transcript(segments: &[TranscriptSegment]) -> Vec<SummarySegmentInput> {
    segments
        .iter()
        .map(|segment| {
            let text = segment
                .speaker
                .as_ref()
                .map(|speaker| format!("{}: {}", speaker.label, segment.text))
                .unwrap_or_else(|| segment.text.clone());
            SummarySegmentInput {
                id: segment.id.clone(),
                text,
                start: segment.start as f32,
                end: segment.end as f32,
                is_final: segment.is_final,
            }
        })
        .collect()
}

pub(crate) fn merge_translated_items_into_segments(
    mut segments: Vec<TranscriptSegment>,
    items: &[TranslatedSegment],
) -> Vec<TranscriptSegment> {
    let items_by_id: HashMap<&str, &TranslatedSegment> =
        items.iter().map(|item| (item.id.as_str(), item)).collect();
    for segment in &mut segments {
        if let Some(item) = items_by_id.get(segment.id.as_str()) {
            segment.translation = Some(item.translation.clone());
        }
    }
    segments
}

pub(crate) fn merge_polished_items_into_segments(
    mut segments: Vec<TranscriptSegment>,
    items: &[PolishedSegment],
) -> Vec<TranscriptSegment> {
    let items_by_id: HashMap<&str, &PolishedSegment> =
        items.iter().map(|item| (item.id.as_str(), item)).collect();
    for segment in &mut segments {
        if let Some(item) = items_by_id.get(segment.id.as_str()) {
            segment.text = item.text.clone();
        }
    }
    segments
}

pub(crate) fn compute_summary_source_fingerprint(segments: &[TranscriptSegment]) -> String {
    segments
        .iter()
        .map(|segment| {
            let (speaker_id, speaker_label, speaker_kind, speaker_score) = segment
                .speaker
                .as_ref()
                .map_or(("", "", "", String::new()), |speaker| {
                    (
                        speaker.id.as_str(),
                        speaker.label.as_str(),
                        speaker.kind.as_str(),
                        speaker
                            .score
                            .map(|score| score.to_string())
                            .unwrap_or_default(),
                    )
                });
            format!(
                "{}:{}:{}:{}:{}:{}:{}:{}:{}",
                segment.id,
                segment.text,
                segment.start,
                segment.end,
                segment.is_final,
                speaker_id,
                speaker_label,
                speaker_kind,
                speaker_score
            )
        })
        .collect::<Vec<_>>()
        .join("|")
}

fn emit_transcript_job_update(
    app: &AppHandle,
    task_id: &str,
    task_type: LlmTaskType,
    job_history_id: Option<String>,
    segments: Option<Vec<TranscriptSegment>>,
    summary: Option<HistorySummaryPayload>,
    history_item: Option<crate::repositories::history::HistoryItemRecord>,
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
    state: HistoryRepositoryState,
    history_id: Option<&str>,
    reason: TranscriptSnapshotReason,
    segments: &[TranscriptSegment],
) -> Result<(), String> {
    if let Some(history_id) = history_id {
        history_create_llm_transcript_snapshot(
            app,
            state,
            history_id.to_string(),
            reason,
            segments.to_vec(),
        )
        .await?;
    }
    Ok(())
}

async fn run_translate_job(
    app: AppHandle,
    history_state: HistoryRepositoryState,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    let history_id = normalized_job_history_id(request.job_history_id.as_deref());
    create_transcript_job_snapshot(
        app.clone(),
        history_state.clone(),
        history_id.as_deref(),
        TranscriptSnapshotReason::Translate,
        &request.segments,
    )
    .await?;

    let merged_segments = Arc::new(Mutex::new(request.segments.clone()));
    let observer_segments = merged_segments.clone();
    let update_app = app.clone();
    let update_task_id = request.task_id.clone();
    let update_history_id = history_id.clone();
    let llm_request = TranslateSegmentsRequest {
        task_id: request.task_id.clone(),
        config: request.config.clone(),
        segments: segment_inputs_from_transcript(&request.segments),
        chunk_size: request.chunk_size,
        target_language: request.target_language.unwrap_or_else(|| "zh".to_string()),
        target_language_name: request.target_language_name.clone(),
    };

    commands::translate_transcript_segments_with_observer(app.clone(), llm_request, move |items| {
        let next_segments = {
            let mut guard = observer_segments
                .lock()
                .map_err(|error| error.to_string())?;
            *guard = merge_translated_items_into_segments(guard.clone(), items);
            guard.clone()
        };
        emit_transcript_job_update(
            &update_app,
            &update_task_id,
            LlmTaskType::Translate,
            update_history_id.clone(),
            Some(next_segments),
            None,
            None,
        )
    })
    .await?;

    let final_segments = merged_segments
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let history_item = if let Some(history_id) = history_id.clone() {
        history_update_llm_transcript_segments(
            app.clone(),
            history_state,
            history_id,
            final_segments.clone(),
        )
        .await?
    } else {
        None
    };

    emit_transcript_job_update(
        &app,
        &request.task_id,
        LlmTaskType::Translate,
        history_id.clone(),
        Some(final_segments.clone()),
        None,
        history_item.clone(),
    )?;

    Ok(TranscriptLlmJobResult {
        task_id: request.task_id,
        task_type: LlmTaskType::Translate,
        job_history_id: history_id,
        segments: Some(final_segments),
        summary: None,
        history_item,
    })
}

async fn run_polish_job(
    app: AppHandle,
    history_state: HistoryRepositoryState,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    let history_id = normalized_job_history_id(request.job_history_id.as_deref());
    create_transcript_job_snapshot(
        app.clone(),
        history_state.clone(),
        history_id.as_deref(),
        TranscriptSnapshotReason::Polish,
        &request.segments,
    )
    .await?;

    let merged_segments = Arc::new(Mutex::new(request.segments.clone()));
    let observer_segments = merged_segments.clone();
    let update_app = app.clone();
    let update_task_id = request.task_id.clone();
    let update_history_id = history_id.clone();
    let llm_request = PolishSegmentsRequest {
        task_id: request.task_id.clone(),
        config: request.config.clone(),
        segments: segment_inputs_from_transcript(&request.segments),
        chunk_size: request.chunk_size,
        context: request.context.clone(),
        keywords: request.keywords.clone(),
    };

    commands::polish_transcript_segments_with_observer(app.clone(), llm_request, move |items| {
        let next_segments = {
            let mut guard = observer_segments
                .lock()
                .map_err(|error| error.to_string())?;
            *guard = merge_polished_items_into_segments(guard.clone(), items);
            guard.clone()
        };
        emit_transcript_job_update(
            &update_app,
            &update_task_id,
            LlmTaskType::Polish,
            update_history_id.clone(),
            Some(next_segments),
            None,
            None,
        )
    })
    .await?;

    let final_segments = merged_segments
        .lock()
        .map_err(|error| error.to_string())?
        .clone();
    let history_item = if let Some(history_id) = history_id.clone() {
        history_update_llm_transcript_segments(
            app.clone(),
            history_state,
            history_id,
            final_segments.clone(),
        )
        .await?
    } else {
        None
    };

    emit_transcript_job_update(
        &app,
        &request.task_id,
        LlmTaskType::Polish,
        history_id.clone(),
        Some(final_segments.clone()),
        None,
        history_item.clone(),
    )?;

    Ok(TranscriptLlmJobResult {
        task_id: request.task_id,
        task_type: LlmTaskType::Polish,
        job_history_id: history_id,
        segments: Some(final_segments),
        summary: None,
        history_item,
    })
}

async fn run_summary_job(
    app: AppHandle,
    history_state: HistoryRepositoryState,
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
            generated_at: chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
            source_fingerprint: compute_summary_source_fingerprint(&request.segments),
        }),
    };

    if let Some(history_id) = history_id.clone() {
        history_save_llm_summary(
            app.clone(),
            history_state,
            history_id,
            to_value(&summary).map_err(|error| error.to_string())?,
        )
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
    state: State<'_, HistoryRepositoryState>,
    request: TranscriptLlmJobRequest,
) -> Result<TranscriptLlmJobResult, String> {
    let history_state = state.inner().clone();
    match request.task_type {
        LlmTaskType::Translate => run_translate_job(app, history_state, request).await,
        LlmTaskType::Polish => run_polish_job(app, history_state, request).await,
        LlmTaskType::Summary => run_summary_job(app, history_state, request).await,
    }
}
