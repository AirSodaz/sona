use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use ringbuf::traits::{Consumer, Producer, Split};
use ringbuf::HeapRb;
use rubato::{FftFixedOut, Resampler};
use std::collections::HashSet;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;
use tauri::{AppHandle, Emitter, Manager, Runtime, Window};

pub enum RecorderCommand {
    Start(String), // filepath
    Stop(tokio::sync::oneshot::Sender<String>),
    SetPaused(bool),
}

#[derive(Default)]
struct SharedCaptureState {
    stop_signal: Option<Sender<()>>,
    instance_ids: HashSet<String>,
    paused_instances: HashSet<String>,
    recorder_tx: Option<tokio::sync::mpsc::Sender<RecorderCommand>>,
    active_device_name: Option<String>,
}

struct SharedCaptureDetachResult {
    should_stop_hardware: bool,
    remaining_instances: Vec<String>,
    stop_signal: Option<Sender<()>>,
    recorder_tx: Option<tokio::sync::mpsc::Sender<RecorderCommand>>,
    active_device_name: Option<String>,
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
        self.paused_instances.remove(&instance_id);
        self.instance_ids.insert(instance_id);
        self.owners()
    }

    fn commit_start(
        &mut self,
        instance_id: String,
        active_device_name: String,
        stop_signal: Sender<()>,
        recorder_tx: tokio::sync::mpsc::Sender<RecorderCommand>,
    ) -> Vec<String> {
        self.instance_ids.clear();
        self.paused_instances.clear();
        self.instance_ids.insert(instance_id);
        self.active_device_name = Some(active_device_name);
        self.stop_signal = Some(stop_signal);
        self.recorder_tx = Some(recorder_tx);
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
        let remaining_instances = self.owners();
        let should_stop_hardware = remaining_instances.is_empty();
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
        let active_device_name = if should_stop_hardware {
            self.active_device_name.take()
        } else {
            self.active_device_name.clone()
        };

        SharedCaptureDetachResult {
            should_stop_hardware,
            remaining_instances,
            stop_signal,
            recorder_tx,
            active_device_name,
        }
    }

    fn active_device_label(&self) -> &str {
        self.active_device_name.as_deref().unwrap_or("unknown")
    }
}

pub struct AudioState {
    system_start_guard: Mutex<()>,
    mic_start_guard: Mutex<()>,
    system_capture: Mutex<SharedCaptureState>,
    mic_capture: Mutex<SharedCaptureState>,
    mic_boost: Mutex<f32>,
}

impl Default for AudioState {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            system_start_guard: Mutex::new(()),
            mic_start_guard: Mutex::new(()),
            system_capture: Mutex::new(SharedCaptureState::default()),
            mic_capture: Mutex::new(SharedCaptureState::default()),
            mic_boost: Mutex::new(1.0),
        }
    }
}

#[derive(serde::Serialize)]
pub struct AudioDevice {
    name: String,
}

#[tauri::command]
pub fn get_system_audio_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.output_devices().map_err(|e| e.to_string())?;

    let result = devices
        .filter_map(|device| device.name().ok())
        .map(|name| AudioDevice { name })
        .collect();

    Ok(result)
}

fn requested_device_label(device_name: &Option<String>) -> String {
    device_name.as_deref().unwrap_or("default").to_string()
}

fn should_record_microphone(instance_id: &str) -> bool {
    instance_id != "voice-typing" && !instance_id.starts_with("test_")
}

fn should_record_system(instance_id: &str) -> bool {
    instance_id == "record"
}

fn create_history_recording_path<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let app_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
    let history_dir = app_data_dir.join("history");
    if !history_dir.exists() {
        std::fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
    }

    let wav_filename = format!("{}.wav", uuid::Uuid::new_v4());
    let wav_filepath = history_dir.join(&wav_filename);
    Ok(wav_filepath.to_string_lossy().into_owned())
}

