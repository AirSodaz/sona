use std::sync::Arc;
use tauri::{Emitter, State};

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
    app: tauri::AppHandle,
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
    configure_realtime_translation(&services, &app);
    let acquire_res = services
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
        .await;
    if acquire_res.is_err() && !services.asr.live_coordinator().is_active().await {
        services.asr.live_coordinator().set_translation(None);
        services.translation_pool.unload_all();
    }
    acquire_res
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
    let _op_guard = services.recording_session.lock_operation().await;
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
    configure_realtime_translation(&services, &app);
    let lease = match crate::integrations::audio::start_native_live_capture(
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
    {
        Ok(lease) => lease,
        Err(err) => {
            services.asr.live_coordinator().set_translation(None);
            return Err(AsrPortError::runtime(err));
        }
    };
    let segments_arc = Arc::new(std::sync::Mutex::new(Vec::new()));
    let observer_inner =
        TauriAsrRuntimeObserver::new(services.emitter.clone(), services.asr.metrics_store());
    let observer = Arc::new(
        crate::platform::agent_control::AgentAsrRuntimeObserver::new(
            observer_inner,
            segments_arc.clone(),
        ),
    ) as Arc<dyn AsrRuntimeObserver>;
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
                        segments: segments_arc,
                        has_coordinator_consumer: true,
                        coordinator_released: false,
                        audio_stopped: false,
                        frozen_duration_seconds: None,
                        _sleep_guard: None,
                        is_ui_session: true,
                        saved_wav_path: None,
                    });
                }
            }
            Ok(LiveNativeTranscriptionStart {
                lease,
                subscription,
            })
        }
        Err(error) => {
            services.asr.live_coordinator().set_translation(None);
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
    if !services.asr.live_coordinator().is_active().await {
        services.asr.live_coordinator().set_translation(None);
        services.translation_pool.unload_all();
    }
    {
        let mut guard = services.recording_session.lock().await;
        if let Some(session) = guard.as_ref()
            && session.consumer_id == consumer_id
        {
            *guard = None;
            services.recording_session.notify_cleared();
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
    app: tauri::AppHandle,
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
    configure_realtime_translation(&services, &app);
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
    let capture_result = match crate::integrations::audio::stop_native_live_capture(
        &services.audio,
        &source_kind,
        consumer_id.clone(),
    )
    .await
    {
        Ok(path) => Ok(path),
        Err(err) => {
            if let Some(path) = services
                .recording_session
                .take_cached_saved_wav_path(&consumer_id)
                .await
            {
                Ok(path)
            } else {
                let cached_path = {
                    let guard = services.recording_session.lock().await;
                    guard
                        .as_ref()
                        .filter(|s| s.consumer_id == consumer_id)
                        .and_then(|s| s.saved_wav_path.clone())
                };
                if let Some(path) = cached_path {
                    Ok(path)
                } else if err.contains("is not active") {
                    log::warn!(
                        "[ASR] Stop native live capture for '{consumer_id}' found instance already inactive: {err}"
                    );
                    Ok(String::new())
                } else {
                    Err(AsrPortError::runtime(err))
                }
            }
        }
    };
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
    if !services.asr.live_coordinator().is_active().await {
        services.asr.live_coordinator().set_translation(None);
        services.translation_pool.unload_all();
    }
    {
        let mut guard = services.recording_session.lock().await;
        if let Some(session) = guard.as_ref()
            && session.consumer_id == consumer_id
        {
            *guard = None;
            services.recording_session.notify_cleared();
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

pub(crate) fn configure_realtime_translation(services: &DesktopServices, app: &tauri::AppHandle) {
    let realtime_translation_enabled = services
        .config
        .get_setting("realtimeTranslationEnabled")
        .ok()
        .flatten()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);

    if realtime_translation_enabled {
        let src = services
            .config
            .get_setting("realtimeTranslationSourceLanguage")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "en".to_string());
        let tgt = services
            .config
            .get_setting("realtimeTranslationTargetLanguage")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "zh".to_string());
        let engine_str = services
            .config
            .get_setting("realtimeTranslationEngine")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "onnx".to_string());

        let engine_kind = match engine_str.to_ascii_lowercase().as_str() {
            "llama-cpp" | "llama_cpp" | "llamacpp" => {
                sona_core::ports::translation::TranslationEngineKind::LlamaCpp
            }
            _ => sona_core::ports::translation::TranslationEngineKind::OnnxNmt,
        };

        let model_id = services
            .config
            .get_setting("realtimeTranslationModelId")
            .ok()
            .flatten()
            .and_then(|v| v.as_str().map(String::from));

        if let Ok(models_dir) =
            crate::platform::storage_location::resolve_active_models_dir_for_app(app)
        {
            match crate::integrations::translation::TranslationEngineFactory::create_engine(
                services,
                engine_kind,
                model_id.as_deref(),
                &src,
                &tgt,
                &models_dir,
            ) {
                Ok(engine) => {
                    let coordinator = match engine_kind {
                        sona_core::ports::translation::TranslationEngineKind::LlamaCpp => {
                            Arc::new(
                                sona_application::live_transcription::StreamingTranslationCoordinator::new(
                                    engine, src, tgt,
                                )
                                .with_debounce(1000, 3),
                            )
                        }
                        sona_core::ports::translation::TranslationEngineKind::OnnxNmt => {
                            Arc::new(
                                sona_application::live_transcription::StreamingTranslationCoordinator::new(
                                    engine, src, tgt,
                                )
                                .with_debounce(300, 3),
                            )
                        }
                    };
                    services
                        .asr
                        .live_coordinator()
                        .set_translation(Some(coordinator));
                }
                Err(e) => {
                    log::warn!(
                        "[ASR] Realtime translation model could not be loaded ({engine_str}): {e}"
                    );
                    services.asr.live_coordinator().set_translation(None);
                    let _ = app.emit(
                        "realtime-translation-error",
                        serde_json::json!({
                            "error": e.to_string(),
                            "engine": engine_str,
                        }),
                    );
                }
            }
        } else {
            log::warn!("[ASR] Failed to resolve active models directory for realtime translation");
            services.asr.live_coordinator().set_translation(None);
            let _ = app.emit(
                "realtime-translation-error",
                serde_json::json!({
                    "error": "Failed to resolve active models directory".to_string(),
                    "engine": engine_str,
                }),
            );
        }
    } else {
        services.asr.live_coordinator().set_translation(None);
    }
}

#[tauri::command(async)]
pub async fn toggle_live_translation(
    services: State<'_, DesktopServices>,
    app: tauri::AppHandle,
    enabled: bool,
    source_lang: Option<String>,
    target_lang: Option<String>,
    catchup_segments: Option<Vec<sona_core::transcription::transcript::TranscriptSegment>>,
) -> Result<(), String> {
    if !enabled {
        services.asr.live_coordinator().set_translation(None);
        return Ok(());
    }

    let src = source_lang
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            services
                .config
                .get_setting("realtimeTranslationSourceLanguage")
                .ok()
                .flatten()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "auto".to_string())
        });
    let tgt = target_lang
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            services
                .config
                .get_setting("realtimeTranslationTargetLanguage")
                .ok()
                .flatten()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| "zh".to_string())
        });

    let engine_str = services
        .config
        .get_setting("realtimeTranslationEngine")
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(String::from))
        .unwrap_or_else(|| "onnx".to_string());

    let engine_kind = match engine_str.to_ascii_lowercase().as_str() {
        "llama-cpp" | "llama_cpp" | "llamacpp" => {
            sona_core::ports::translation::TranslationEngineKind::LlamaCpp
        }
        _ => sona_core::ports::translation::TranslationEngineKind::OnnxNmt,
    };

    let model_id = services
        .config
        .get_setting("realtimeTranslationModelId")
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(String::from));

    let norm_src = sona_application::live_transcription::normalize_language_code(&src);
    let norm_tgt = sona_application::live_transcription::normalize_language_code(&tgt);
    if !src.eq_ignore_ascii_case("auto") && norm_src.eq_ignore_ascii_case(norm_tgt) {
        services.asr.live_coordinator().set_translation(None);
        return Ok(());
    }

    let resolved_engine_src = if engine_kind
        == sona_core::ports::translation::TranslationEngineKind::OnnxNmt
        && (src.trim().is_empty() || src.eq_ignore_ascii_case("auto"))
    {
        if norm_tgt.eq_ignore_ascii_case("zh") {
            "en".to_string()
        } else {
            "zh".to_string()
        }
    } else {
        src.clone()
    };

    let models_dir = crate::platform::storage_location::resolve_active_models_dir_for_app(&app)
        .map_err(|e| format!("Failed to resolve models directory: {e}"))?;

    let engine = crate::integrations::translation::TranslationEngineFactory::create_engine(
        &services,
        engine_kind,
        model_id.as_deref(),
        &resolved_engine_src,
        &tgt,
        &models_dir,
    )
    .map_err(|e| format!("Failed to create translation engine: {e}"))?;

    let debounce_ms = match engine_kind {
        sona_core::ports::translation::TranslationEngineKind::LlamaCpp => 1000,
        _ => 300,
    };

    let coordinator = Arc::new(
        sona_application::live_transcription::StreamingTranslationCoordinator::new(
            Arc::clone(&engine),
            src.clone(),
            tgt.clone(),
        )
        .with_debounce(debounce_ms, 3),
    );

    services
        .asr
        .live_coordinator()
        .set_translation(Some(coordinator));

    if let Some(segments) = catchup_segments.filter(|s| !s.is_empty()) {
        let live_coord = services.asr.live_coordinator().clone();
        let recording_session = services.recording_session.clone();
        let active_consumer = {
            let guard = services.recording_session.lock().await;
            guard.as_ref().map(|s| s.consumer_id.clone())
        };
        tokio::spawn(async move {
            for mut seg in segments {
                let still_active = {
                    let guard = recording_session.lock().await;
                    guard
                        .as_ref()
                        .is_some_and(|s| active_consumer.as_deref() == Some(&s.consumer_id))
                };
                if !still_active || !live_coord.is_translation_enabled() {
                    break;
                }

                let text = seg.text.trim();
                if text.is_empty() {
                    continue;
                }
                let eff_src = if src.trim().is_empty() || src.eq_ignore_ascii_case("auto") {
                    sona_application::live_transcription::detect_script_language(text)
                } else {
                    sona_application::live_transcription::normalize_language_code(&src)
                };
                let eff_tgt = sona_application::live_transcription::normalize_language_code(&tgt);
                if eff_src.eq_ignore_ascii_case(eff_tgt) {
                    continue;
                }

                match engine.translate(text, eff_src, eff_tgt).await {
                    Ok(translated) => {
                        seg.translation = Some(translated);
                        seg.draft_translation = None;
                        let update = sona_core::transcription::transcript::TranscriptUpdate {
                            remove_ids: Vec::new(),
                            upsert_segments: vec![seg],
                        };
                        live_coord.broadcast_transcript_update(update).await;
                    }
                    Err(e) => {
                        log::warn!(
                            "[ASR] Catchup translation failed for segment {}: {e}",
                            seg.id
                        );
                    }
                }
            }
        });
    }

    Ok(())
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
