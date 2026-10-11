use std::sync::Arc;
use tauri::State;

use crate::integrations::asr::{
    AsrPortError, AsrRuntimeMetricsSnapshot, AsrTranscriptionRequest, TauriAsrRuntimeObserver,
    TranscriptSegment,
};
use crate::services::DesktopServices;
use sona_application::live_transcription::{LiveInputTransform, LiveSourceEpoch};
use sona_core::ports::asr::AsrRuntimeObserver;

/// Cancels an in-flight `process_batch_file` call identified by `instance_id`.
///
/// The command is a no-op when no matching task is registered (e.g. the task
/// already finished before the signal arrived).
#[tauri::command]
pub async fn cancel_batch_task(
    services: State<'_, DesktopServices>,
    instance_id: String,
) -> Result<(), String> {
    services.asr.batch_cancel.cancel(&instance_id).await;
    Ok(())
}

#[tauri::command(async)]
pub async fn prepare_live_transcription(
    services: State<'_, DesktopServices>,
    asr_request: AsrTranscriptionRequest,
) -> Result<(), AsrPortError> {
    services.asr.live_coordinator().prepare(&asr_request).await
}

#[tauri::command(async)]
pub async fn create_external_live_source(
    services: State<'_, DesktopServices>,
) -> Result<crate::integrations::asr::ExternalLiveSource, AsrPortError> {
    Ok(services.asr.create_external_source().await)
}

#[tauri::command(async)]
pub async fn start_external_live_transcription(
    services: State<'_, DesktopServices>,
    consumer_id: String,
    source_token: String,
    gain: f32,
    asr_request: AsrTranscriptionRequest,
) -> Result<sona_application::live_transcription::LiveTranscriptionSubscription, AsrPortError> {
    let (source, source_cursor) = services
        .asr
        .external_source(&source_token)
        .await
        .ok_or_else(|| AsrPortError::invalid_request("External live source token is invalid"))?;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(
        services.emitter.clone(),
        services.asr.metrics_store(),
    )) as Arc<dyn AsrRuntimeObserver>;
    if services
        .asr
        .live_coordinator()
        .has_consumer(&consumer_id)
        .await
    {
        log::warn!(
            "[ASR] Consumer '{consumer_id}' is already active before external acquire; releasing stale session"
        );
        let _ = services.asr.live_coordinator().release(&consumer_id).await;
    }
    services
        .asr
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
    services: State<'_, DesktopServices>,
    source_token: String,
    samples: Vec<u8>,
) -> Result<(), AsrPortError> {
    let samples = sona_core::ports::asr::pcm_i16_le_to_f32_samples(&samples)?;
    let (source, frame) = services
        .asr
        .next_external_frame(&source_token, samples)
        .await
        .ok_or_else(|| AsrPortError::invalid_request("External live source token is invalid"))?;
    services
        .asr
        .live_coordinator()
        .feed_source(&source, frame)
        .await
}

#[tauri::command(async)]
pub async fn retire_external_live_source(
    services: State<'_, DesktopServices>,
    source_token: String,
) -> Result<(), AsrPortError> {
    let source = services
        .asr
        .remove_external_source(&source_token)
        .await
        .ok_or_else(|| AsrPortError::invalid_request("External live source token is invalid"))?;
    services.asr.live_coordinator().retire_source(&source).await
}