fn queue_recording_start<R: Runtime>(
    app: &AppHandle<R>,
    recorder_tx: Option<&tokio::sync::mpsc::Sender<RecorderCommand>>,
    should_record: bool,
    capture_label: &str,
    instance_id: &str,
) -> Result<(), String> {
    if !should_record {
        return Ok(());
    }

    let Some(tx) = recorder_tx else {
        eprintln!(
            "[Audio] {} recorder missing while starting capture file for instance {}",
            capture_label, instance_id
        );
        return Ok(());
    };

    let wav_filepath = create_history_recording_path(app)?;
    if let Err(err) = tx.try_send(RecorderCommand::Start(wav_filepath.clone())) {
        eprintln!(
            "[Audio] Failed to queue {} recorder start for instance {} at {}: {}",
            capture_label, instance_id, wav_filepath, err
        );
    }

    Ok(())
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

    let active_instances = capture.set_instance_paused(instance_id, paused)?;

    if should_record {
        let recorder_tx = capture
            .recorder_tx
            .as_ref()
            .ok_or_else(|| format!("{} recorder task is not available", capture_label))?;

        recorder_tx
            .try_send(RecorderCommand::SetPaused(paused))
            .map_err(|err| {
                format!(
                    "Failed to {} {} recorder: {}",
                    if paused { "pause" } else { "resume" },
                    capture_label.to_lowercase(),
                    err
                )
            })?;
    }

    println!(
        "[Audio] {} capture {} instance {}. active_instances={:?}",
        capture_label,
        if paused { "paused" } else { "resumed" },
        instance_id,
        active_instances
    );

    Ok(())
}

