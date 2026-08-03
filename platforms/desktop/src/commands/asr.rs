use crate::integrations::asr::{
    AsrPortError, AsrRuntimeMetricsSnapshot, AsrState, AsrTranscriptionRequest,
    TauriAsrRuntimeObserver, TranscriptSegment, ensure_adapter, get_provider_id,
};
use crate::platform::event::{EventEmitterPort, TauriEventEmitter};
use sona_application::live_transcription::{LiveInputTransform, LiveSourceEpoch};
use sona_core::ports::asr::AsrRuntimeObserver;
use std::sync::Arc;
use tauri::{AppHandle, State};

#[tauri::command(async)]
pub async fn prepare_live_transcription(
    state: State<'_, crate::integrations::asr::AsrState>,
    asr_request: AsrTranscriptionRequest,
) -> Result<(), AsrPortError> {
    state.live_coordinator().prepare(&asr_request).await
}

#[tauri::command(async)]
pub async fn create_external_live_source(
    state: State<'_, crate::integrations::asr::AsrState>,
) -> Result<crate::integrations::asr::ExternalLiveSource, AsrPortError> {
    Ok(state.create_external_source().await)
}

#[tauri::command(async)]
pub async fn start_external_live_transcription(
    app: AppHandle,
    state: State<'_, crate::integrations::asr::AsrState>,
    consumer_id: String,
    source_token: String,
    gain: f32,
    asr_request: AsrTranscriptionRequest,
) -> Result<sona_application::live_transcription::LiveTranscriptionSubscription, AsrPortError> {
    let (source, source_cursor) = state
        .external_source(&source_token)
        .await
        .ok_or_else(|| AsrPortError::invalid_request("External live source token is invalid"))?;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(
        Arc::new(TauriEventEmitter(app)),
        state.metrics_store(),
    )) as Arc<dyn AsrRuntimeObserver>;
    state
        .live_coordinator()
        .acquire(
            consumer_id,
            source,
            source_cursor,
            LiveInputTransform { gain },
            asr_request,
            observer,
        )
        .await
}

#[tauri::command(async)]
pub async fn feed_external_live_source(
    state: State<'_, crate::integrations::asr::AsrState>,
    source_token: String,
    samples: Vec<u8>,
) -> Result<(), AsrPortError> {
    if !samples.len().is_multiple_of(2) {
        return Err(AsrPortError::invalid_request(
            "External PCM payload must contain complete i16 samples",
        ));
    }
    let samples = samples
        .chunks_exact(2)
        .map(|sample| i16::from_le_bytes([sample[0], sample[1]]) as f32 / 32768.0)
        .collect::<Vec<_>>();
    let (source, frame) = state
        .next_external_frame(&source_token, samples)
        .await
        .ok_or_else(|| AsrPortError::invalid_request("External live source token is invalid"))?;
    state.live_coordinator().feed_source(&source, frame).await
}

#[tauri::command(async)]
pub async fn retire_external_live_source(
    state: State<'_, crate::integrations::asr::AsrState>,
    source_token: String,
) -> Result<(), AsrPortError> {
    let source = state
        .remove_external_source(&source_token)
        .await
        .ok_or_else(|| AsrPortError::invalid_request("External live source token is invalid"))?;
    state.live_coordinator().retire_source(&source).await
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNativeTranscriptionStart {
    pub lease: crate::integrations::audio::LiveCaptureLease,
    pub subscription: sona_application::live_transcription::LiveTranscriptionSubscription,
}

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub async fn start_native_live_transcription(
    app: AppHandle,
    window: tauri::Window,
    audio_state: State<'_, crate::integrations::audio::AudioState>,
    state: State<'_, crate::integrations::asr::AsrState>,
    consumer_id: String,
    source_kind: String,
    device_name: Option<String>,
    output_path: Option<String>,
    gain: f32,
    asr_request: AsrTranscriptionRequest,
) -> Result<LiveNativeTranscriptionStart, AsrPortError> {
    let lease = crate::integrations::audio::start_native_live_capture(
        app.clone(),
        window,
        &audio_state,
        &source_kind,
        device_name,
        consumer_id.clone(),
        output_path,
    )
    .map_err(AsrPortError::runtime)?;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(
        Arc::new(TauriEventEmitter(app)),
        state.metrics_store(),
    )) as Arc<dyn AsrRuntimeObserver>;
    match state
        .live_coordinator()
        .acquire(
            consumer_id.clone(),
            LiveSourceEpoch::new(lease.source_id.clone(), lease.source_generation),
            lease.source_cursor,
            LiveInputTransform { gain },
            asr_request,
            observer,
        )
        .await
    {
        Ok(subscription) => Ok(LiveNativeTranscriptionStart {
            lease,
            subscription,
        }),
        Err(error) => {
            let _ = crate::integrations::audio::stop_native_live_capture(
                &audio_state,
                &source_kind,
                consumer_id,
            )
            .await;
            Err(error)
        }
    }
}

