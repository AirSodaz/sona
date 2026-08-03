use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};
use rubato::{FftFixedOut, Resampler};
use sona_application::live_transcription::LiveSourceEpoch;
use sona_core::ports::asr::AsrAudioFrame;
use sona_local_asr::audio::LiveWavRecorder;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::thread;
use tauri::{AppHandle, Emitter, Manager, Runtime, Window};

const MICROPHONE_PEAK_EVENT: &str = "microphone-audio";
const SYSTEM_PEAK_EVENT: &str = "system-audio";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum CaptureKind {
    System,
    Microphone,
}

impl CaptureKind {
    fn label(self) -> &'static str {
        match self {
            CaptureKind::System => "System",
            CaptureKind::Microphone => "Microphone",
        }
    }

    fn log_name(self) -> &'static str {
        match self {
            CaptureKind::System => "system",
            CaptureKind::Microphone => "microphone",
        }
    }

    fn peak_event(self) -> &'static str {
        match self {
            CaptureKind::System => SYSTEM_PEAK_EVENT,
            CaptureKind::Microphone => MICROPHONE_PEAK_EVENT,
        }
    }

    fn resampler_error_label(self) -> &'static str {
        match self {
            CaptureKind::System => "System",
            CaptureKind::Microphone => "Mic",
        }
    }

    fn should_record(self, instance_id: &str) -> bool {
        match self {
            CaptureKind::System => instance_id == "record",
            CaptureKind::Microphone => {
                instance_id != "voice-typing" && !instance_id.starts_with("test_")
            }
        }
    }

    fn stream_error_label(self) -> &'static str {
        match self {
            CaptureKind::System => "Stream",
            CaptureKind::Microphone => "Mic stream",
        }
    }

    fn stop_signal_label(self) -> &'static str {
        match self {
            CaptureKind::System => "Stop signal",
            CaptureKind::Microphone => "Mic stop signal",
        }
    }

    fn stop_log_label(self) -> &'static str {
        match self {
            CaptureKind::System => "System",
            CaptureKind::Microphone => "Mic",
        }
    }

    fn no_device_message(self) -> &'static str {
        match self {
            CaptureKind::System => "No output device found",
            CaptureKind::Microphone => "No input device found",
        }
    }

    fn config_error_message(self, err: cpal::Error) -> String {
        match self {
            CaptureKind::System => format!("Failed to get default config: {}", err),
            CaptureKind::Microphone => format!("Failed to get default mic config: {}", err),
        }
    }

    fn resampler_error_message(self, err: rubato::ResamplerConstructionError) -> String {
        match self {
            CaptureKind::System => format!("Failed to create resampler: {}", err),
            CaptureKind::Microphone => format!("Failed to create mic resampler: {}", err),
        }
    }

    fn unsupported_sample_format_message(self) -> &'static str {
        match self {
            CaptureKind::System => "Unsupported sample format",
            CaptureKind::Microphone => "Unsupported mic sample format",
        }
    }

    fn build_stream_error_message(self, err: cpal::Error) -> String {
        match self {
            CaptureKind::System => format!("Failed to build input stream: {}", err),
            CaptureKind::Microphone => format!("Failed to build mic input stream: {}", err),
        }
    }

    fn play_stream_error_message(self, err: cpal::Error) -> String {
        match self {
            CaptureKind::System => format!("Failed to play stream: {}", err),
            CaptureKind::Microphone => format!("Failed to play mic stream: {}", err),
        }
    }

    fn startup_channel_error_message(self, err: std::sync::mpsc::RecvError) -> String {
        match self {
            CaptureKind::System => {
                format!(
                    "System capture startup channel closed before completion: {}",
                    err
                )
            }
            CaptureKind::Microphone => format!(
                "Microphone capture startup channel closed before completion: {}",
                err
            ),
        }
    }
}

pub enum RecorderCommand {
    Start {
        owner: String,
        path: String,
        completed: Sender<Result<(), String>>,
    },
    Stop {
        owner: String,
        completed: tokio::sync::oneshot::Sender<String>,
    },
    SetPaused {
        owner: String,
        paused: bool,
    },
}

struct CaptureWriterState {
    writer: LiveWavRecorder,
    path: String,
    paused: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct CaptureKey {
    kind: CaptureKind,
    device_name: String,
}

#[derive(Default)]
/// Tracks one live hardware capture that can be shared by multiple logical
/// recorder instances. Ownership lives at the instance-id layer, so attaching a
/// second consumer should reuse the same device stream instead of starting a
/// parallel hardware capture.
struct SharedCaptureState {
    stop_signal: Option<Sender<()>>,
    instance_ids: HashSet<String>,
    paused_instances: HashSet<String>,
    recorder_tx: Option<tokio::sync::mpsc::Sender<RecorderCommand>>,
    source: LiveSourceEpoch,
    sample_cursor: std::sync::Arc<AtomicU64>,
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiveCaptureLease {
    pub source_id: String,
    pub source_generation: u64,
    pub source_cursor: u64,
}

#[derive(Default)]
struct CaptureRegistry {
    captures: HashMap<CaptureKey, SharedCaptureState>,
    instance_keys: HashMap<(CaptureKind, String), CaptureKey>,
}

impl CaptureRegistry {
    fn attach_running(
        &mut self,
        key: &CaptureKey,
        kind: CaptureKind,
        instance_id: &str,
    ) -> Result<Option<(LiveCaptureLease, tokio::sync::mpsc::Sender<RecorderCommand>)>, String>
    {
        let instance_key = (kind, instance_id.to_string());
        if let Some(existing_key) = self.instance_keys.get(&instance_key)
            && existing_key != key
        {
            return Err(format!(
                "Capture instance '{instance_id}' is already attached to another device"
            ));
        }
        let Some(capture) = self.captures.get_mut(key) else {
            return Ok(None);
        };
        if !capture.is_running() {
            return Ok(None);
        }
        let recorder_tx = capture
            .recorder_tx
            .clone()
            .ok_or_else(|| "Capture recorder task is unavailable".to_string())?;
        capture.attach_instance(instance_id.to_string());
        let lease = capture.lease();
        self.instance_keys.insert(instance_key, key.clone());
        Ok(Some((lease, recorder_tx)))
    }