#[tauri::command]
pub fn start_system_audio_capture<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
    state: tauri::State<'_, AudioState>,
    _sherpa_state: tauri::State<'_, crate::sherpa::SherpaState>,
    device_name: Option<String>,
    instance_id: String,
) -> Result<(), String> {
    let _start_guard = state.system_start_guard.lock().map_err(|e| e.to_string())?;
    let requested_device = requested_device_label(&device_name);

    {
        let mut capture = state.system_capture.lock().map_err(|e| e.to_string())?;
        if capture.is_running() {
            let owners = capture.attach_instance(instance_id.clone());
            let active_device = capture.active_device_label().to_string();
            let recorder_tx = capture.recorder_tx.clone();
            println!(
                "[Audio] System capture already running. Attached instance: {}. requested_device={}, active_device={}, owners={:?}",
                instance_id, requested_device, active_device, owners
            );
            drop(capture);
            queue_recording_start(
                &app,
                recorder_tx.as_ref(),
                should_record_system(&instance_id),
                "System",
                &instance_id,
            )?;
            return Ok(());
        }
    }

    println!(
        "[Audio] Starting system audio capture. instance={}, requested_device={}",
        instance_id, requested_device
    );

    let (stop_tx, rx) = channel::<()>();
    let task_rb_capacity = 16000 * 5;
    let task_rb = HeapRb::<f32>::new(task_rb_capacity);
    let (mut task_producer, mut task_consumer) = task_rb.split();
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<()>(100);
    let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(10);
    let (startup_tx, startup_rx) = channel::<Result<String, String>>();

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
        let mut current_filepath = String::new();
        let mut pull_buffer = vec![0.0; 16000];
        let mut recorder_paused = false;

        loop {
            tokio::select! {
                biased;
                cmd = recorder_rx.recv() => {
                    match cmd {
                        Some(RecorderCommand::Start(path)) => {
                            if let Some(w) = writer.take() {
                                let _ = w.finalize();
                            }
                            let spec = hound::WavSpec {
                                channels: 1,
                                sample_rate: 16000,
                                bits_per_sample: 16,
                                sample_format: hound::SampleFormat::Int,
                            };
                            match hound::WavWriter::create(&path, spec) {
                                Ok(w) => {
                                    writer = Some(w);
                                    current_filepath = path;
                                    recorder_paused = false;
                                }
                                Err(e) => {
                                    eprintln!("[Audio] Failed to create system WAV writer: {}", e)
                                }
                            }
                        }
                        Some(RecorderCommand::Stop(tx)) => {
                            recorder_paused = false;
                            if let Some(w) = writer.take() {
                                let _ = w.finalize();
                            }
                            let _ = tx.send(current_filepath.clone());
                            current_filepath.clear();
                        }
                        Some(RecorderCommand::SetPaused(paused)) => {
                            recorder_paused = paused;
                        }
                        None => break,
                    }
                }
                opt = data_rx.recv() => {
                    match opt {
                        Some(()) => {
                            let len = task_consumer.pop_slice(&mut pull_buffer);
                            if len > 0 {
                                let chunk = &pull_buffer[..len];

                                if !recorder_paused {
                                    if let Some(w) = writer.as_mut() {
                                        let amplitude = i16::MAX as f32;
                                        for &sample in chunk {
                                            let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                                        }
                                    }
                                }

                                feed_system_audio_to_instances(&app_clone, chunk).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }

        loop {
            let len = task_consumer.pop_slice(&mut pull_buffer);
            if len == 0 {
                break;
            }
            let chunk = &pull_buffer[..len];

            if !recorder_paused {
                if let Some(w) = writer.as_mut() {
                    let amplitude = i16::MAX as f32;
                    for &sample in chunk {
                        let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                    }
                }
            }

            feed_system_audio_to_instances(&app_clone, chunk).await;
        }

        if let Some(w) = writer {
            let _ = w.finalize();
        }
    });

    let startup_instance_id = instance_id.clone();
    let startup_requested_device = requested_device.clone();
    thread::spawn(move || {
        let fail_start = |message: String| {
            eprintln!(
                "[Audio] Failed to start system capture for instance {} (requested_device={}): {}",
                startup_instance_id, startup_requested_device, message
            );
            let _ = startup_tx.send(Err(message));
        };

        let err_fn = |err| eprintln!("[Audio] Stream error: {}", err);
        let host = cpal::default_host();
        let device = device_name
            .as_ref()
            .and_then(|name| {
                host.output_devices().ok().and_then(|mut devices| {
                    devices.find(|d| d.name().map(|n| n == *name).unwrap_or(false))
                })
            })
            .or_else(|| host.default_output_device());

        let Some(device) = device else {
            fail_start("No output device found".to_string());
            return;
        };
        let resolved_device_name = device.name().unwrap_or_else(|_| "unknown".to_string());

        let supported_config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                fail_start(format!("Failed to get default config: {}", e));
                return;
            }
        };

        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        let target_sample_rate = 16000;
        let chunk_size_out = 1024;

        let mut resampler = match FftFixedOut::<f32>::new(
            sample_rate as usize,
            target_sample_rate,
            chunk_size_out,
            2,
            1,
        ) {
            Ok(r) => r,
            Err(e) => {
                fail_start(format!("Failed to create resampler: {}", e));
                return;
            }
        };

        let input_frames_next = resampler.input_frames_next();
        let resampler_input_buffer_size = input_frames_next;
        let rb_capacity = resampler_input_buffer_size * 4;
        let rb = HeapRb::<f32>::new(rb_capacity);
        let (mut producer, mut consumer) = rb.split();
        let mut input_buffer: Vec<Vec<f32>> = vec![vec![0.0; resampler_input_buffer_size]; 1];
        let mut output_buffer: Vec<Vec<f32>> = vec![vec![0.0; chunk_size_out]; 1];
        let window_clone = window.clone();

        let stream_result = match sample_format {
            SampleFormat::F32 => device.build_input_stream(
                &config,
                move |data: &[f32], _: &_| {
                    process_audio(
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
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |data: &[i16], _: &_| {
                    let data_f32: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                    process_audio(
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
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config,
                move |data: &[u16], _: &_| {
                    let data_f32: Vec<f32> = data
                        .iter()
                        .map(|&s| (s as f32 - 32768.0) / 32768.0)
                        .collect();
                    process_audio(
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
            ),
            _ => {
                fail_start("Unsupported sample format".to_string());
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                fail_start(format!("Failed to build input stream: {}", e));
                return;
            }
        };

        if let Err(e) = stream.play() {
            fail_start(format!("Failed to play stream: {}", e));
            return;
        }

        println!(
            "[Audio] Capture started successfully on background thread. instance={}, active_device={}",
            startup_instance_id, resolved_device_name
        );
        if startup_tx.send(Ok(resolved_device_name.clone())).is_err() {
            return;
        }

        let _ = rx.recv();
        println!(
            "[Audio] Stop signal received. Dropping stream. instance={}, active_device={}",
            startup_instance_id, resolved_device_name
        );
    });

    let active_device = match startup_rx.recv() {
        Ok(Ok(device_name)) => device_name,
        Ok(Err(err)) => return Err(err),
        Err(err) => {
            return Err(format!(
                "System capture startup channel closed before completion: {}",
                err
            ))
        }
    };

    {
        let mut capture = state.system_capture.lock().map_err(|e| e.to_string())?;
        let owners = capture.commit_start(
            instance_id.clone(),
            active_device.clone(),
            stop_tx,
            recorder_tx.clone(),
        );
        println!(
            "[Audio] System capture startup committed. instance={}, active_device={}, owners={:?}",
            instance_id, active_device, owners
        );
    }

    queue_recording_start(
        &app,
        Some(&recorder_tx),
        should_record_system(&instance_id),
        "System",
        &instance_id,
    )?;

    Ok(())
}

async fn feed_system_audio_to_instances<R: Runtime>(app: &AppHandle<R>, chunk: &[f32]) {
    let instance_ids: Vec<String> = {
        let audio_state = app.state::<AudioState>();
        let guard = match audio_state.system_capture.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.active_instances()
    };

    if instance_ids.is_empty() {
        return;
    }

    let sherpa_state = app.state::<crate::sherpa::SherpaState>();
    for instance_id in instance_ids {
        if instance_id.starts_with("test_") {
            continue;
        }
        if let Err(e) =
            crate::sherpa::feed_audio_samples(app, &sherpa_state, &instance_id, chunk).await
        {
            eprintln!(
                "[Audio] Failed to feed system audio to Sherpa instance {}: {}",
                instance_id, e
            );
        }
    }
}

#[tauri::command]
pub fn get_microphone_devices() -> Result<Vec<AudioDevice>, String> {
    let host = cpal::default_host();
    let devices = host.input_devices().map_err(|e| e.to_string())?;

    let result = devices
        .filter_map(|device| device.name().ok())
        .map(|name| AudioDevice { name })
        .collect();

    Ok(result)
}

#[tauri::command]
pub fn start_microphone_capture<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
    state: tauri::State<'_, AudioState>,
    _sherpa_state: tauri::State<'_, crate::sherpa::SherpaState>,
    device_name: Option<String>,
    instance_id: String,
) -> Result<(), String> {
    let _start_guard = state.mic_start_guard.lock().map_err(|e| e.to_string())?;
    let requested_device = requested_device_label(&device_name);

    {
        let mut capture = state.mic_capture.lock().map_err(|e| e.to_string())?;
        if capture.is_running() {
            let owners = capture.attach_instance(instance_id.clone());
            let active_device = capture.active_device_label().to_string();
            let recorder_tx = capture.recorder_tx.clone();
            println!(
                "[Audio] Microphone capture already running. Attached instance: {}. requested_device={}, active_device={}, owners={:?}",
                instance_id, requested_device, active_device, owners
            );
            drop(capture);
            queue_recording_start(
                &app,
                recorder_tx.as_ref(),
                should_record_microphone(&instance_id),
                "Microphone",
                &instance_id,
            )?;
            return Ok(());
        }
    }

    println!(
        "[Audio] Starting microphone capture. instance={}, requested_device={}",
        instance_id, requested_device
    );

    let (stop_tx, rx) = channel::<()>();
    let task_rb_capacity = 16000 * 5;
    let task_rb = HeapRb::<f32>::new(task_rb_capacity);
    let (mut task_producer, mut task_consumer) = task_rb.split();
    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<()>(100);
    let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(10);
    let (startup_tx, startup_rx) = channel::<Result<String, String>>();

    let app_clone = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
        let mut current_filepath = String::new();
        let mut pull_buffer = vec![0.0; 16000];
        let mut recorder_paused = false;

        loop {
            tokio::select! {
                biased;
                cmd = recorder_rx.recv() => {
                    match cmd {
                        Some(RecorderCommand::Start(path)) => {
                            if let Some(w) = writer.take() {
                                let _ = w.finalize();
                            }
                            let spec = hound::WavSpec {
                                channels: 1,
                                sample_rate: 16000,
                                bits_per_sample: 16,
                                sample_format: hound::SampleFormat::Int,
                            };
                            match hound::WavWriter::create(&path, spec) {
                                Ok(w) => {
                                    writer = Some(w);
                                    current_filepath = path;
                                    recorder_paused = false;
                                }
                                Err(e) => eprintln!("[Audio] Failed to create mic WAV writer: {}", e),
                            }
                        }
                        Some(RecorderCommand::Stop(tx)) => {
                            recorder_paused = false;
                            if let Some(w) = writer.take() {
                                let _ = w.finalize();
                            }
                            let _ = tx.send(current_filepath.clone());
                            current_filepath.clear();
                        }
                        Some(RecorderCommand::SetPaused(paused)) => {
                            recorder_paused = paused;
                        }
                        None => break,
                    }
                }
                opt = data_rx.recv() => {
                    match opt {
                        Some(()) => {
                            let len = task_consumer.pop_slice(&mut pull_buffer);
                            if len > 0 {
                                let chunk = &pull_buffer[..len];

                                if !recorder_paused {
                                    if let Some(w) = writer.as_mut() {
                                        let amplitude = i16::MAX as f32;
                                        for &sample in chunk {
                                            let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                                        }
                                    }
                                }

                                feed_mic_audio_to_instances(&app_clone, chunk).await;
                            }
                        }
                        None => break,
                    }
                }
            }
        }

        loop {
            let len = task_consumer.pop_slice(&mut pull_buffer);
            if len == 0 {
                break;
            }
            let chunk = &pull_buffer[..len];

            if !recorder_paused {
                if let Some(w) = writer.as_mut() {
                    let amplitude = i16::MAX as f32;
                    for &sample in chunk {
                        let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                    }
                }
            }

            feed_mic_audio_to_instances(&app_clone, chunk).await;
        }

        if let Some(w) = writer {
            let _ = w.finalize();
        }
    });

    let startup_instance_id = instance_id.clone();
    let startup_requested_device = requested_device.clone();
    thread::spawn(move || {
        let fail_start = |message: String| {
            eprintln!(
                "[Audio] Failed to start microphone capture for instance {} (requested_device={}): {}",
                startup_instance_id, startup_requested_device, message
            );
            let _ = startup_tx.send(Err(message));
        };

        let err_fn = |err| eprintln!("[Audio] Mic stream error: {}", err);
        let host = cpal::default_host();
        let device = device_name
            .as_ref()
            .and_then(|name| {
                host.input_devices().ok().and_then(|mut devices| {
                    devices.find(|d| d.name().map(|n| n == *name).unwrap_or(false))
                })
            })
            .or_else(|| host.default_input_device());

        let Some(device) = device else {
            fail_start("No input device found".to_string());
            return;
        };
        let resolved_device_name = device.name().unwrap_or_else(|_| "unknown".to_string());

        let supported_config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                fail_start(format!("Failed to get default mic config: {}", e));
                return;
            }
        };

        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();
        let sample_rate = config.sample_rate.0;
        let channels = config.channels;
        let target_sample_rate = 16000;
        let chunk_size_out = 1024;

        let mut resampler = match FftFixedOut::<f32>::new(
            sample_rate as usize,
            target_sample_rate,
            chunk_size_out,
            2,
            1,
        ) {
            Ok(r) => r,
            Err(e) => {
                fail_start(format!("Failed to create mic resampler: {}", e));
                return;
            }
        };

        let input_frames_next = resampler.input_frames_next();
        let resampler_input_buffer_size = input_frames_next;
        let rb_capacity = resampler_input_buffer_size * 4;
        let rb = HeapRb::<f32>::new(rb_capacity);
        let (mut producer, mut consumer) = rb.split();
        let mut input_buffer: Vec<Vec<f32>> = vec![vec![0.0; resampler_input_buffer_size]; 1];
        let mut output_buffer: Vec<Vec<f32>> = vec![vec![0.0; chunk_size_out]; 1];
        let app_handle_f32 = window.app_handle().clone();
        let app_handle_i16 = window.app_handle().clone();
        let app_handle_u16 = window.app_handle().clone();

        let stream_result = match sample_format {
            SampleFormat::F32 => {
                let window_clone = window.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[f32], _: &_| {
                        let boost = *app_handle_f32
                            .state::<AudioState>()
                            .mic_boost
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        process_mic_audio(
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
                            boost,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::I16 => {
                let window_clone = window.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[i16], _: &_| {
                        let boost = *app_handle_i16
                            .state::<AudioState>()
                            .mic_boost
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        let data_f32: Vec<f32> = data.iter().map(|&s| s as f32 / 32768.0).collect();
                        process_mic_audio(
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
                            boost,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            SampleFormat::U16 => {
                let window_clone = window.clone();
                device.build_input_stream(
                    &config,
                    move |data: &[u16], _: &_| {
                        let boost = *app_handle_u16
                            .state::<AudioState>()
                            .mic_boost
                            .lock()
                            .unwrap_or_else(|e| e.into_inner());
                        let data_f32: Vec<f32> = data
                            .iter()
                            .map(|&s| (s as f32 - 32768.0) / 32768.0)
                            .collect();
                        process_mic_audio(
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
                            boost,
                        );
                    },
                    err_fn,
                    None,
                )
            }
            _ => {
                fail_start("Unsupported mic sample format".to_string());
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                fail_start(format!("Failed to build mic input stream: {}", e));
                return;
            }
        };

        if let Err(e) = stream.play() {
            fail_start(format!("Failed to play mic stream: {}", e));
            return;
        }

        println!(
            "[Audio] Mic capture started successfully on background thread. instance={}, active_device={}",
            startup_instance_id, resolved_device_name
        );
        if startup_tx.send(Ok(resolved_device_name.clone())).is_err() {
            return;
        }

        let _ = rx.recv();
        println!(
            "[Audio] Mic stop signal received. Dropping stream. instance={}, active_device={}",
            startup_instance_id, resolved_device_name
        );
    });

    let active_device = match startup_rx.recv() {
        Ok(Ok(device_name)) => device_name,
        Ok(Err(err)) => return Err(err),
        Err(err) => {
            return Err(format!(
                "Microphone capture startup channel closed before completion: {}",
                err
            ))
        }
    };

    {
        let mut capture = state.mic_capture.lock().map_err(|e| e.to_string())?;
        let owners = capture.commit_start(
            instance_id.clone(),
            active_device.clone(),
            stop_tx,
            recorder_tx.clone(),
        );
        println!(
            "[Audio] Microphone capture startup committed. instance={}, active_device={}, owners={:?}",
            instance_id, active_device, owners
        );
    }

    queue_recording_start(
        &app,
        Some(&recorder_tx),
        should_record_microphone(&instance_id),
        "Microphone",
        &instance_id,
    )?;

    Ok(())
}

async fn feed_mic_audio_to_instances<R: Runtime>(app: &AppHandle<R>, chunk: &[f32]) {
    let instance_ids: Vec<String> = {
        let audio_state = app.state::<AudioState>();
        let guard = match audio_state.mic_capture.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.active_instances()
    };

    if instance_ids.is_empty() {
        return;
    }

    let sherpa_state = app.state::<crate::sherpa::SherpaState>();
    for instance_id in instance_ids {
        if instance_id.starts_with("test_") {
            continue;
        }
        if let Err(e) =
            crate::sherpa::feed_audio_samples(app, &sherpa_state, &instance_id, chunk).await
        {
            eprintln!(
                "[Audio] Failed to feed mic audio to Sherpa instance {}: {}",
                instance_id, e
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn process_mic_audio<R: Runtime>(
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
    boost: f32,
) {
    for frame in data.chunks(channels) {
        let mut sum = 0.0;
        for sample in frame {
            sum += sample;
        }
        let mut mono_sample = sum / channels as f32;

        if (boost - 1.0).abs() > f32::EPSILON {
            mono_sample = (mono_sample * boost).clamp(-1.0, 1.0);
        }

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
                    let _ = window.app_handle().emit("microphone-audio", peak_i16);
                }
            }
            Err(e) => {
                eprintln!("[Audio] Mic resampler error: {}", e);
            }
        }
    }
}

#[tauri::command]
pub async fn stop_microphone_capture(
    state: tauri::State<'_, AudioState>,
    instance_id: String,
) -> Result<String, String> {
    let was_recording = should_record_microphone(&instance_id);
    let detach_result = {
        let mut capture = state.mic_capture.lock().map_err(|e| e.to_string())?;
        let detach_result = capture.detach_instance(&instance_id);
        let active_device = detach_result
            .active_device_name
            .as_deref()
            .unwrap_or("unknown");
        if detach_result.should_stop_hardware {
            println!(
                "[Audio] Mic capture detaching final instance {}. active_device={}",
                instance_id, active_device
            );
        } else {
            println!(
                "[Audio] Mic capture remains active after detaching {}. active_device={}, owners={:?}",
                instance_id, active_device, detach_result.remaining_instances
            );
        }
        detach_result
    };

    let mut saved_path = String::new();
    if was_recording {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = detach_result
            .recorder_tx
            .as_ref()
            .map(|recorder_tx| recorder_tx.try_send(RecorderCommand::Stop(tx)).is_ok())
            .unwrap_or(false);

        if sent {
            match rx.await {
                Ok(path) => saved_path = path,
                Err(_) => eprintln!("[Audio] Failed to receive mic WAV filepath from task"),
            }
        } else {
            eprintln!(
                "[Audio] Mic recorder stop was requested for {}, but no recorder task was available",
                instance_id
            );
        }
    }

    if !detach_result.should_stop_hardware {
        return Ok(saved_path);
    }

    if let Some(tx) = detach_result.stop_signal {
        println!("[Audio] Stopping microphone capture...");
        let _ = tx.send(());
    } else {
        println!("[Audio] Mic stop requested but not running");
    }

    Ok(saved_path)
}

#[allow(clippy::too_many_arguments)]
fn process_audio<R: Runtime>(
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
                    let _ = window.app_handle().emit("system-audio", peak_i16);
                }
            }
            Err(e) => {
                eprintln!("[Audio] System resampler error: {}", e);
            }
        }
    }
}

#[tauri::command]
pub async fn stop_system_audio_capture(
    state: tauri::State<'_, AudioState>,
    instance_id: String,
) -> Result<String, String> {
    let was_recording = should_record_system(&instance_id);
    let detach_result = {
        let mut capture = state.system_capture.lock().map_err(|e| e.to_string())?;
        let detach_result = capture.detach_instance(&instance_id);
        let active_device = detach_result
            .active_device_name
            .as_deref()
            .unwrap_or("unknown");
        if detach_result.should_stop_hardware {
            println!(
                "[Audio] System capture detaching final instance {}. active_device={}",
                instance_id, active_device
            );
        } else {
            println!(
                "[Audio] System capture remains active after detaching {}. active_device={}, owners={:?}",
                instance_id, active_device, detach_result.remaining_instances
            );
        }
        detach_result
    };

    let mut saved_path = String::new();
    if was_recording {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = detach_result
            .recorder_tx
            .as_ref()
            .map(|recorder_tx| recorder_tx.try_send(RecorderCommand::Stop(tx)).is_ok())
            .unwrap_or(false);

        if sent {
            match rx.await {
                Ok(path) => saved_path = path,
                Err(_) => eprintln!("[Audio] Failed to receive system WAV filepath from task"),
            }
        } else {
            eprintln!(
                "[Audio] System recorder stop was requested for {}, but no recorder task was available",
                instance_id
            );
        }
    }

    if !detach_result.should_stop_hardware {
        return Ok(saved_path);
    }

    if let Some(tx) = detach_result.stop_signal {
        println!("[Audio] Stopping system capture...");
        let _ = tx.send(());
    } else {
        println!("[Audio] Stop requested but not running");
    }

    Ok(saved_path)
}

#[tauri::command]
pub fn set_system_audio_capture_paused(
    state: tauri::State<'_, AudioState>,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    let mut capture = state.system_capture.lock().map_err(|e| e.to_string())?;
    update_capture_pause_state(
        &mut capture,
        &instance_id,
        paused,
        "System",
        should_record_system(&instance_id),
    )
}

#[tauri::command]
pub fn set_microphone_capture_paused(
    state: tauri::State<'_, AudioState>,
    instance_id: String,
    paused: bool,
) -> Result<(), String> {
    let mut capture = state.mic_capture.lock().map_err(|e| e.to_string())?;
    update_capture_pause_state(
        &mut capture,
        &instance_id,
        paused,
        "Microphone",
        should_record_microphone(&instance_id),
    )
}

#[tauri::command]
pub fn set_microphone_boost(state: tauri::State<'_, AudioState>, boost: f32) -> Result<(), String> {
    let mut mic_boost = state.mic_boost.lock().map_err(|e| e.to_string())?;
    *mic_boost = boost;
    println!("[Audio] Set microphone boost to {}", boost);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_capture_state_only_becomes_running_after_commit() {
        let mut capture = SharedCaptureState::default();
        assert!(!capture.is_running());
        assert!(capture.owners().is_empty());
        assert!(capture.active_instances().is_empty());
        assert!(capture.recorder_tx.is_none());

        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        let owners = capture.commit_start(
            "record".to_string(),
            "default mic".to_string(),
            stop_tx,
            recorder_tx,
        );

        assert!(capture.is_running());
        assert_eq!(owners, vec!["record".to_string()]);
        assert_eq!(capture.active_instances(), vec!["record".to_string()]);
        assert_eq!(capture.active_device_name.as_deref(), Some("default mic"));
        assert!(capture.recorder_tx.is_some());
    }

    #[test]
    fn shared_capture_state_failed_start_leaves_no_runtime_state() {
        let capture = SharedCaptureState::default();

        assert!(!capture.is_running());
        assert!(capture.owners().is_empty());
        assert!(capture.active_instances().is_empty());
        assert!(capture.recorder_tx.is_none());
        assert!(capture.active_device_name.is_none());
    }

    #[test]
    fn shared_capture_state_attach_adds_owner_to_running_capture() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start(
            "voice-typing".to_string(),
            "default mic".to_string(),
            stop_tx,
            recorder_tx,
        );

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
        capture.commit_start(
            "record".to_string(),
            "default mic".to_string(),
            stop_tx,
            recorder_tx,
        );

        let detach_result = capture.detach_instance("record");

        assert!(detach_result.should_stop_hardware);
        assert!(detach_result.remaining_instances.is_empty());
        assert!(detach_result.stop_signal.is_some());
        assert!(detach_result.recorder_tx.is_some());
        assert_eq!(
            detach_result.active_device_name.as_deref(),
            Some("default mic")
        );
        assert!(!capture.is_running());
        assert!(capture.owners().is_empty());
        assert!(capture.active_instances().is_empty());
        assert!(capture.recorder_tx.is_none());
        assert!(capture.active_device_name.is_none());
    }

    #[test]
    fn shared_capture_state_pause_filters_active_instances_without_detaching_owner() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start(
            "voice-typing".to_string(),
            "default mic".to_string(),
            stop_tx,
            recorder_tx,
        );
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
    fn shared_capture_state_detach_clears_paused_instance_state() {
        let mut capture = SharedCaptureState::default();
        let (stop_tx, _stop_rx) = channel::<()>();
        let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
        capture.commit_start(
            "record".to_string(),
            "default mic".to_string(),
            stop_tx,
            recorder_tx,
        );
        capture.set_instance_paused("record", true).unwrap();

        let detach_result = capture.detach_instance("record");

        assert!(detach_result.should_stop_hardware);
        assert!(capture.paused_instances.is_empty());
        assert!(capture.active_instances().is_empty());
    }
}
