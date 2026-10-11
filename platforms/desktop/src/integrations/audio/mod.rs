use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};

pub mod coordinator;
pub mod devices;
pub mod recorder;
pub mod registry;
pub mod types;
pub mod worker;

#[cfg(test)]
mod tests;

pub use devices::{AudioDevice, get_microphone_devices, get_system_audio_devices};
pub use recorder::{RecorderCommand, resolve_recording_output_path};
pub use types::{
    AUDIO_CAPTURE_ERROR_EVENT, AudioCaptureError, AudioStopSignal, CaptureKey, CaptureKind,
    LiveCaptureLease, MICROPHONE_PEAK_EVENT, SYSTEM_PEAK_EVENT,
};

#[derive(Clone)]
pub struct AudioState {
    pub(crate) registry: Arc<Mutex<registry::CaptureRegistry>>,
    pub(crate) device_start_locks:
        Arc<Mutex<HashMap<types::CaptureKey, Arc<tokio::sync::Mutex<()>>>>>,
    pub(crate) next_source_generation: Arc<AtomicU64>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(registry::CaptureRegistry::default())),
            device_start_locks: Arc::new(Mutex::new(HashMap::new())),
            next_source_generation: Arc::new(AtomicU64::new(1)),
        }
    }

    pub fn get_device_start_lock(&self, key: &types::CaptureKey) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.device_start_locks.lock().unwrap();
        locks
            .entry(key.clone())
            .or_insert_with(|| Arc::new(tokio::sync::Mutex::new(())))
            .clone()
    }

    pub fn has_active_captures(&self) -> bool {
        match self.registry.lock() {
            Ok(registry) => registry.has_active_captures(),
            Err(_) => false,
        }
    }
}

pub async fn start_system_audio_capture(
    audio_state: Arc<AudioState>,
    asr_state: Arc<crate::integrations::asr::AsrState>,
    emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
    app_data_dir: PathBuf,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    coordinator::start_shared_capture(
        audio_state,
        asr_state,
        emitter,
        app_data_dir,
        CaptureKind::System,
        device_name,
        instance_id,
        output_path,
        true,
    )
    .await
    .map(|_| ())
}

pub async fn start_microphone_capture(
    audio_state: Arc<AudioState>,
    asr_state: Arc<crate::integrations::asr::AsrState>,
    emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
    app_data_dir: PathBuf,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    coordinator::start_shared_capture(
        audio_state,
        asr_state,
        emitter,
        app_data_dir,
        CaptureKind::Microphone,
        device_name,
        instance_id,
        output_path,
        true,
    )
    .await
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn start_native_live_capture(
    audio_state: Arc<AudioState>,
    asr_state: Arc<crate::integrations::asr::AsrState>,
    emitter: Arc<dyn crate::platform::event::EventEmitterPort>,
    app_data_dir: PathBuf,
    source_kind: &str,
    device_name: Option<String>,
    consumer_id: String,
    output_path: Option<String>,
) -> Result<LiveCaptureLease, String> {
    let kind = match source_kind {
        "system" => CaptureKind::System,
        "microphone" => CaptureKind::Microphone,
        _ => return Err(format!("Unsupported native capture source: {source_kind}")),
    };
    let should_record = output_path.is_some();
    coordinator::start_shared_capture(
        audio_state,
        asr_state,
        emitter,
        app_data_dir,
        kind,
        device_name,
        consumer_id,
        output_path,
        should_record,
    )
    .await
}

pub async fn stop_system_audio_capture(
    state: &AudioState,
    instance_id: String,
) -> Result<String, String> {
    coordinator::stop_shared_capture(state, CaptureKind::System, instance_id).await
}

pub async fn stop_microphone_capture(
    state: &AudioState,
    instance_id: String,
) -> Result<String, String> {
    coordinator::stop_shared_capture(state, CaptureKind::Microphone, instance_id).await
}

pub(crate) async fn stop_native_live_capture(
    state: &AudioState,
    source_kind: &str,
    consumer_id: String,
) -> Result<String, String> {
    let kind = match source_kind {
        "system" => CaptureKind::System,
        "microphone" => CaptureKind::Microphone,
        _ => return Err(format!("Unsupported native capture source: {source_kind}")),
    };
    coordinator::stop_shared_capture(state, kind, consumer_id).await
}

pub async fn stop_all_captures(state: &AudioState) {
    let instances: Vec<(CaptureKind, String)> = {
        if let Ok(registry) = state.registry.lock() {
            registry.instance_keys.keys().cloned().collect()
        } else {
            Vec::new()
        }
    };
    for (kind, instance_id) in instances {
        let _ = coordinator::stop_shared_capture(state, kind, instance_id).await;
    }
}

pub fn set_system_audio_capture_paused(
    state: &AudioState,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    let kind = CaptureKind::System;
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    let key = registry
        .instance_keys
        .get(&(kind, instance_id.clone()))
        .cloned()
        .ok_or_else(|| format!("Capture instance '{instance_id}' is not active"))?;
    let capture = registry
        .captures
        .get_mut(&key)
        .ok_or_else(|| "Capture registry entry is missing".to_string())?;
    let should_record = capture.recording_instances.contains(&instance_id);
    registry::update_capture_pause_state(capture, &instance_id, paused, kind.label(), should_record)
}

pub fn set_microphone_capture_paused(
    state: &AudioState,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    let kind = CaptureKind::Microphone;
    let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
    let key = registry
        .instance_keys
        .get(&(kind, instance_id.clone()))
        .cloned()
        .ok_or_else(|| format!("Capture instance '{instance_id}' is not active"))?;
    let capture = registry
        .captures
        .get_mut(&key)
        .ok_or_else(|| "Capture registry entry is missing".to_string())?;
    let should_record = capture.recording_instances.contains(&instance_id);
    registry::update_capture_pause_state(capture, &instance_id, paused, kind.label(), should_record)
}

pub(crate) fn set_native_live_capture_paused(
    state: &AudioState,
    source_kind: &str,
    consumer_id: &str,
    paused: bool,
) -> Result<LiveCaptureLease, AudioCaptureError> {
    let kind = match source_kind {
        "system" => CaptureKind::System,
        "microphone" => CaptureKind::Microphone,
        _ => {
            return Err(AudioCaptureError::UnsupportedSource(
                source_kind.to_string(),
            ));
        }
    };
    let mut registry = state
        .registry
        .lock()
        .map_err(|error| AudioCaptureError::LockPoisoned(error.to_string()))?;
    let key = registry
        .instance_keys
        .get(&(kind, consumer_id.to_string()))
        .cloned()
        .ok_or_else(|| AudioCaptureError::InstanceNotActive(consumer_id.to_string()))?;
    let capture = registry
        .captures
        .get_mut(&key)
        .ok_or(AudioCaptureError::MissingRegistryEntry)?;
    let should_record = capture.recording_instances.contains(consumer_id);
    registry::update_capture_pause_state(capture, consumer_id, paused, kind.label(), should_record)
        .map_err(AudioCaptureError::PauseState)?;
    Ok(capture.lease())
}
