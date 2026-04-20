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
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use uuid::Uuid;

pub enum RecorderCommand {
    Start(String), // filepath
    Stop(tokio::sync::oneshot::Sender<String>),
}

pub struct AudioState {
    system_stop_signal: Mutex<Option<Sender<()>>>,
    mic_stop_signal: Mutex<Option<Sender<()>>>,
    system_instance_ids: Mutex<HashSet<String>>,
    mic_instance_ids: Mutex<HashSet<String>>,
    system_recorder_tx: Mutex<Option<tokio::sync::mpsc::Sender<RecorderCommand>>>,
    mic_recorder_tx: Mutex<Option<tokio::sync::mpsc::Sender<RecorderCommand>>>,
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
            system_stop_signal: Mutex::new(None),
            mic_stop_signal: Mutex::new(None),
            system_instance_ids: Mutex::new(HashSet::new()),
            mic_instance_ids: Mutex::new(HashSet::new()),
            system_recorder_tx: Mutex::new(None),
            mic_recorder_tx: Mutex::new(None),
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

#[tauri::command]
pub fn start_system_audio_capture<R: Runtime>(
    app: AppHandle<R>,
    window: Window<R>,
    state: tauri::State<'_, AudioState>,
    _sherpa_state: tauri::State<'_, crate::sherpa::SherpaState>,
    device_name: Option<String>,
    instance_id: String,
) -> Result<(), String> {
    let mut system_instance_ids = state.system_instance_ids.lock().map_err(|e| e.to_string())?;
    let is_running = {
        let guard = state.system_stop_signal.lock().map_err(|e| e.to_string())?;
        guard.is_some()
    };

    if is_running {
        system_instance_ids.insert(instance_id.clone());
        println!(
            "[Audio] System capture already running. Attached instance: {}",
            instance_id
        );

        if instance_id == "record" {
            if let Some(tx) = state.system_recorder_tx.lock().map_err(|e| e.to_string())?.as_ref() {
                let app_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
                let history_dir = app_data_dir.join("history");
                if !history_dir.exists() {
                    std::fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
                }
                let wav_filename = format!("{}.wav", uuid::Uuid::new_v4());
                let wav_filepath = history_dir.join(&wav_filename);
                let wav_filepath_str = wav_filepath.to_string_lossy().into_owned();
                let _ = tx.try_send(RecorderCommand::Start(wav_filepath_str));
            }
        }
        return Ok(());
    }

    system_instance_ids.clear();
    system_instance_ids.insert(instance_id.clone());

    println!("[Audio] Starting system audio capture...");

    let (stop_tx, rx) = channel::<()>();
    *state.system_stop_signal.lock().map_err(|e| e.to_string())? = Some(stop_tx);

    let task_rb_capacity = 16000 * 5;
    let task_rb = HeapRb::<f32>::new(task_rb_capacity);
    let (mut task_producer, mut task_consumer) = task_rb.split();

    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<()>(100);

    let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(10);
    *state.system_recorder_tx.lock().map_err(|e| e.to_string())? = Some(recorder_tx.clone());

    let is_test = instance_id.starts_with("test_");

    if !is_test && instance_id == "record" {
        let app_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
        let history_dir = app_data_dir.join("history");
        if !history_dir.exists() {
            std::fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
        }
        let wav_filename = format!("{}.wav", uuid::Uuid::new_v4());
        let wav_filepath = history_dir.join(&wav_filename);
        let wav_filepath_str = wav_filepath.to_string_lossy().into_owned();
        let _ = recorder_tx.try_send(RecorderCommand::Start(wav_filepath_str));
    }

    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
        let mut current_filepath = String::new();

        let mut pull_buffer = vec![0.0; 16000];

        loop {
            tokio::select! {
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
                                },
                                Err(e) => eprintln!("[Audio] Failed to create system WAV writer: {}", e),
                            }
                        },
                        Some(RecorderCommand::Stop(tx)) => {
                            if let Some(w) = writer.take() {
                                let _ = w.finalize();
                            }
                            let _ = tx.send(current_filepath.clone());
                            current_filepath.clear();
                        },
                        None => break,
                    }
                },
                _ = data_rx.recv() => {
                    let len = task_consumer.pop_slice(&mut pull_buffer);
                    if len > 0 {
                        let chunk = &pull_buffer[..len];

                        if let Some(w) = writer.as_mut() {
                            let amplitude = i16::MAX as f32;
                            for &sample in chunk {
                                let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                            }
                        }

                        feed_system_audio_to_instances(&app_clone, chunk).await;
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

            if let Some(w) = writer.as_mut() {
                let amplitude = i16::MAX as f32;
                for &sample in chunk {
                    let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                }
            }

            feed_system_audio_to_instances(&app_clone, chunk).await;
        }

        if let Some(w) = writer {
            let _ = w.finalize();
        }
    });

    thread::spawn(move || {
        let err_fn = |err| eprintln!("[Audio] Stream error: {}", err);

        let host = cpal::default_host();
        let device = device_name
            .as_ref()
            .and_then(|name| {
                host.output_devices()
                    .ok()
                    .and_then(|mut devices| {
                        devices.find(|d| d.name().map(|n| n == *name).unwrap_or(false))
                    })
            })
            .or_else(|| host.default_output_device());

        let Some(device) = device else {
            eprintln!("[Audio] No output device found");
            return;
        };

        let supported_config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Audio] Failed to get default config: {}", e);
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
                eprintln!("[Audio] Failed to create resampler: {}", e);
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
                eprintln!("[Audio] Unsupported sample format");
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[Audio] Failed to build input stream: {}", e);
                return;
            }
        };

        if let Err(e) = stream.play() {
            eprintln!("[Audio] Failed to play stream: {}", e);
            return;
        }

        println!("[Audio] Capture started successfully on background thread.");

        let _ = rx.recv();

        println!("[Audio] Stop signal received. Dropping stream.");
    });

    Ok(())
}