#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub async fn start_native_live_transcription(
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    consumer_id: String,
    source_kind: String,
    device_name: Option<String>,
    output_path: Option<String>,
    gain: f32,
    asr_request: AsrTranscriptionRequest,
) -> Result<LiveNativeTranscriptionStart, AsrPortError> {
    let audio_arc = services.audio.clone();
    let asr_arc = services.asr.clone();
    let emitter = services.emitter.clone();
    let app_data_dir = services
        .sqlite
        .current_context()
        .map_err(AsrPortError::runtime)?
        .app_data_dir()
        .to_path_buf();
    let capture_source_kind = source_kind.clone();
    let capture_consumer_id = consumer_id.clone();
    let lease = crate::integrations::audio::start_native_live_capture(
        audio_arc,
        asr_arc,
        emitter,
        app_data_dir,
        &capture_source_kind,
        device_name,
        capture_consumer_id,
        output_path,
    )
    .await
    .map_err(AsrPortError::runtime)?;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(
        services.emitter.clone(),
        services.asr.metrics_store(),
    )) as Arc<dyn AsrRuntimeObserver>;
    if services
        .asr
        .live_coordinator()
        .has_consumer(&consumer_id)
        .await
    {
        log::warn!(
            "[ASR] Consumer '{consumer_id}' is already active before native acquire; releasing stale session"
        );
        let _ = services.asr.live_coordinator().release(&consumer_id).await;
    }
    let asr_request_for_session = asr_request.clone();
    match services
        .asr
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
        Ok(subscription) => {
            crate::app::tray::set_recording_active(&app, true);
            {
                let mut guard = services.recording_session.lock().await;
                if guard.is_none() {
                    *guard = Some(crate::platform::agent_control::ActiveRecordingSession {
                        history_id: format!("ui-record-{}", uuid::Uuid::new_v4()),
                        consumer_id: consumer_id.clone(),
                        source_kind: source_kind.clone(),
                        is_paused: false,
                        paused_at_instant: None,
                        accumulated_pause_duration: std::time::Duration::ZERO,
                        asr_request: Some(asr_request_for_session),
                        gain,
                        started_at_epoch: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or(0),
                        started_at_instant: std::time::Instant::now(),
                        segments: Arc::new(std::sync::Mutex::new(Vec::new())),
                        has_coordinator_consumer: true,
                        coordinator_released: false,
                        audio_stopped: false,
                        frozen_duration_seconds: None,
                        _sleep_guard: None,
                        is_ui_session: true,
                    });
                }
            }
            Ok(LiveNativeTranscriptionStart {
                lease,
                subscription,
            })
        }
        Err(error) => {
            let _ = crate::integrations::audio::stop_native_live_capture(
                &services.audio,
                &source_kind,
                consumer_id.clone(),
            )
            .await;
            if services
                .asr
                .live_coordinator()
                .has_consumer(&consumer_id)
                .await
            {
                let _ = services.asr.live_coordinator().release(&consumer_id).await;
            }
            if !services.audio.has_active_captures() {
                crate::app::tray::set_recording_active(&app, false);
            }
            Err(error)
        }
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveNativeTranscriptionStart {
    pub lease: crate::integrations::audio::LiveCaptureLease,
    pub subscription: sona_application::live_transcription::LiveTranscriptionSubscription,
}

#[tauri::command(async)]
pub async fn stop_live_transcription(
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    consumer_id: String,
) -> Result<(), AsrPortError> {
    let res = if services
        .asr
        .live_coordinator()
        .has_consumer(&consumer_id)
        .await
    {
        services.asr.live_coordinator().release(&consumer_id).await
    } else {
        Ok(())
    };
    {
        let mut guard = services.recording_session.lock().await;
        if let Some(session) = guard.as_ref()
            && session.consumer_id == consumer_id
        {
            *guard = None;
        }
    }
    if !services.audio.has_active_captures() {
        crate::app::tray::set_recording_active(&app, false);
    }
    res
}

