use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Producer, Split};
use rubato::{FftFixedOut, Resampler};
use std::sync::Mutex;
use tauri::{Emitter, Runtime, Window};

pub struct AudioState {
    stream: Mutex<Option<Stream>>,
    ref_count: Mutex<u32>,
}

impl AudioState {
    pub fn new() -> Self {
        Self {
            stream: Mutex::new(None),
            ref_count: Mutex::new(0),
        }
    }
}

#[tauri::command]
pub fn start_system_audio_capture<R: Runtime>(
    window: Window<R>,
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    let mut ref_count = state.ref_count.lock().unwrap();
    if *ref_count > 0 {
        *ref_count += 1;
        println!("[Audio] Capture already running. Ref count: {}", *ref_count);
        return Ok(());
    }

    println!("[Audio] Starting system audio capture...");

    // Initialize CPAL
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or("No default output device found")?;

    println!("[Audio] Device: {}", device.name().unwrap_or("Unknown".into()));

    let config = device
        .default_output_config()
        .map_err(|e| format!("Failed to get default config: {}", e))?;

    println!("[Audio] Config: {:?}", config);

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    // Prepare Resampler (Target 16000Hz, Mono)
    let target_sample_rate = 16000;
    // Chunk size for output.
    let chunk_size_out = 1024;
    let mut resampler = FftFixedOut::<f32>::new(
        sample_rate as usize,
        target_sample_rate,
        chunk_size_out,
        2,
        1,
    ).map_err(|e| format!("Failed to create resampler: {}", e))?;

    let input_frames_next = resampler.input_frames_next();
    let resampler_input_buffer_size = input_frames_next;

    // Ring buffer to bridge CPAL (variable size) and Rubato (fixed size)
    // Size it to hold a few chunks
    let rb_capacity = resampler_input_buffer_size * 4;
    let rb = HeapRb::<f32>::new(rb_capacity);
    let (mut producer, mut consumer) = rb.split();

    let err_fn = |err| eprintln!("[Audio] Stream error: {}", err);

    let stream_config: cpal::StreamConfig = config.into();

    // Clone window to move into closure
    let window_clone = window.clone();

    // Buffers for resampler
    let mut input_buffer: Vec<Vec<f32>> = vec![vec![0.0; resampler_input_buffer_size]; 1]; // 1 channel
    let mut output_buffer: Vec<Vec<f32>> = vec![vec![0.0; chunk_size_out]; 1]; // 1 channel

    let stream = match stream_config.sample_format {
        SampleFormat::F32 => device.build_input_stream(
            &stream_config,
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
            &stream_config,
            move |data: &[i16], _: &_| {
                // Convert i16 to f32
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
            &stream_config,
            move |data: &[u16], _: &_| {
                // Convert u16 to f32
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
        _ => return Err("Unsupported sample format".to_string()),
    }.map_err(|e| format!("Failed to build input stream: {}", e))?;

    stream.play().map_err(|e| format!("Failed to play stream: {}", e))?;

    // Store stream
    let mut stream_guard = state.stream.lock().unwrap();
    *stream_guard = Some(stream);
    *ref_count = 1;

    println!("[Audio] Capture started successfully");
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
    // Iterate by frames (stride = channels)
    // Simple averaging mixing
    for frame in data.chunks(channels) {
        let mut sum = 0.0;
        for sample in frame {
            sum += sample;
        }
        let mono_sample = sum / channels as f32;
        // If buffer is full, we might drop samples (overrun).
        // For now, just try push.
        let _ = producer.push(mono_sample);
    }

    // 2. Process chunks if enough data
    while consumer.occupied_len() >= input_frames_needed {
        // Read into input_buffer
        // input_buffer[0] is the vector for channel 0
        let chunk_slice = &mut input_buffer[0];

        // Read exact amount
        let _read = consumer.pop_slice(chunk_slice);

        // Resample
        // process expects &[Vec<f32>] and returns &[Vec<f32>] (or writes to output)
        // process_into is safer if we have buffers
        let result = resampler.process_into(input_buffer, output_buffer, None);

        if let Ok((_in_len, out_len)) = result {
            if out_len > 0 {
                // Convert back to i16 for frontend
                let output_f32 = &output_buffer[0][..out_len];
                let output_i16: Vec<i16> = output_f32
                    .iter()
                    .map(|&s| {
                        // Clamp and scale
                        let s = s.clamp(-1.0, 1.0);
                        (s * 32767.0) as i16
                    })
                    .collect();

                // Emit
                // We emit to all windows or just the caller?
                // The caller window is passed in.
                // But we probably want to emit to all listening windows if we want to share?
                // Actually `window.emit` emits to that window.
                // If we have multiple windows (e.g. main and caption), we might need `app_handle.emit`.
                // But `window` is fine if both components are in the same window (Webview).
                // "LiveRecord" is in main window. "Caption" is a separate window?
                // Wait, `CaptionWindow` is usually a separate WebviewWindow.
                // If so, `window.emit` only goes to that window.
                // The instruction was: "Send the data to the frontend."
                // If "Live Record" (Main) starts it, Main gets data.
                // If "Caption" (Overlay) starts it, Overlay gets data.
                // If BOTH start it, we have a problem:
                // `stream` is singleton. The callback captures `window_clone`.
                // `window_clone` points to the window that *started* the stream (e.g. Main).
                // If Caption starts later, it increments refcount but DOES NOT update the callback or window.
                // So Caption won't get events if Main started it.

                // FIX: Use `emit_to("main", ...)` and `emit_to("caption", ...)`?
                // Or emit to all. `window.app_handle().emit_all(...)` (deprecated in v2? use `emit` on app handle).

                // Let's use `window.app_handle().emit(...)` which emits globally to all windows listening.
                // But `window` here is `Window<R>`. `window.app_handle()` exists.

                let _ = window.app_handle().emit("system-audio", &output_i16);
            }
        }
    }
}

#[tauri::command]
pub fn stop_system_audio_capture(
    state: tauri::State<'_, AudioState>,
) -> Result<(), String> {
    let mut ref_count = state.ref_count.lock().unwrap();
    let mut stream_guard = state.stream.lock().unwrap();

    if *ref_count > 0 {
        *ref_count -= 1;
        println!("[Audio] Stopping capture... Ref count: {}", *ref_count);
    } else {
        println!("[Audio] Stop requested but ref count is 0");
    }

    if *ref_count == 0 {
        if stream_guard.is_some() {
            println!("[Audio] Dropping stream.");
            *stream_guard = None; // Drop stream -> stops capture
        }
    }

    Ok(())
}