async fn feed_system_audio_to_instances<R: Runtime>(app: &AppHandle<R>, chunk: &[f32]) {
    let instance_ids: Vec<String> = {
        let audio_state = app.state::<AudioState>();
        let guard = match audio_state.system_instance_ids.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.iter().cloned().collect()
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
    let mut mic_instance_ids = state.mic_instance_ids.lock().map_err(|e| e.to_string())?;
    let is_running = {
        let guard = state.mic_stop_signal.lock().map_err(|e| e.to_string())?;
        guard.is_some()
    };

    if is_running {
        mic_instance_ids.insert(instance_id.clone());
        println!(
            "[Audio] Microphone capture already running. Attached instance: {}",
            instance_id
        );

        if instance_id != "voice-typing" && !instance_id.starts_with("test_") {
            if let Some(tx) = state.mic_recorder_tx.lock().map_err(|e| e.to_string())?.as_ref() {
                let app_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
                let history_dir = app_data_dir.join("history");
                if !history_dir.exists() {
                    std::fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
                }
                let wav_filename = format!("{}.wav", uuid::Uuid::new_v4());
                let wav_filepath = history_dir.join(&wav_filename);
                let wav_filepath_str = wav_filepath.to_string_lossy().into_owned();
                let _ = tx.try_send(RecorderCommand::Start(wav_filepath_str));
            }
        }
        return Ok(());
    }

    mic_instance_ids.clear();
    mic_instance_ids.insert(instance_id.clone());

    println!("[Audio] Starting microphone capture...");

    let (stop_tx, rx) = channel::<()>();
    *state.mic_stop_signal.lock().map_err(|e| e.to_string())? = Some(stop_tx);

    let task_rb_capacity = 16000 * 5;
    let task_rb = HeapRb::<f32>::new(task_rb_capacity);
    let (mut task_producer, mut task_consumer) = task_rb.split();

    let (data_tx, mut data_rx) = tokio::sync::mpsc::channel::<()>(100);

    let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(10);
    *state.mic_recorder_tx.lock().map_err(|e| e.to_string())? = Some(recorder_tx.clone());

    let is_test = instance_id.starts_with("test_");

    if !is_test && instance_id != "voice-typing" {
        let app_data_dir = app.path().app_local_data_dir().map_err(|e| e.to_string())?;
        let history_dir = app_data_dir.join("history");
        if !history_dir.exists() {
            std::fs::create_dir_all(&history_dir).map_err(|e| e.to_string())?;
        }
        let wav_filename = format!("{}.wav", uuid::Uuid::new_v4());
        let wav_filepath = history_dir.join(&wav_filename);
        let wav_filepath_str = wav_filepath.to_string_lossy().into_owned();
        let _ = recorder_tx.try_send(RecorderCommand::Start(wav_filepath_str));
    }

    let app_clone = app.clone();

    tauri::async_runtime::spawn(async move {
        let mut writer: Option<hound::WavWriter<std::io::BufWriter<std::fs::File>>> = None;
        let mut current_filepath = String::new();

        let mut pull_buffer = vec![0.0; 16000];

        loop {
            tokio::select! {
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
                                },
                                Err(e) => eprintln!("[Audio] Failed to create mic WAV writer: {}", e),
                            }
                        },
                        Some(RecorderCommand::Stop(tx)) => {
                            if let Some(w) = writer.take() {
                                let _ = w.finalize();
                            }
                            let _ = tx.send(current_filepath.clone());
                            current_filepath.clear();
                        },
                        None => break,
                    }
                },
                opt = data_rx.recv() => {
                    match opt {
                        Some(()) => {
                            let len = task_consumer.pop_slice(&mut pull_buffer);
                            if len > 0 {
                                let chunk = &pull_buffer[..len];

                                if let Some(w) = writer.as_mut() {
                                    let amplitude = i16::MAX as f32;
                                    for &sample in chunk {
                                        let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                                    }
                                }

                                feed_mic_audio_to_instances(&app_clone, chunk).await;
                            }
                        },
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

            if let Some(w) = writer.as_mut() {
                let amplitude = i16::MAX as f32;
                for &sample in chunk {
                    let _ = w.write_sample((sample.clamp(-1.0, 1.0) * amplitude) as i16);
                }
            }

            feed_mic_audio_to_instances(&app_clone, chunk).await;
        }

        if let Some(w) = writer {
            let _ = w.finalize();
        }
    });

    thread::spawn(move || {
        let err_fn = |err| eprintln!("[Audio] Mic stream error: {}", err);

        let host = cpal::default_host();
        let device = device_name
            .as_ref()
            .and_then(|name| {
                host.input_devices()
                    .ok()
                    .and_then(|mut devices| {
                        devices.find(|d| d.name().map(|n| n == *name).unwrap_or(false))
                    })
            })
            .or_else(|| host.default_input_device());

        let Some(device) = device else {
            eprintln!("[Audio] No input device found");
            return;
        };

        let supported_config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Audio] Failed to get default mic config: {}", e);
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
                eprintln!("[Audio] Failed to create mic resampler: {}", e);
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
                eprintln!("[Audio] Unsupported mic sample format");
                return;
            }
        };

        let stream = match stream_result {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[Audio] Failed to build mic input stream: {}", e);
                return;
            }
        };

        if let Err(e) = stream.play() {
            eprintln!("[Audio] Failed to play mic stream: {}", e);
            return;
        }

        println!("[Audio] Mic capture started successfully on background thread.");

        let _ = rx.recv();

        println!("[Audio] Mic stop signal received. Dropping stream.");
    });

    Ok(())
}

