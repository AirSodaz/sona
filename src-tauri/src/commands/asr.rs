use crate::core::event::EventEmitter;
use crate::integrations::asr::{
    AsrRuntimeMetricsSnapshot, AsrState, AsrTranscriptionRequest, SherpaError, TranscriptSegment,
    ensure_adapter, get_provider_id,
};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn init_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
    asr_request: AsrTranscriptionRequest,
) -> Result<(), SherpaError> {
    let adapter = ensure_adapter(&asr_request)?;
    let session = adapter
        .create_streaming_session(&state, &instance_id, &asr_request)
        .await?;
    if let Some(session) = session {
        let mut active = state.active_sessions.lock().await;
        active.insert(instance_id.clone(), session);
        state
            .set_instance_engine(&instance_id, asr_request.engine())
            .await;
        Ok(())
    } else {
        Err(SherpaError::StreamingNotSupported {
            provider_id: get_provider_id(&asr_request)?.to_string(),
        })
    }
}

#[tauri::command]
pub async fn start_recognizer(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    let emitter = Arc::new(app.clone()) as Arc<dyn EventEmitter>;
    session.start(emitter, &state, &instance_id).await
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    session.stop(&state, &instance_id).await
}

#[tauri::command]
pub async fn flush_recognizer(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    let emitter = Arc::new(app.clone()) as Arc<dyn EventEmitter>;
    session.flush(emitter, &state, &instance_id).await
}

#[tauri::command]
pub async fn feed_audio_chunk(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), SherpaError> {
    let session = {
        let sessions = state.active_sessions.lock().await;
        sessions.get(&instance_id).cloned().ok_or_else(|| {
            SherpaError::Generic(format!("ASR instance {} not found", instance_id))
        })?
    };
    let emitter = Arc::new(app.clone()) as Arc<dyn EventEmitter>;
    session
        .feed_audio_chunk(emitter, &state, &instance_id, samples)
        .await
}

#[tauri::command]
pub async fn process_batch_file(
    app: AppHandle,
    state: State<'_, AsrState>,
    file_path: String,
    save_to_path: Option<String>,
    speaker_processing: Option<crate::integrations::speaker::SpeakerProcessingConfig>,
    asr_request: AsrTranscriptionRequest,
) -> Result<Vec<TranscriptSegment>, SherpaError> {
    let adapter = ensure_adapter(&asr_request)?;
    let processor = adapter
        .create_batch_processor(&asr_request)?
        .ok_or_else(|| {
            SherpaError::Generic(format!(
                "Batch mode not supported for provider {}",
                get_provider_id(&asr_request).unwrap_or("unknown")
            ))
        })?;
    let emitter = Arc::new(app.clone()) as Arc<dyn EventEmitter>;
    processor
        .process_file(
            emitter,
            &state,
            file_path.into(),
            save_to_path.map(Into::into),
            asr_request,
            speaker_processing,
        )
        .await
}

#[tauri::command]
pub async fn get_asr_runtime_metrics(
    state: State<'_, AsrState>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    Ok(state.metrics_snapshot().await)
}