#[tauri::command(async)]
pub async fn pause_native_live_transcription(
    services: State<'_, DesktopServices>,
    consumer_id: String,
    source_kind: String,
) -> Result<(), AsrPortError> {
    let pause_result = crate::integrations::audio::set_native_live_capture_paused(
        &services.audio,
        &source_kind,
        &consumer_id,
        true,
    );
    let release_result = if services
        .asr
        .live_coordinator()
        .has_consumer(&consumer_id)
        .await
    {
        services.asr.live_coordinator().release(&consumer_id).await
    } else {
        Ok(())
    };
    let res = if let Err(error) = release_result {
        if pause_result.is_ok() {
            let _ = crate::integrations::audio::set_native_live_capture_paused(
                &services.audio,
                &source_kind,
                &consumer_id,
                false,
            );
        }
        Err(error)
    } else {
        match pause_result {
            Ok(_) | Err(crate::integrations::audio::AudioCaptureError::InstanceNotActive(_)) => {
                Ok(())
            }
            Err(err) => Err(AsrPortError::runtime(err.to_string())),
        }
    };
    if res.is_ok() {
        let mut guard = services.recording_session.lock().await;
        if let Some(session) = guard.as_mut()
            && session.consumer_id == consumer_id
            && !session.is_paused
        {
            session.is_paused = true;
            session.paused_at_instant = Some(std::time::Instant::now());
            session.coordinator_released = true;
        }
    }
    res
}
#[allow(clippy::too_many_arguments)]
#[tauri::command(async)]
pub async fn resume_native_live_transcription(
    services: State<'_, DesktopServices>,
    consumer_id: String,
    source_kind: String,
    gain: f32,
    asr_request: AsrTranscriptionRequest,
) -> Result<LiveNativeTranscriptionStart, AsrPortError> {
    let lease = crate::integrations::audio::set_native_live_capture_paused(
        &services.audio,
        &source_kind,
        &consumer_id,
        false,
    )
    .map_err(|err| AsrPortError::runtime(err.to_string()))?;
    let observer = Arc::new(TauriAsrRuntimeObserver::new(
        services.emitter.clone(),
        services.asr.metrics_store(),
    )) as Arc<dyn AsrRuntimeObserver>;
    match services
        .asr
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
        Ok(subscription) => {
            {
                let mut guard = services.recording_session.lock().await;
                if let Some(session) = guard.as_mut()
                    && session.consumer_id == consumer_id
                    && session.is_paused
                {
                    if let Some(paused_at) = session.paused_at_instant.take() {
                        session.accumulated_pause_duration += paused_at.elapsed();
                    }
                    session.is_paused = false;
                    session.coordinator_released = false;
                }
            }
            Ok(LiveNativeTranscriptionStart {
                lease,
                subscription,
            })
        }
        Err(error) => {
            let _ = crate::integrations::audio::set_native_live_capture_paused(
                &services.audio,
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
    app: tauri::AppHandle,
    services: State<'_, DesktopServices>,
    consumer_id: String,
    source_kind: String,
) -> Result<String, AsrPortError> {
    let capture_result = crate::integrations::audio::stop_native_live_capture(
        &services.audio,
        &source_kind,
        consumer_id.clone(),
    )
    .await
    .map_err(AsrPortError::runtime);
    let release_result = if services
        .asr
        .live_coordinator()
        .has_consumer(&consumer_id)
        .await
    {
        services.asr.live_coordinator().release(&consumer_id).await
    } else {
        Ok(())
    };
    {
        let mut guard = services.recording_session.lock().await;
        if let Some(session) = guard.as_ref()
            && session.consumer_id == consumer_id
        {
            *guard = None;
        }
    }
    if !services.audio.has_active_captures() {
        crate::app::tray::set_recording_active(&app, false);
    }
    release_result.and(capture_result)
}

#[tauri::command(async)]
pub async fn get_live_transcription_metrics(
    services: State<'_, DesktopServices>,
) -> Result<sona_application::live_transcription::LiveTranscriptionMetrics, AsrPortError> {
    Ok(services.asr.live_coordinator().metrics().await)
}

#[tauri::command]
pub async fn process_batch_file(
    services: State<'_, DesktopServices>,
    file_path: String,
    save_to_path: Option<String>,
    speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
    asr_request: AsrTranscriptionRequest,
    instance_id: Option<String>,
) -> Result<Vec<TranscriptSegment>, AsrPortError> {
    crate::integrations::asr::process_batch_file(
        services.emitter.clone(),
        &services.asr,
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
    services: State<'_, DesktopServices>,
) -> Result<AsrRuntimeMetricsSnapshot, String> {
    let metrics = services.asr.metrics_snapshot().await;
    sona_ts_bind::validate_asr_runtime_metrics_for_typescript(&metrics)
        .map_err(|error| error.to_string())?;
    Ok(metrics)
}

#[tauri::command]
pub async fn test_online_asr_provider(
    provider_id: String,
    config: serde_json::Value,
) -> Result<u64, AsrPortError> {
    sona_online_asr::test_online_asr_provider(&provider_id, &config).await
}

#[cfg(test)]
mod tests {

    #[tokio::test(flavor = "current_thread")]
    async fn native_capture_start_does_not_starve_async_writer_acknowledgement() {
        let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel(1);
        let writer = tokio::spawn(async move {
            tokio::task::yield_now().await;
            if let Some(crate::integrations::audio::RecorderCommand::Start { completed, .. }) =
                recorder_rx.recv().await
            {
                let _ = completed.send(Ok(()));
            }
        });

        let res = crate::integrations::audio::recorder::queue_recording_start(
            Some(&recorder_tx),
            true,
            "Microphone",
            "test-consumer",
            Some("test_non_starve.wav".to_string()),
            || Ok("test.wav".to_string()),
        )
        .await;

        writer.await.unwrap();
        assert!(res.is_ok());
    }

    #[test]
    fn audio_capture_error_instance_not_active_matches_for_idempotent_pause() {
        let audio_state = crate::integrations::audio::AudioState::new();
        let result = crate::integrations::audio::set_native_live_capture_paused(
            &audio_state,
            "system",
            "unknown-consumer",
            true,
        );
        assert!(matches!(
            &result,
            Err(crate::integrations::audio::AudioCaptureError::InstanceNotActive(id)) if id == "unknown-consumer"
        ));
        let handled = match result {
            Ok(_) | Err(crate::integrations::audio::AudioCaptureError::InstanceNotActive(_)) => {
                Ok(())
            }
            Err(e) => Err(e),
        };
        assert!(handled.is_ok());
    }
}