async fn feed_mic_audio_to_instances<R: Runtime>(app: &AppHandle<R>, chunk: &[f32]) {
    let instance_ids: Vec<String> = {
        let audio_state = app.state::<AudioState>();
        let guard = match audio_state.mic_instance_ids.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        guard.iter().cloned().collect()
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
    let (should_stop_hardware, was_recording) = {
        let mut instance_ids = state.mic_instance_ids.lock().map_err(|e| e.to_string())?;
        instance_ids.remove(&instance_id);

        let was_recording = instance_id != "voice-typing" && !instance_id.starts_with("test_");

        if instance_ids.is_empty() {
            (true, was_recording)
        } else {
            println!(
                "[Audio] Mic capture remains active for {} instance(s) after detaching {}",
                instance_ids.len(),
                instance_id
            );
            (false, was_recording)
        }
    };

    let mut saved_path = String::new();
    if was_recording {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = {
            if let Some(recorder_tx) = state.mic_recorder_tx.lock().map_err(|e| e.to_string())?.as_ref() {
                recorder_tx.try_send(RecorderCommand::Stop(tx)).is_ok()
            } else {
                false
            }
        };
        
        if sent {
            match rx.await {
                Ok(path) => saved_path = path,
                Err(_) => eprintln!("[Audio] Failed to receive mic WAV filepath from task"),
            }
        }
    }

    if !should_stop_hardware {
        return Ok(saved_path);
    }

    {
        let mut stop_signal_guard = state.mic_stop_signal.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = stop_signal_guard.take() {
            println!("[Audio] Stopping microphone capture...");
            let _ = tx.send(());
        } else {
            println!("[Audio] Mic stop requested but not running");
        }
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
    let (should_stop_hardware, was_recording) = {
        let mut instance_ids = state.system_instance_ids.lock().map_err(|e| e.to_string())?;
        instance_ids.remove(&instance_id);

        let was_recording = instance_id == "record";

        if instance_ids.is_empty() {
            (true, was_recording)
        } else {
            println!(
                "[Audio] System capture remains active for {} instance(s) after detaching {}",
                instance_ids.len(),
                instance_id
            );
            (false, was_recording)
        }
    };

    let mut saved_path = String::new();
    if was_recording {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let sent = {
            if let Some(recorder_tx) = state.system_recorder_tx.lock().map_err(|e| e.to_string())?.as_ref() {
                recorder_tx.try_send(RecorderCommand::Stop(tx)).is_ok()
            } else {
                false
            }
        };
        
        if sent {
            match rx.await {
                Ok(path) => saved_path = path,
                Err(_) => eprintln!("[Audio] Failed to receive system WAV filepath from task"),
            }
        }
    }

    if !should_stop_hardware {
        return Ok(saved_path);
    }

    {
        let mut stop_signal_guard = state.system_stop_signal.lock().map_err(|e| e.to_string())?;
        if let Some(tx) = stop_signal_guard.take() {
            println!("[Audio] Stopping system capture...");
            let _ = tx.send(());
        } else {
            println!("[Audio] Stop requested but not running");
        }
    }

    Ok(saved_path)
}

#[tauri::command]
pub fn set_microphone_boost(state: tauri::State<'_, AudioState>, boost: f32) -> Result<(), String> {
    let mut mic_boost = state.mic_boost.lock().map_err(|e| e.to_string())?;
    *mic_boost = boost;
    println!("[Audio] Set microphone boost to {}", boost);
    Ok(())
}
