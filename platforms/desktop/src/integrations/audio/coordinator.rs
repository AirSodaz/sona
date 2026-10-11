use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use log::{debug, error, info, warn};
use sona_application::live_transcription::LiveSourceEpoch;
use sona_audio_capture::{
    CaptureDirection, CaptureEvent, LiveAudioCapturePipeline, LiveCaptureConfig,
};

use super::AudioState;
use super::devices::resolve_capture_device;
use super::recorder::{RecorderCommand, queue_recording_start};
use super::registry::SharedCaptureState;
use super::types::{AudioStopSignal, CaptureKey, CaptureKind, LiveCaptureLease};
use super::worker::spawn_capture_worker_task;

pub async fn wait_for_device_teardown(
    state: &AudioState,
    key: &CaptureKey,
    timeout: Duration,
) -> Result<(), String> {
    let rx = {
        let registry = state.registry.lock().map_err(|e| e.to_string())?;
        registry.is_stopping(key)
    };
    if let Some(mut rx) = rx {
        tokio::time::timeout(timeout, async {
            let _ = rx.wait_for(|&stopped| stopped).await;
        })
        .await
        .map_err(|_| {
            format!(
                "Timed out waiting for previous {} capture on '{}' to stop",
                key.kind.label(),
                key.device_name
            )
        })?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn start_shared_capture(
    audio_state: Arc<AudioState>,
    asr_state: Arc<crate::integrations::asr::AsrState>,
    emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
    app_data_dir: std::path::PathBuf,
    kind: CaptureKind,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
    should_record: bool,
) -> Result<LiveCaptureLease, String> {
    let state = audio_state.as_ref();
    let existing_key = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .instance_keys
        .get(&(kind, instance_id.clone()))
        .cloned();
    let (device, resolved_device) = if device_name.is_none() {
        match existing_key.as_ref() {
            Some(key) => resolve_capture_device(kind, &Some(key.device_name.clone()))?,
            None => resolve_capture_device(kind, &device_name)?,
        }
    } else {
        resolve_capture_device(kind, &device_name)?
    };
    let key = CaptureKey {
        kind,
        device_name: resolved_device.clone(),
    };

    let device_lock = audio_state.get_device_start_lock(&key);
    let _permit = device_lock.lock().await;

    wait_for_device_teardown(&audio_state, &key, Duration::from_secs(3)).await?;

    let existing_attachment = {
        let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
        registry.attach_running(&key, kind, &instance_id, should_record)?
    };
    if let Some((lease, recorder_tx)) = existing_attachment {
        info!(
            "[Audio] {} capture already running. source_id={}",
            kind.label(),
            lease.source_id
        );
        if let Err(error) = queue_recording_start(
            Some(&recorder_tx),
            should_record,
            kind.label(),
            &instance_id,
            output_path.clone(),
            || {
                crate::platform::audio_storage::create_history_recording_path_from_dir(
                    &app_data_dir,
                )
            },
        )
        .await
        {
            rollback_capture_attachment(state, kind, &instance_id, &key);
            return Err(error);
        }
        return Ok(lease);
    }

    let direction = match kind {
        CaptureKind::System => CaptureDirection::Output,
        CaptureKind::Microphone => CaptureDirection::Input,
    };

    let (event_tx, event_rx) = tokio::sync::mpsc::channel::<CaptureEvent>(100);
    let device_for_stream = device.clone();
    let resolved_device_for_stream = resolved_device.clone();
    let event_tx_for_stream = event_tx.clone();

    let pipeline_handle = tokio::task::spawn_blocking(move || {
        LiveAudioCapturePipeline::start(
            &device_for_stream,
            &resolved_device_for_stream,
            direction,
            LiveCaptureConfig::default(),
            move |event| match event {
                CaptureEvent::Chunk(chunk) => {
                    match event_tx_for_stream.try_send(CaptureEvent::Chunk(chunk)) {
                        Ok(()) => true,
                        Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                            warn!("[Audio] Live capture buffer full; dropping frame");
                            true
                        }
                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
                    }
                }
                CaptureEvent::Error(err) => {
                    error!("[Audio] Hardware capture stream error: {err}");
                    let _ = event_tx_for_stream.blocking_send(CaptureEvent::Error(err));
                    false
                }
                CaptureEvent::Eof => {
                    let _ = event_tx_for_stream.blocking_send(CaptureEvent::Eof);
                    false
                }
            },
        )
    })
    .await
    .map_err(|e| format!("Failed to spawn audio capture init task: {e}"))?
    .map_err(|e| format!("Failed to start {} capture pipeline: {e}", kind.log_name()))?;

    let pipeline_cell = Arc::new(std::sync::Mutex::new(Some(pipeline_handle)));
    let cell_for_stop = pipeline_cell.clone();
    let cell_for_join = pipeline_cell.clone();
    let stop_signal = AudioStopSignal::with_joiner(
        move || {
            if let Ok(guard) = cell_for_stop.lock()
                && let Some(handle) = guard.as_ref()
            {
                handle.stop();
            }
        },
        move || {
            let handle = cell_for_join.lock().ok().and_then(|mut g| g.take());
            if let Some(handle) = handle {
                drop(handle);
            }
        },
    );

    let (recorder_tx, recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(10);
    let source_generation = state.next_source_generation.fetch_add(1, Ordering::Relaxed);
    let source = LiveSourceEpoch::new(
        format!("desktop-source-{source_generation}"),
        source_generation,
    );
    let sample_cursor = Arc::new(AtomicU64::new(0));

    spawn_capture_worker_task(
        asr_state,
        audio_state.clone(),
        emitter,
        key.clone(),
        source.clone(),
        sample_cursor.clone(),
        stop_signal.clone(),
        event_rx,
        recorder_rx,
    );

    {
        let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
        let mut capture = SharedCaptureState::default();
        capture.commit_start_with_source(
            instance_id.clone(),
            should_record,
            stop_signal,
            recorder_tx.clone(),
            source.clone(),
            sample_cursor,
        );
        registry
            .instance_keys
            .insert((kind, instance_id.clone()), key.clone());
        registry.captures.insert(key.clone(), capture);
        info!(
            "[Audio] {} capture startup committed. source_id={}",
            kind.label(),
            source.source_id
        );
    }

    if let Err(error) = queue_recording_start(
        Some(&recorder_tx),
        should_record,
        kind.label(),
        &instance_id,
        output_path,
        move || {
            crate::platform::audio_storage::create_history_recording_path_from_dir(&app_data_dir)
        },
    )
    .await
    {
        rollback_capture_attachment(state, kind, &instance_id, &key);
        return Err(error);
    }

    Ok(LiveCaptureLease {
        source_id: source.source_id,
        source_generation: source.generation,
        source_cursor: 0,
    })
}

pub async fn stop_shared_capture(
    state: &AudioState,
    kind: CaptureKind,
    instance_id: String,
) -> Result<String, String> {
    let (key, source, detach_result) = {
        let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
        let instance_key = (kind, instance_id.clone());
        let key = registry
            .instance_keys
            .remove(&instance_key)
            .ok_or_else(|| format!("Capture instance '{instance_id}' is not active"))?;
        let capture = registry
            .captures
            .get_mut(&key)
            .ok_or_else(|| "Capture registry entry is missing".to_string())?;
        let detach_result = capture.detach_instance(&instance_id);
        let source = capture.source.clone();
        if detach_result.should_stop_hardware {
            info!(
                "[Audio] {} capture detaching final owner",
                kind.stop_log_label()
            );
            registry.captures.remove(&key);
            let _rx = registry.mark_stopping(key.clone(), source.clone());
        } else {
            debug!("[Audio] {} capture remains active", kind.stop_log_label());
        }
        (key, source, detach_result)
    };
    let was_recording = detach_result.was_recording;

    if detach_result.should_stop_hardware {
        if let Some(signal) = detach_result.stop_signal.as_ref() {
            info!("[Audio] Stopping {} capture...", kind.log_name());
            signal.stop();
        } else {
            match kind {
                CaptureKind::System => debug!("[Audio] Stop requested but not running"),
                CaptureKind::Microphone => debug!("[Audio] Mic stop requested but not running"),
            }
        }
    }

    let mut saved_path = String::new();
    if was_recording {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = match detach_result.recorder_tx.as_ref() {
            Some(recorder_tx) => matches!(
                tokio::time::timeout(
                    Duration::from_millis(1000),
                    recorder_tx.send(RecorderCommand::Stop {
                        owner: instance_id.clone(),
                        completed: tx,
                    }),
                )
                .await,
                Ok(Ok(()))
            ),
            None => false,
        };
        if sent {
            match rx.await {
                Ok(path) => saved_path = path,
                Err(_) => warn!(
                    "[Audio] Failed to receive {} WAV filepath from task",
                    kind.log_name()
                ),
            }
        } else {
            warn!(
                "[Audio] {} recorder stop was requested for {}, but no recorder task was available",
                kind.stop_log_label(),
                instance_id
            );
        }
    }

    if detach_result.should_stop_hardware {
        if let Some(signal) = detach_result.stop_signal.as_ref() {
            let signal_clone = signal.clone();
            let _ = tokio::task::spawn_blocking(move || {
                signal_clone.join_hardware();
            })
            .await;
        }
        if let Ok(mut registry) = state.registry.lock() {
            registry.clear_stopping(&key, &source);
        }
    }

    Ok(saved_path)
}

pub fn rollback_capture_attachment(
    state: &AudioState,
    kind: CaptureKind,
    instance_id: &str,
    key: &CaptureKey,
) {
    let stop_signal = state
        .registry
        .lock()
        .ok()
        .and_then(|mut registry| registry.rollback_attachment(kind, instance_id, key));
    if let Some(stop_signal) = stop_signal {
        stop_signal.stop();
        tokio::task::spawn_blocking(move || {
            stop_signal.join_hardware();
        });
    }
}

pub fn evict_defunct_capture(audio_state: &AudioState, key: &CaptureKey, source: &LiveSourceEpoch) {
    if let Ok(mut registry) = audio_state.registry.lock() {
        let evicted = registry
            .captures
            .get(key)
            .is_some_and(|capture| &capture.source == source);
        if evicted {
            registry.captures.remove(key);
            registry.instance_keys.retain(|_, k| k != key);
        }
        let cleared = registry.clear_stopping(key, source);
        if evicted {
            info!(
                "[Audio] Evicted defunct {} capture (source={})",
                key.kind.label(),
                source.source_id
            );
        }
        let _ = cleared;
    }
}