    fn rollback_attachment(
        &mut self,
        kind: CaptureKind,
        instance_id: &str,
        key: &CaptureKey,
    ) -> Option<Sender<()>> {
        let instance_key = (kind, instance_id.to_string());
        if self.instance_keys.get(&instance_key) != Some(key) {
            return None;
        }
        self.instance_keys.remove(&instance_key);
        let capture = self.captures.get_mut(key)?;
        let detach = capture.detach_instance(instance_id);
        if detach.should_stop_hardware {
            self.captures.remove(key);
        }
        detach.stop_signal
    }
}

/// Result of detaching one logical owner from a shared hardware capture.
/// Callers only stop the underlying device when the final owner leaves.
struct SharedCaptureDetachResult {
    should_stop_hardware: bool,
    stop_signal: Option<Sender<()>>,
    recorder_tx: Option<tokio::sync::mpsc::Sender<RecorderCommand>>,
}

impl SharedCaptureState {
    fn is_running(&self) -> bool {
        self.stop_signal.is_some()
    }

    fn owners(&self) -> Vec<String> {
        let mut owners = self.instance_ids.iter().cloned().collect::<Vec<_>>();
        owners.sort();
        owners
    }

    fn active_instances(&self) -> Vec<String> {
        let mut active_instances = self
            .instance_ids
            .iter()
            .filter(|instance_id| !self.paused_instances.contains(*instance_id))
            .cloned()
            .collect::<Vec<_>>();
        active_instances.sort();
        active_instances
    }

    fn attach_instance(&mut self, instance_id: String) -> Vec<String> {
        // Re-attaching an existing instance should also make it active again if
        // it had previously been paused.
        self.paused_instances.remove(&instance_id);
        self.instance_ids.insert(instance_id);
        self.owners()
    }

    #[cfg(test)]
    fn commit_start(
        &mut self,
        instance_id: String,
        stop_signal: Sender<()>,
        recorder_tx: tokio::sync::mpsc::Sender<RecorderCommand>,
    ) -> Vec<String> {
        self.commit_start_with_source(
            instance_id,
            stop_signal,
            recorder_tx,
            LiveSourceEpoch::default(),
            std::sync::Arc::new(AtomicU64::new(0)),
        )
    }

    fn commit_start_with_source(
        &mut self,
        instance_id: String,
        stop_signal: Sender<()>,
        recorder_tx: tokio::sync::mpsc::Sender<RecorderCommand>,
        source: LiveSourceEpoch,
        sample_cursor: std::sync::Arc<AtomicU64>,
    ) -> Vec<String> {
        // Starting a brand-new hardware session replaces any stale ownership
        // state so future attach/detach calls describe only the current run.
        self.instance_ids.clear();
        self.paused_instances.clear();
        self.instance_ids.insert(instance_id);
        self.stop_signal = Some(stop_signal);
        self.recorder_tx = Some(recorder_tx);
        self.source = source;
        self.sample_cursor = sample_cursor;
        self.owners()
    }

    fn set_instance_paused(
        &mut self,
        instance_id: &str,
        paused: bool,
    ) -> Result<Vec<String>, String> {
        if !self.instance_ids.contains(instance_id) {
            return Err(format!(
                "Capture instance '{}' is not attached to the active session",
                instance_id
            ));
        }

        if paused {
            self.paused_instances.insert(instance_id.to_string());
        } else {
            self.paused_instances.remove(instance_id);
        }

        Ok(self.active_instances())
    }

    fn detach_instance(&mut self, instance_id: &str) -> SharedCaptureDetachResult {
        self.instance_ids.remove(instance_id);
        self.paused_instances.remove(instance_id);
        let should_stop_hardware = self.instance_ids.is_empty();
        // Keep the recorder channel available while at least one owner still
        // depends on the shared capture. Only the final detach consumes the
        // stop signal and owned resources.
        let recorder_tx = if should_stop_hardware {
            self.recorder_tx.take()
        } else {
            self.recorder_tx.clone()
        };
        let stop_signal = if should_stop_hardware {
            self.stop_signal.take()
        } else {
            None
        };
        SharedCaptureDetachResult {
            should_stop_hardware,
            stop_signal,
            recorder_tx,
        }
    }

    fn lease(&self) -> LiveCaptureLease {
        LiveCaptureLease {
            source_id: self.source.source_id.clone(),
            source_generation: self.source.generation,
            source_cursor: self.sample_cursor.load(Ordering::Acquire),
        }
    }
}

pub struct AudioState {
    start_guard: Mutex<()>,
    registry: Mutex<CaptureRegistry>,
    next_source_generation: AtomicU64,
}

impl Default for AudioState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            start_guard: Mutex::new(()),
            registry: Mutex::new(CaptureRegistry::default()),
            next_source_generation: AtomicU64::new(1),
        }
    }
}