#[tauri::command(async)]
pub async fn stop_live_transcription(
    state: State<'_, crate::integrations::asr::AsrState>,
    consumer_id: String,
) -> Result<(), AsrPortError> {
    state.live_coordinator().release(&consumer_id).await
}

#[tauri::command(async)]
pub async fn pause_native_live_transcription(
    audio_state: State<'_, crate::integrations::audio::AudioState>,
    state: State<'_, crate::integrations::asr::AsrState>,
    consumer_id: String,
    source_kind: String,
) -> Result<(), AsrPortError> {
    crate::integrations::audio::set_native_live_capture_paused(
        &audio_state,
        &source_kind,
        &consumer_id,
        true,
    )
    .map_err(AsrPortError::runtime)?;
    if let Err(error) = state.live_coordinator().release(&consumer_id).await {
        let _ = crate::integrations::audio::set_native_live_capture_paused(
            &audio_state,
            &source_kind,
            &consumer_id,
            false,
        );
        return Err(error);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub async fn resume_native_live_transcription(
    app: AppHandle,
    audio_state: State<'_, crate::integrations::audio::AudioState>,
    state: State<'_, crate::integrations::asr::AsrState>,
    consumer_id: String,
    source_kind: String,
    gain: f32,
    asr_request: AsrTranscriptionRequest,
) -> Result<LiveNativeTranscriptionStart, AsrPortError> {
    let lease = crate::integrations::audio::set_native_live_capture_paused(
        &audio_state,
        &source_kind,
        &consumer_id,
        false,
    )
    .map_err(AsrPortError::runtime)?;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(
        Arc::new(TauriEventEmitter(app)),
        state.metrics_store(),
    )) as Arc<dyn AsrRuntimeObserver>;
    match state
        .live_coordinator()
        .acquire(
            consumer_id.clone(),
            LiveSourceEpoch::new(lease.source_id.clone(), lease.source_generation),
            lease.source_cursor,
            LiveInputTransform { gain },
            asr_request,
            observer,
        )
        .await
    {
        Ok(subscription) => Ok(LiveNativeTranscriptionStart {
            lease,
            subscription,
        }),
        Err(error) => {
            let _ = crate::integrations::audio::set_native_live_capture_paused(
                &audio_state,
                &source_kind,
                &consumer_id,
                true,
            );
            Err(error)
        }
    }
}

#[tauri::command(async)]
pub async fn stop_native_live_transcription(
    audio_state: State<'_, crate::integrations::audio::AudioState>,
    state: State<'_, crate::integrations::asr::AsrState>,
    consumer_id: String,
    source_kind: String,
) -> Result<String, AsrPortError> {
    let release_result = if state.live_coordinator().has_consumer(&consumer_id).await {
        state.live_coordinator().release(&consumer_id).await
    } else {
        Ok(())
    };
    let capture_result = crate::integrations::audio::stop_native_live_capture(
        &audio_state,
        &source_kind,
        consumer_id,
    )
    .await
    .map_err(AsrPortError::runtime);
    release_result.and(capture_result)
}

#[tauri::command(async)]
pub async fn get_live_transcription_metrics(
    state: State<'_, crate::integrations::asr::AsrState>,
) -> Result<sona_application::live_transcription::LiveTranscriptionMetrics, AsrPortError> {
    Ok(state.live_coordinator().metrics().await)
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
    let emitter = Arc::new(TauriEventEmitter(app.clone())) as Arc<dyn EventEmitterPort>;
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
