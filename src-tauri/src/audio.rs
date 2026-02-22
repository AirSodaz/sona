use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};
use rubato::{FftFixedOut, Resampler};
use std::sync::Mutex;
use std::sync::mpsc::{channel, Sender};
use std::thread;
use tauri::{Emitter, Manager, Runtime, Window};

pub struct AudioState {
    stop_signal: Mutex<Option<Sender<()>>>,
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            stop_signal: Mutex::new(None),
        }
    }
}

#[tauri::command]
pub fn start_system_audio_capture<R: Runtime>(
    window: Window<R>,
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    let mut stop_signal_guard = state.stop_signal.lock().unwrap();
    if stop_signal_guard.is_some() {
        println!("[Audio] Capture already running.");
        return Ok(());
    }

    println!("[Audio] Starting system audio capture...");

    // Create channel for stop signal
    let (tx, rx) = channel::<()>();
    *stop_signal_guard = Some(tx);

    // Spawn thread to handle audio stream
    thread::spawn(move || {
        let err_fn = |err| eprintln!("[Audio] Stream error: {}", err);

        let host = cpal::default_host();
        let device = match host.default_output_device() {
            Some(d) => d,
            None => {
                eprintln!("[Audio] No default output device found");
                return;
            }
        };

        println!("[Audio] Device: {}", device.name().unwrap_or("Unknown".into()));

        let supported_config = match device.default_output_config() {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[Audio] Failed to get default config: {}", e);
                return;
            }
        };

        let sample_format = supported_config.sample_format();
        let config: cpal::StreamConfig = supported_config.into();

        println!("[Audio] Config: {:?}", config);

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
                        input_frames_next
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
                        input_frames_next
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
                        input_frames_next
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

        // Wait for stop signal
        let _ = rx.recv();

        println!("[Audio] Stop signal received. Dropping stream.");
        // stream is dropped here, stopping capture
    });

    Ok(())
}

fn process_audio<R: Runtime>(
    data: &[f32],
    channels: usize,
    producer: &mut impl Producer<Item = f32>,
    consumer: &mut impl Consumer<Item = f32>,
    resampler: &mut FftFixedOut<f32>,
    input_buffer: &mut [Vec<f32>],
    output_buffer: &mut [Vec<f32>],
    window: &Window<R>,
    input_frames_needed: usize
) {
    // 1. Mix to Mono and Push to RingBuffer
    for frame in data.chunks(channels) {
        let mut sum = 0.0;
        for sample in frame {
            sum += sample;
        }
        let mono_sample = sum / channels as f32;
        let _ = producer.try_push(mono_sample);
    }

    // 2. Process chunks if enough data
    while consumer.occupied_len() >= input_frames_needed {
        let chunk_slice = &mut input_buffer[0];
        let _read = consumer.pop_slice(chunk_slice);

        // input_buffer is &mut [Vec<f32>], output_buffer is &mut [Vec<f32>]
        // process_into takes &[Vec], &mut [Vec].
        // &mut [T] coerces to &[T] for first arg.
        let result = resampler.process_into(input_buffer, output_buffer, None);

        if let Ok((_in_len, out_len)) = result {
            if out_len > 0 {
                let output_f32 = &output_buffer[0][..out_len];
                let output_i16: Vec<i16> = output_f32
                    .iter()
                    .map(|&s| {
                        let s = s.clamp(-1.0, 1.0);
                        (s * 32767.0) as i16
                    })
                    .collect();

                let _ = window.app_handle().emit("system-audio", &output_i16);
            }
        }
    }
}

#[tauri::command]
pub fn stop_system_audio_capture(
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    let mut stop_signal_guard = state.stop_signal.lock().unwrap();

    if let Some(tx) = stop_signal_guard.take() {
        println!("[Audio] Stopping capture...");
        let _ = tx.send(());
    } else {
        println!("[Audio] Stop requested but not running");
    }

    Ok(())
}