#[derive(serde::Serialize)]
pub struct AudioDevice {
    name: String,
}

pub fn get_system_audio_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.output_devices().map_err(|e| e.to_string())?;

    let result = devices
        .map(|device| AudioDevice {
            name: device.to_string(),
        })
        .collect();

    Ok(result)
}

fn resolve_recording_output_path<F>(
    output_path: Option<String>,
    fallback: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String>,
{
    match output_path {
        Some(path) => Ok(path),
        None => fallback(),
    }
}

fn queue_recording_start(
    recorder_tx: Option<&tokio::sync::mpsc::Sender<RecorderCommand>>,
    should_record: bool,
    capture_label: &str,
    instance_id: &str,
    output_path: Option<String>,
    fallback_path: impl FnOnce() -> Result<String, String>,
) -> Result<(), String> {
    if !should_record {
        return Ok(());
    }

    // Even when we attach to an already-running hardware capture, each logical
    // recording owner still needs its own output file. This command asks the
    // recorder task to begin writing a fresh WAV for that owner.
    let Some(tx) = recorder_tx else {
        return Err(format!(
            "{} recorder is unavailable while starting capture",
            capture_label
        ));
    };

    let wav_filepath = resolve_recording_output_path(output_path, fallback_path)?;
    let (completed, completion) = channel();
    tx.try_send(RecorderCommand::Start {
        owner: instance_id.to_string(),
        path: wav_filepath,
        completed,
    })
    .map_err(|err| format!("Failed to queue {} recorder start: {}", capture_label, err))?;
    completion
        .recv_timeout(std::time::Duration::from_secs(5))
        .map_err(|err| {
            format!(
                "{} recorder start acknowledgement failed: {}",
                capture_label, err
            )
        })?
}

fn update_capture_pause_state(
    capture: &mut SharedCaptureState,
    instance_id: &str,
    paused: bool,
    capture_label: &str,
    should_record: bool,
) -> Result<(), String> {
    if !capture.is_running() {
        return Err(format!("{} capture is not running", capture_label));
    }

    // Pause/resume is tracked per owner. The shared hardware stream keeps
    // running until the last owner detaches, but paused owners are removed from
    // the active set and optionally mirrored to the recorder task.
    capture.set_instance_paused(instance_id, paused)?;

    if should_record {
        let recorder_tx = capture
            .recorder_tx
            .as_ref()
            .ok_or_else(|| format!("{} recorder task is not available", capture_label))?;

        recorder_tx
            .try_send(RecorderCommand::SetPaused {
                owner: instance_id.to_string(),
                paused,
            })
            .map_err(|err| {
                format!(
                    "Failed to {} {} recorder: {}",
                    if paused { "pause" } else { "resume" },
                    capture_label.to_lowercase(),
                    err
                )
            })?;
    }

    Ok(())
}

fn rollback_capture_attachment(
    state: &tauri::State<'_, AudioState>,
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
        let _ = stop_signal.send(());
    }
}

fn spawn_capture_worker_task(
    app: AppHandle,
    key: CaptureKey,
    source: LiveSourceEpoch,
    sample_cursor: std::sync::Arc<AtomicU64>,
    mut task_consumer: impl Consumer<Item = f32> + Send + 'static,
    mut data_rx: tokio::sync::mpsc::Receiver<()>,
    mut recorder_rx: tokio::sync::mpsc::Receiver<RecorderCommand>,
) {
    tauri::async_runtime::spawn(async move {
        let mut writers = HashMap::<String, CaptureWriterState>::new();
        let mut pull_buffer = vec![0.0; 16000];
        let mut sequence = 0_u64;
        let worker_source = CaptureWorkerSource {
            app: &app,
            key: &key,
            source: &source,
        };

        loop {
            tokio::select! {
                biased;
                cmd = recorder_rx.recv() => {
                    match cmd {
                        Some(RecorderCommand::Start { owner, path, completed }) => {
                            if let Some(previous) = writers.remove(&owner) {
                                let _ = previous.writer.finalize();
                            }
                            match LiveWavRecorder::create(std::path::Path::new(&path), 16000) {
                                Ok(w) => {
                                    writers.insert(owner, CaptureWriterState {
                                        writer: w,
                                        path,
                                        paused: false,
                                    });
                                    let _ = completed.send(Ok(()));
                                }
                                Err(error) => {
                                    let _ = completed.send(Err(format!(
                                        "Failed to create {} WAV writer: {}",
                                        key.kind.log_name(),
                                        error
                                    )));
                                }
                            }
                        }
                        Some(RecorderCommand::Stop { owner, completed }) => {
                            let path = if let Some(writer) = writers.remove(&owner) {
                                let path = writer.path;
                                let _ = writer.writer.finalize();
                                path
                            } else {
                                String::new()
                            };
                            let _ = completed.send(path);
                        }
                        Some(RecorderCommand::SetPaused { owner, paused }) => {
                            if let Some(writer) = writers.get_mut(&owner) {
                                writer.paused = paused;
                            }
                        }
                        None => break,
                    }
                }
                opt = data_rx.recv() => {
                    match opt {
                        Some(()) => {
                            drain_capture_worker_chunk(
                                &worker_source,
                                &mut task_consumer,
                                &mut pull_buffer,
                                &mut writers,
                                &mut sequence,
                                sample_cursor.as_ref(),
                            ).await;
                        }
                        None => break,
                    }
                }
            }
        }

        loop {
            let had_chunk = drain_capture_worker_chunk(
                &worker_source,
                &mut task_consumer,
                &mut pull_buffer,
                &mut writers,
                &mut sequence,
                sample_cursor.as_ref(),
            )
            .await;
            if !had_chunk {
                break;
            }
        }

        for writer in writers.into_values() {
            let _ = writer.writer.finalize();
        }
    });
}

