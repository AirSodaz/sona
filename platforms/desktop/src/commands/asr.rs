use crate::integrations::asr::{
    AsrPortError, AsrRuntimeMetricsSnapshot, AsrState, AsrTranscriptionRequest,
    TauriAsrRuntimeObserver, TranscriptSegment, ensure_adapter, get_provider_id,
};
use crate::platform::event::{EventEmitter, TauriEventEmitter};
use sona_core::ports::asr::{AsrPortErrorKind, AsrRuntimeObserver};
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command]
pub async fn init_recognizer(
    app: AppHandle,
    state: State<'_, AsrState>,
    instance_id: String,
    asr_request: AsrTranscriptionRequest,
) -> Result<(), AsrPortError> {
    let adapter = ensure_adapter(&asr_request)?;
    let emitter = Arc::new(TauriEventEmitter(app)) as Arc<dyn EventEmitter>;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(emitter, state.metrics_store()))
        as Arc<dyn AsrRuntimeObserver>;
    let session = adapter
        .create_streaming_session(&state, &instance_id, &asr_request, observer)
        .await?;
    if let Some(session) = session {
        state.insert_session(&instance_id, session).await;
        state
            .set_instance_engine(&instance_id, asr_request.engine())
            .await;
        Ok(())
    } else {
        Err(AsrPortError::new(
            AsrPortErrorKind::Unsupported,
            format!("provider {} 不支持流式识别", get_provider_id(&asr_request)?),
        )
        .with_code("STREAMING_NOT_SUPPORTED"))
    }
}

#[tauri::command]
pub async fn start_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), AsrPortError> {
    let session = state
        .session(&instance_id)
        .await
        .ok_or_else(|| AsrPortError::runtime(format!("ASR instance {} not found", instance_id)))?;
    session.start().await
}

#[tauri::command]
pub async fn stop_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), AsrPortError> {
    let session = state
        .remove_session(&instance_id)
        .await
        .ok_or_else(|| AsrPortError::runtime(format!("ASR instance {} not found", instance_id)))?;
    session.stop().await
}

#[tauri::command]
pub async fn flush_recognizer(
    state: State<'_, AsrState>,
    instance_id: String,
) -> Result<(), AsrPortError> {
    let session = state
        .session(&instance_id)
        .await
        .ok_or_else(|| AsrPortError::runtime(format!("ASR instance {} not found", instance_id)))?;
    session.flush().await
}

#[tauri::command]
pub async fn feed_audio_chunk(
    state: State<'_, AsrState>,
    instance_id: String,
    samples: Vec<u8>,
) -> Result<(), AsrPortError> {
    let session = state
        .session(&instance_id)
        .await
        .ok_or_else(|| AsrPortError::runtime(format!("ASR instance {} not found", instance_id)))?;
    session.feed_audio_chunk(samples).await
}

#[tauri::command]
pub async fn process_batch_file(
    app: AppHandle,
    state: State<'_, AsrState>,
    file_path: String,
    save_to_path: Option<String>,
    speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
    asr_request: AsrTranscriptionRequest,
    instance_id: Option<String>,
) -> Result<Vec<TranscriptSegment>, AsrPortError> {
    let adapter = ensure_adapter(&asr_request)?;
    let processor = adapter
        .create_batch_processor(&asr_request)?
        .ok_or_else(|| {
            AsrPortError::runtime(format!(
                "Batch mode not supported for provider {}",
                get_provider_id(&asr_request).unwrap_or("unknown")
            ))
        })?;
    let emitter = Arc::new(TauriEventEmitter(app.clone())) as Arc<dyn EventEmitter>;
    processor
        .process_file(
            emitter,
            &state,
            file_path.into(),
            save_to_path.map(Into::into),
            asr_request,
            speaker_processing,
            instance_id,
        )
        .await
}

#[tauri::command]
pub async fn get_asr_runtime_metrics(
    state: State<'_, AsrState>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    let metrics = state.metrics_snapshot().await;
    sona_ts_bind::validate_asr_runtime_metrics_for_typescript(&metrics)
        .map_err(|error| error.to_string())?;
    Ok(metrics)
}