struct CaptureWorkerSource<'a> {
    app: &'a AppHandle,
    key: &'a CaptureKey,
    source: &'a LiveSourceEpoch,
}

async fn drain_capture_worker_chunk(
    worker_source: &CaptureWorkerSource<'_>,
    task_consumer: &mut impl Consumer<Item = f32>,
    pull_buffer: &mut [f32],
    writers: &mut HashMap<String, CaptureWriterState>,
    sequence: &mut u64,
    sample_cursor: &AtomicU64,
) -> bool {
    let len = task_consumer.pop_slice(pull_buffer);
    if len == 0 {
        return false;
    }

    let chunk = &pull_buffer[..len];
    for writer in writers.values_mut() {
        if !writer.paused
            && let Err(error) = writer.writer.write_samples(chunk)
        {
            eprintln!("[Audio] Failed to write WAV samples: {error}");
        }
    }

    *sequence = sequence.saturating_add(1);
    let start_sample = sample_cursor.fetch_add(chunk.len() as u64, Ordering::AcqRel);
    let frame = AsrAudioFrame::new(*sequence, start_sample, chunk.to_vec());
    feed_capture_audio(
        worker_source.app,
        worker_source.key,
        worker_source.source,
        frame,
    )
    .await;
    true
}

fn resolve_capture_device_name(
    kind: CaptureKind,
    device_name: &Option<String>,
) -> Result<String, String> {
    let host = cpal::default_host();
    let device = match (kind, device_name) {
        (CaptureKind::System, Some(name)) => host
            .output_devices()
            .map_err(|error| error.to_string())?
            .find(|device| device.to_string() == *name)
            .ok_or_else(|| format!("Requested system audio device is unavailable: {name}"))?,
        (CaptureKind::Microphone, Some(name)) => host
            .input_devices()
            .map_err(|error| error.to_string())?
            .find(|device| device.to_string() == *name)
            .ok_or_else(|| format!("Requested microphone device is unavailable: {name}"))?,
        (CaptureKind::System, None) => host
            .default_output_device()
            .ok_or_else(|| kind.no_device_message().to_string())?,
        (CaptureKind::Microphone, None) => host
            .default_input_device()
            .ok_or_else(|| kind.no_device_message().to_string())?,
    };
    Ok(device.to_string())
}

pub fn start_system_audio_capture(
    app: AppHandle,
    window: Window,
    state: tauri::State<'_, AudioState>,
    _sherpa_state: tauri::State<'_, crate::integrations::asr::AsrState>,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    start_shared_capture(
        app,
        window,
        &state,
        CaptureKind::System,
        device_name,
        instance_id,
        output_path,
    )
    .map(|_| ())
}

fn start_shared_capture(
    app: AppHandle,
    window: Window,
    state: &tauri::State<'_, AudioState>,
    kind: CaptureKind,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<LiveCaptureLease, String> {
    let _start_guard = state.start_guard.lock().map_err(|e| e.to_string())?;
    let existing_key = state
        .registry
        .lock()
        .map_err(|error| error.to_string())?
        .instance_keys
        .get(&(kind, instance_id.clone()))
        .cloned();
    let resolved_device = if device_name.is_none() {
        match existing_key.as_ref() {
            Some(key) => key.device_name.clone(),
            None => resolve_capture_device_name(kind, &device_name)?,
        }
    } else {
        resolve_capture_device_name(kind, &device_name)?
    };
    let key = CaptureKey {
        kind,
        device_name: resolved_device.clone(),
    };

    let existing_attachment = {
        let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
        registry.attach_running(&key, kind, &instance_id)?
    };
    if let Some((lease, recorder_tx)) = existing_attachment {
        println!(
            "[Audio] {} capture already running. source_id={}",
            kind.label(),
            lease.source_id
        );
        if let Err(error) = queue_recording_start(
            Some(&recorder_tx),
            kind.should_record(&instance_id),
            kind.label(),
            &instance_id,
            output_path.clone(),
            || crate::platform::audio_storage::create_history_recording_path_for_app(&app),
        ) {
            rollback_capture_attachment(state, kind, &instance_id, &key);
            return Err(error);
        }
        return Ok(lease);
    }

    let (stop_tx, rx) = channel::<()>();
    let task_rb = HeapRb::<f32>::new(16000 * 5);
    let (task_producer, task_consumer) = task_rb.split();
    let (data_tx, data_rx) = tokio::sync::mpsc::channel::<()>(100);
    let (recorder_tx, recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(10);
    let (startup_tx, startup_rx) = channel::<Result<String, String>>();
    let source_generation = state.next_source_generation.fetch_add(1, Ordering::Relaxed);
    let source = LiveSourceEpoch::new(
        format!("desktop-source-{source_generation}"),
        source_generation,
    );
    let sample_cursor = std::sync::Arc::new(AtomicU64::new(0));

    spawn_capture_worker_task(
        app.clone(),
        key.clone(),
        source.clone(),
        sample_cursor.clone(),
        task_consumer,
        data_rx,
        recorder_rx,
    );
    spawn_cpal_startup_thread(
        window,
        kind,
        Some(resolved_device),
        rx,
        startup_tx,
        data_tx,
        task_producer,
    );

    match startup_rx.recv() {
        Ok(Ok(_)) => {}
        Ok(Err(err)) => return Err(err),
        Err(err) => return Err(kind.startup_channel_error_message(err)),
    }

    {
        let mut registry = state.registry.lock().map_err(|e| e.to_string())?;
        let mut capture = SharedCaptureState::default();
        capture.commit_start_with_source(
            instance_id.clone(),
            stop_tx,
            recorder_tx.clone(),
            source.clone(),
            sample_cursor,
        );
        registry
            .instance_keys
            .insert((kind, instance_id.clone()), key.clone());
        registry.captures.insert(key.clone(), capture);
        println!(
            "[Audio] {} capture startup committed. source_id={}",
            kind.label(),
            source.source_id
        );
    }

    if let Err(error) = queue_recording_start(
        Some(&recorder_tx),
        kind.should_record(&instance_id),
        kind.label(),
        &instance_id,
        output_path,
        || crate::platform::audio_storage::create_history_recording_path_for_app(&app),
    ) {
        rollback_capture_attachment(state, kind, &instance_id, &key);
        return Err(error);
    }

    Ok(LiveCaptureLease {
        source_id: source.source_id,
        source_generation: source.generation,
        source_cursor: 0,
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_cpal_startup_thread<R: Runtime + 'static>(
    window: Window<R>,
    kind: CaptureKind,
    device_name: Option<String>,
    rx: std::sync::mpsc::Receiver<()>,
    startup_tx: Sender<Result<String, String>>,
    data_tx: tokio::sync::mpsc::Sender<()>,
    mut task_producer: impl Producer<Item = f32> + Send + 'static,
) {
    thread::spawn(move || {
        let fail_start = |message: String| {
            eprintln!(
                "[Audio] Failed to start {} capture: {}",
                kind.log_name(),
                message
            );
            let _ = startup_tx.send(Err(message));
        };

        let err_fn = move |err| eprintln!("[Audio] {} error: {}", kind.stream_error_label(), err);
        let host = cpal::default_host();
        let device = match (kind, device_name.as_ref()) {
            (CaptureKind::System, Some(name)) => host
                .output_devices()
                .ok()
                .and_then(|mut devices| devices.find(|device| device.to_string() == *name)),
            (CaptureKind::Microphone, Some(name)) => host
                .input_devices()
                .ok()
                .and_then(|mut devices| devices.find(|device| device.to_string() == *name)),
            (CaptureKind::System, None) => host.default_output_device(),
            (CaptureKind::Microphone, None) => host.default_input_device(),
        };

        let Some(device) = device else {
            fail_start(kind.no_device_message().to_string());
            return;
        };
        let resolved_device_name = device.to_string();

        let supported_config = match kind {
            CaptureKind::System => device.default_output_config(),
            CaptureKind::Microphone => device.default_input_config(),
        };
        let supported_config = match supported_config {
            Ok(c) => c,
            Err(e) => {
                fail_start(kind.config_error_message(e));
                return;
            }
        };

        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate;
        let channels = config.channels;
        let chunk_size_out = 1024;

        let mut resampler =
            match FftFixedOut::<f32>::new(sample_rate as usize, 16000, chunk_size_out, 2, 1) {
                Ok(r) => r,
                Err(e) => {
                    fail_start(kind.resampler_error_message(e));
                    return;
                }
            };

        let input_frames_next = resampler.input_frames_next();
        let rb = HeapRb::<f32>::new(input_frames_next * 4);
        let (mut producer, mut consumer) = rb.split();
        let mut input_buffer: Vec<Vec<f32>> = vec![vec![0.0; input_frames_next]; 1];
        let mut output_buffer: Vec<Vec<f32>> = vec![vec![0.0; chunk_size_out]; 1];

        let stream_result = match sample_format {
            SampleFormat::F32 => {
                let window_clone = window.clone();
                device.build_input_stream(
                    config,
                    move |data: &[f32], _: &_| {
                        process_capture_audio(
                            kind,
                            data,
                            channels as usize,
                            &mut producer,
                            &mut consumer,
                            &mut resampler,
                            &mut input_buffer,
                            &mut output_buffer,
                            &window_clone,
                            &data_tx,
                            &mut task_producer,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let window_clone = window.clone();
                device.build_input_stream(
                    config,
                    move |data: &[i16], _: &_| {
                        let data_f32: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        process_capture_audio(
                            kind,
                            &data_f32,
                            channels as usize,
                            &mut producer,
                            &mut consumer,
                            &mut resampler,
                            &mut input_buffer,
                            &mut output_buffer,
                            &window_clone,
                            &data_tx,
                            &mut task_producer,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let window_clone = window.clone();
                device.build_input_stream(
                    config,
                    move |data: &[u16], _: &_| {
                        let data_f32: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();
                        process_capture_audio(
                            kind,
                            &data_f32,
                            channels as usize,
                            &mut producer,
                            &mut consumer,
                            &mut resampler,
                            &mut input_buffer,
                            &mut output_buffer,
                            &window_clone,
                            &data_tx,
                            &mut task_producer,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                fail_start(kind.unsupported_sample_format_message().to_string());
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                fail_start(kind.build_stream_error_message(e));
                return;
            }
        };

        if let Err(e) = stream.play() {
            fail_start(kind.play_stream_error_message(e));
            return;
        }

        if startup_tx.send(Ok(resolved_device_name.clone())).is_err() {
            return;
        }

        let _ = rx.recv();
        println!("[Audio] {} capture stopped", kind.stop_signal_label());
    });
}

pub fn get_microphone_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;

    let result = devices
        .map(|device| AudioDevice {
            name: device.to_string(),
        })
        .collect();

    Ok(result)
}

pub fn start_microphone_capture(
    app: AppHandle,
    window: Window,
    state: tauri::State<'_, AudioState>,
    _sherpa_state: tauri::State<'_, crate::integrations::asr::AsrState>,
    device_name: Option<String>,
    instance_id: String,
    output_path: Option<String>,
) -> Result<(), String> {
    start_shared_capture(
        app,
        window,
        &state,
        CaptureKind::Microphone,
        device_name,
        instance_id,
        output_path,
    )
    .map(|_| ())
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn start_native_live_capture(
    app: AppHandle,
    window: Window,
    state: &tauri::State<'_, AudioState>,
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
    start_shared_capture(
        app,
        window,
        state,
        kind,
        device_name,
        consumer_id,
        output_path,
    )
}

pub(crate) async fn stop_native_live_capture(
    state: &tauri::State<'_, AudioState>,
    source_kind: &str,
    consumer_id: String,
) -> Result<String, String> {
    let kind = match source_kind {
        "system" => CaptureKind::System,
        "microphone" => CaptureKind::Microphone,
        _ => return Err(format!("Unsupported native capture source: {source_kind}")),
    };
    stop_shared_capture(state, kind, consumer_id).await
}

pub(crate) fn set_native_live_capture_paused(
    state: &tauri::State<'_, AudioState>,
    source_kind: &str,
    consumer_id: &str,
    paused: bool,
) -> Result<LiveCaptureLease, String> {
    let kind = match source_kind {
        "system" => CaptureKind::System,
        "microphone" => CaptureKind::Microphone,
        _ => return Err(format!("Unsupported native capture source: {source_kind}")),
    };
    let mut registry = state.registry.lock().map_err(|error| error.to_string())?;
    let key = registry
        .instance_keys
        .get(&(kind, consumer_id.to_string()))
        .cloned()
        .ok_or_else(|| format!("Capture instance '{consumer_id}' is not active"))?;
    let capture = registry
        .captures
        .get_mut(&key)
        .ok_or_else(|| "Capture registry entry is missing".to_string())?;
    update_capture_pause_state(
        capture,
        consumer_id,
        paused,
        kind.label(),
        kind.should_record(consumer_id),
    )?;
    Ok(capture.lease())
}

async fn feed_capture_audio(
    app: &AppHandle,
    key: &CaptureKey,
    source: &LiveSourceEpoch,
    frame: AsrAudioFrame,
) {
    let instance_ids = {
        let audio_state = app.state::<AudioState>();
        let registry = match audio_state.registry.lock() {
            Ok(registry) => registry,
            Err(_) => return,
        };
        let Some(capture) = registry.captures.get(key) else {
            return;
        };
        if &capture.source != source {
            return;
        }
        capture.active_instances()
    };

    if instance_ids.is_empty() {
        return;
    }

    let asr_state = app.state::<crate::integrations::asr::AsrState>();
    if let Err(error) = asr_state
        .live_coordinator()
        .feed_source(source, frame.clone())
        .await
        && !error.message.contains("retired")
    {
        eprintln!(
            "[Audio] Failed to feed live source {}: {error}",
            source.source_id
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn process_capture_audio<R: Runtime>(
    kind: CaptureKind,
    data: &[f32],
    channels: usize,
    producer: &mut impl Producer<Item = f32>,
    consumer: &mut impl Consumer<Item = f32>,
    resampler: &mut FftFixedOut<f32>,
    input_buffer: &mut [Vec<f32>],
    output_buffer: &mut [Vec<f32>],
    window: &Window<R>,
    data_tx: &tokio::sync::mpsc::Sender<()>,
    task_producer: &mut impl Producer<Item = f32>,
) {
    for frame in data.chunks(channels) {
        let mut sum = 0.0;
        for sample in frame {
            sum += sample;
        }
        let mono_sample = sum / channels as f32;

        let _ = producer.try_push(mono_sample);
    }

    while consumer.occupied_len() >= resampler.input_frames_next() {
        let input_frames_needed = resampler.input_frames_next();
        input_buffer[0].resize(input_frames_needed, 0.0);
        let chunk_slice = &mut input_buffer[0];
        let _read = consumer.pop_slice(chunk_slice);

        let result = resampler.process_into_buffer(input_buffer, output_buffer, None);

        match result {
            Ok((_in_len, out_len)) => {
                if out_len > 0 {
                    let output_f32 = &output_buffer[0][..out_len];

                    let _ = task_producer.push_slice(output_f32);
                    let _ = data_tx.try_send(());

                    let mut max_abs = 0.0_f32;
                    for &sample in output_f32 {
                        let abs_val = sample.abs();
                        if abs_val > max_abs {
                            max_abs = abs_val;
                        }
                    }
                    let peak_i16 = (max_abs.clamp(0.0, 1.0) * 32767.0) as i16;
                    let _ = window.app_handle().emit(kind.peak_event(), peak_i16);
                }
            }
            Err(e) => {
                eprintln!(
                    "[Audio] {} resampler error: {}",
                    kind.resampler_error_label(),
                    e
                );
            }
        }
    }
}

pub async fn stop_microphone_capture(
    state: tauri::State<'_, AudioState>,
    instance_id: String,
) -> Result<String, String> {
    stop_shared_capture(&state, CaptureKind::Microphone, instance_id).await
}

pub async fn stop_system_audio_capture(
    state: tauri::State<'_, AudioState>,
    instance_id: String,
) -> Result<String, String> {
    stop_shared_capture(&state, CaptureKind::System, instance_id).await
}

async fn stop_shared_capture(
    state: &tauri::State<'_, AudioState>,
    kind: CaptureKind,
    instance_id: String,
) -> Result<String, String> {
    let was_recording = kind.should_record(&instance_id);
    let detach_result = {
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
        if detach_result.should_stop_hardware {
            println!(
                "[Audio] {} capture detaching final owner",
                kind.stop_log_label()
            );
        } else {
            println!("[Audio] {} capture remains active", kind.stop_log_label());
        }
        if detach_result.should_stop_hardware {
            registry.captures.remove(&key);
        }
        detach_result
    };

    let mut saved_path = String::new();
    if was_recording {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = detach_result
            .recorder_tx
            .as_ref()
            .map(|recorder_tx| {
                recorder_tx
                    .try_send(RecorderCommand::Stop {
                        owner: instance_id.clone(),
                        completed: tx,
                    })
                    .is_ok()
            })
            .unwrap_or(false);

        if sent {
            match rx.await {
                Ok(path) => saved_path = path,
                Err(_) => eprintln!(
                    "[Audio] Failed to receive {} WAV filepath from task",
                    kind.log_name()
                ),
            }
        } else {
            eprintln!(
                "[Audio] {} recorder stop was requested for {}, but no recorder task was available",
                kind.stop_log_label(),
                instance_id
            );
        }
    }

    if !detach_result.should_stop_hardware {
        return Ok(saved_path);
    }

    if let Some(tx) = detach_result.stop_signal {
        println!("[Audio] Stopping {} capture...", kind.log_name());
        let _ = tx.send(());
    } else {
        match kind {
            CaptureKind::System => println!("[Audio] Stop requested but not running"),
            CaptureKind::Microphone => println!("[Audio] Mic stop requested but not running"),
        }
    }

    Ok(saved_path)
}

pub fn set_system_audio_capture_paused(
    state: tauri::State<'_, AudioState>,
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
    update_capture_pause_state(
        capture,
        &instance_id,
        paused,
        kind.label(),
        kind.should_record(&instance_id),
    )
}

pub fn set_microphone_capture_paused(
    state: tauri::State<'_, AudioState>,
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
    update_capture_pause_state(
        capture,
        &instance_id,
        paused,
        kind.label(),
        kind.should_record(&instance_id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn running_capture(
        owner: &str,
        source_generation: u64,
    ) -> (
        SharedCaptureState,
        std::sync::mpsc::Receiver<()>,
        tokio::sync::mpsc::Receiver<RecorderCommand>,
    ) {
        let (stop_tx, stop_rx) = channel::<()>();
        let (recorder_tx, recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(4);
        let mut capture = SharedCaptureState::default();
        capture.commit_start_with_source(
            owner.to_string(),
            stop_tx,
            recorder_tx,
            LiveSourceEpoch::new(
                format!("desktop-source-{source_generation}"),
                source_generation,
            ),
            std::sync::Arc::new(AtomicU64::new(0)),
        );
        (capture, stop_rx, recorder_rx)
    }

    #[test]
    fn shared_capture_state_only_becomes_running_after_commit() {
        let mut capture = SharedCaptureState::default();
        assert!(!capture.is_running());
        assert!(capture.owners().is_empty());
        assert!(capture.active_instances().is_empty());
        assert!(capture.recorder_tx.is_none());

        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        let owners = capture.commit_start("record".to_string(), stop_tx, recorder_tx);

        assert!(capture.is_running());
        assert_eq!(owners, vec!["record".to_string()]);
        assert_eq!(capture.active_instances(), vec!["record".to_string()]);
        assert!(capture.recorder_tx.is_some());
    }

    #[test]
    fn shared_capture_state_failed_start_leaves_no_runtime_state() {
        let capture = SharedCaptureState::default();

        assert!(!capture.is_running());
        assert!(capture.owners().is_empty());
        assert!(capture.active_instances().is_empty());
        assert!(capture.recorder_tx.is_none());
    }

    #[test]
    fn shared_capture_state_attach_adds_owner_to_running_capture() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start("voice-typing".to_string(), stop_tx, recorder_tx);

        let owners = capture.attach_instance("record".to_string());

        assert_eq!(
            owners,
            vec!["record".to_string(), "voice-typing".to_string()]
        );
        assert_eq!(
            capture.active_instances(),
            vec!["record".to_string(), "voice-typing".to_string()]
        );
        assert!(capture.is_running());
    }

    #[test]
    fn shared_capture_state_detach_last_owner_clears_runtime_state() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start("record".to_string(), stop_tx, recorder_tx);

        let detach_result = capture.detach_instance("record");

        assert!(detach_result.should_stop_hardware);
        assert!(detach_result.stop_signal.is_some());
        assert!(detach_result.recorder_tx.is_some());
        assert!(!capture.is_running());
        assert!(capture.owners().is_empty());
        assert!(capture.active_instances().is_empty());
        assert!(capture.recorder_tx.is_none());
    }

    #[test]
    fn shared_capture_state_pause_filters_active_instances_without_detaching_owner() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start("voice-typing".to_string(), stop_tx, recorder_tx);
        capture.attach_instance("record".to_string());

        let active_instances = capture.set_instance_paused("record", true).unwrap();

        assert_eq!(
            capture.owners(),
            vec!["record".to_string(), "voice-typing".to_string()]
        );
        assert_eq!(active_instances, vec!["voice-typing".to_string()]);
        assert_eq!(capture.active_instances(), vec!["voice-typing".to_string()]);
    }

    #[test]
    fn shared_capture_state_detaches_preview_without_stopping_persistent_owner() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start("voice-typing".to_string(), stop_tx, recorder_tx);
        capture.attach_instance("test_mic".to_string());

        let preview_detach = capture.detach_instance("test_mic");

        assert!(!preview_detach.should_stop_hardware);
        assert!(preview_detach.stop_signal.is_none());
        assert_eq!(capture.owners(), vec!["voice-typing".to_string()]);
        assert!(capture.is_running());

        let persistent_detach = capture.detach_instance("voice-typing");

        assert!(persistent_detach.should_stop_hardware);
        assert!(persistent_detach.stop_signal.is_some());
        assert!(!capture.is_running());
    }

    #[test]
    fn shared_capture_state_detach_clears_paused_instance_state() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start("record".to_string(), stop_tx, recorder_tx);
        capture.set_instance_paused("record", true).unwrap();

        let detach_result = capture.detach_instance("record");

        assert!(detach_result.should_stop_hardware);
        assert!(capture.paused_instances.is_empty());
        assert!(capture.active_instances().is_empty());
    }

    #[test]
    fn capture_registry_shares_same_device_and_keeps_different_devices_independent() {
        let microphone_a = CaptureKey {
            kind: CaptureKind::Microphone,
            device_name: "device-a".to_string(),
        };
        let microphone_b = CaptureKey {
            kind: CaptureKind::Microphone,
            device_name: "device-b".to_string(),
        };
        let (capture_a, _stop_a, _recorder_a) = running_capture("record", 11);
        let (capture_b, _stop_b, _recorder_b) = running_capture("voice-typing", 12);
        let mut registry = CaptureRegistry::default();
        registry.captures.insert(microphone_a.clone(), capture_a);
        registry.captures.insert(microphone_b.clone(), capture_b);
        registry.instance_keys.insert(
            (CaptureKind::Microphone, "record".to_string()),
            microphone_a.clone(),
        );
        registry.instance_keys.insert(
            (CaptureKind::Microphone, "voice-typing".to_string()),
            microphone_b.clone(),
        );

        let (shared_lease, _) = registry
            .attach_running(&microphone_a, CaptureKind::Microphone, "caption")
            .unwrap()
            .unwrap();

        assert_eq!(registry.captures.len(), 2);
        assert_eq!(shared_lease.source_generation, 11);
        assert_eq!(
            registry.captures[&microphone_a].owners(),
            vec!["caption".to_string(), "record".to_string()]
        );
        assert_eq!(
            registry.captures[&microphone_b].owners(),
            vec!["voice-typing".to_string()]
        );
    }

    #[test]
    fn capture_registry_rollback_detaches_only_target_and_stops_last_owner() {
        let key = CaptureKey {
            kind: CaptureKind::Microphone,
            device_name: "device-a".to_string(),
        };
        let (mut capture, stop_rx, _recorder_rx) = running_capture("record", 13);
        capture.attach_instance("caption".to_string());
        let mut registry = CaptureRegistry::default();
        registry.captures.insert(key.clone(), capture);
        for owner in ["record", "caption"] {
            registry
                .instance_keys
                .insert((CaptureKind::Microphone, owner.to_string()), key.clone());
        }

        assert!(
            registry
                .rollback_attachment(CaptureKind::Microphone, "caption", &key)
                .is_none()
        );
        assert_eq!(registry.captures[&key].owners(), vec!["record".to_string()]);
        let stop_signal = registry
            .rollback_attachment(CaptureKind::Microphone, "record", &key)
            .unwrap();
        stop_signal.send(()).unwrap();

        assert!(registry.captures.is_empty());
        assert!(registry.instance_keys.is_empty());
        assert!(stop_rx.recv().is_ok());
    }

    #[test]
    fn recording_start_propagates_writer_creation_failure() {
        let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel(1);
        let worker = thread::spawn(move || {
            let RecorderCommand::Start { completed, .. } = recorder_rx.blocking_recv().unwrap()
            else {
                panic!("expected recorder start command");
            };
            completed
                .send(Err("writer creation failed".to_string()))
                .unwrap();
        });

        let error = queue_recording_start(
            Some(&recorder_tx),
            true,
            "Microphone",
            "record",
            Some("recording.wav".to_string()),
            || Err("fallback must not run".to_string()),
        )
        .unwrap_err();
        worker.join().unwrap();

        assert_eq!(error, "writer creation failed");
    }

    #[test]
    fn resolve_recording_output_path_prefers_explicit_output_path() {
        let resolved = resolve_recording_output_path(Some("C:/tmp/custom.wav".to_string()), || {
            Err("fallback should not run".to_string())
        })
        .unwrap();

        assert_eq!(resolved, "C:/tmp/custom.wav");
    }

    #[test]
    fn resolve_recording_output_path_uses_fallback_when_missing() {
        let resolved =
            resolve_recording_output_path(None, || Ok("generated.wav".to_string())).unwrap();

        assert_eq!(resolved, "generated.wav");
    }
}
