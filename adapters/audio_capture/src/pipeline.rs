use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::Split;

use crate::error::{AudioCaptureError, AudioCaptureResult};
use crate::resampler::AudioResampler;
use crate::stream::{CaptureDirection, open_device_stream};

/// Events emitted by the live audio capture pipeline.
#[derive(Clone, Debug, PartialEq)]
pub enum CaptureEvent {
    /// Mono 16,000 Hz resampled PCM audio chunk.
    Chunk(Vec<f32>),
    /// Hardware capture stream encountered an error.
    Error(String),
    /// Capture finished cleanly.
    Eof,
}

/// Configuration options for the live audio capture pipeline.
#[derive(Clone, Debug)]
pub struct LiveCaptureConfig {
    /// Number of seconds of audio buffer to allocate for the raw ring buffer.
    pub buffer_seconds: usize,
    /// Resampler output chunk size.
    pub chunk_size_out: usize,
    /// Watchdog polling interval when audio notifications are idle.
    pub watchdog_interval: Duration,
}

impl Default for LiveCaptureConfig {
    fn default() -> Self {
        Self {
            buffer_seconds: 5,
            chunk_size_out: 1024,
            watchdog_interval: Duration::from_millis(100),
        }
    }
}

/// Real-time safe notifier between CPAL callback and processing thread.
#[derive(Default)]
pub struct CaptureNotifier {
    ready: AtomicBool,
    mutex: Mutex<()>,
    condvar: Condvar,
}

impl CaptureNotifier {
    /// Signals that new audio samples, a capture error, or a stop command is ready.
    ///
    /// Non-blocking, zero-allocation, and real-time safe for the CPAL audio callback.
    pub fn notify(&self) {
        self.ready.store(true, Ordering::Release);
        self.condvar.notify_one();
    }

    /// Waits until an event arrives or until the watchdog timeout expires.
    pub fn wait(&self, timeout: Duration) {
        if self.ready.swap(false, Ordering::AcqRel) {
            return;
        }

        if let Ok(mut guard) = self.mutex.lock() {
            while !self.ready.swap(false, Ordering::AcqRel) {
                let (next_guard, result) = self
                    .condvar
                    .wait_timeout(guard, timeout)
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard = next_guard;
                if result.timed_out() {
                    break;
                }
            }
        }
    }
}

/// Thread-safe capture failure collector.
#[derive(Clone, Default)]
struct CaptureFailure(Arc<Mutex<Option<String>>>);

impl CaptureFailure {
    fn record(&self, error: String) {
        let mut failure = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if failure.is_none() {
            *failure = Some(error);
        }
    }

    fn take(&self) -> Option<String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
    }
}

/// Handle to a running live audio capture pipeline.
pub struct LiveAudioCaptureHandle {
    device_name: String,
    sample_rate: u32,
    stop_sender: std::sync::mpsc::Sender<()>,
    stop_requested: Arc<AtomicBool>,
    notifier: Arc<CaptureNotifier>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LiveAudioCaptureHandle {
    /// Returns the resolved device name.
    pub fn device_name(&self) -> &str {
        &self.device_name
    }

    /// Returns the native input sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Signals the capture pipeline to stop.
    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
        let _ = self.stop_sender.send(());
        self.notifier.notify();
    }

    /// Stops the capture pipeline and waits for the thread to join.
    pub fn join(&mut self) {
        self.stop();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

impl Drop for LiveAudioCaptureHandle {
    fn drop(&mut self) {
        self.join();
    }
}

/// Unified live audio capture and resampling pipeline.
pub struct LiveAudioCapturePipeline;

impl LiveAudioCapturePipeline {
    /// Starts capturing audio from the specified device and pumps resampled 16kHz mono chunks.
    ///
    /// The handler receives `CaptureEvent` items. Returning `false` from `on_event` signals the
    /// pipeline to stop processing early.
    pub fn start<F>(
        device: &cpal::Device,
        device_name: &str,
        direction: CaptureDirection,
        config: LiveCaptureConfig,
        mut on_event: F,
    ) -> AudioCaptureResult<LiveAudioCaptureHandle>
    where
        F: FnMut(CaptureEvent) -> bool + Send + 'static,
    {
        let (stop_sender, stop_receiver) = std::sync::mpsc::channel();
        let (startup_sender, startup_receiver) = std::sync::mpsc::sync_channel(1);
        let stop_requested = Arc::new(AtomicBool::new(false));
        let stop_requested_for_thread = stop_requested.clone();
        let notifier = Arc::new(CaptureNotifier::default());
        let thread_notifier = notifier.clone();

        let device_clone = device.clone();
        let device_name_string = device_name.to_string();
        let thread_name = format!("audio-capture-{}", device_name);

        let thread = std::thread::Builder::new()
            .name(thread_name)
            .spawn(move || {
                run_capture_worker(
                    device_clone,
                    device_name_string,
                    direction,
                    config,
                    stop_receiver,
                    stop_requested_for_thread,
                    thread_notifier,
                    startup_sender,
                    &mut on_event,
                );
            })
            .map_err(|e| AudioCaptureError::BuildStreamFailed(e.to_string()))?;

        match startup_receiver.recv() {
            Ok(Ok(sample_rate)) => Ok(LiveAudioCaptureHandle {
                device_name: device_name.to_string(),
                sample_rate,
                stop_sender,
                stop_requested,
                notifier,
                thread: Some(thread),
            }),
            Ok(Err(err)) => {
                let _ = thread.join();
                Err(err)
            }
            Err(_) => {
                let _ = thread.join();
                Err(AudioCaptureError::BuildStreamFailed(
                    "Audio capture thread exited prematurely during initialization".to_string(),
                ))
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_capture_worker<F>(
    device: cpal::Device,
    device_name: String,
    direction: CaptureDirection,
    config: LiveCaptureConfig,
    stop_receiver: std::sync::mpsc::Receiver<()>,
    stop_requested: Arc<AtomicBool>,
    notifier: Arc<CaptureNotifier>,
    startup_sender: std::sync::mpsc::SyncSender<AudioCaptureResult<u32>>,
    on_event: &mut F,
) where
    F: FnMut(CaptureEvent) -> bool + Send + 'static,
{
    let sample_rate = match direction {
        CaptureDirection::Input => device.default_input_config().map(|c| c.sample_rate()),
        CaptureDirection::Output => device.default_output_config().map(|c| c.sample_rate()),
    };
    let sample_rate = match sample_rate {
        Ok(rate) => rate,
        Err(e) => {
            let _ = startup_sender.send(Err(AudioCaptureError::ConfigFailed(format!(
                "Failed to get audio config for {device_name}: {e}"
            ))));
            return;
        }
    };

    let buffer = HeapRb::<f32>::new(sample_rate as usize * config.buffer_seconds.max(1));
    let (producer, mut consumer) = buffer.split();

    let capture_failure = CaptureFailure::default();
    let callback_failure = capture_failure.clone();
    let error_notifier = notifier.clone();
    let stream_error = move |error| {
        callback_failure.record(format!("Audio stream failed: {error}"));
        error_notifier.notify();
    };

    let data_notifier = {
        let notifier = notifier.clone();
        move || {
            notifier.notify();
        }
    };

    let (stream, stream_config, _sample_format) =
        match open_device_stream(&device, direction, producer, data_notifier, stream_error) {
            Ok(res) => res,
            Err(e) => {
                let _ = startup_sender.send(Err(e));
                return;
            }
        };
    let sample_rate = stream_config.sample_rate;

    if let Err(e) = stream.play() {
        let _ = startup_sender.send(Err(AudioCaptureError::PlayStreamFailed(e.to_string())));
        return;
    }
    let mut resampler = match AudioResampler::with_chunk_size(sample_rate, config.chunk_size_out) {
        Ok(r) => r,
        Err(e) => {
            let _ = startup_sender.send(Err(AudioCaptureError::ConfigFailed(format!(
                "Failed to initialize audio resampler: {e}"
            ))));
            return;
        }
    };

    if startup_sender.send(Ok(sample_rate)).is_err() {
        return;
    }

    let mut pull_buffer = Vec::new();

    loop {
        if let Some(err) = capture_failure.take() {
            let _ = on_event(CaptureEvent::Error(err));
            return;
        }

        pull_buffer.clear();
        resampler.drain_from_consumer(&mut consumer, &mut pull_buffer);
        if !pull_buffer.is_empty() && !on_event(CaptureEvent::Chunk(pull_buffer.clone())) {
            return;
        }

        if stop_requested.load(Ordering::Acquire) {
            break;
        }
        match stop_receiver.try_recv() {
            Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }

        notifier.wait(config.watchdog_interval);
    }

    drop(stream);

    if let Some(err) = capture_failure.take() {
        let _ = on_event(CaptureEvent::Error(err));
        return;
    }

    pull_buffer.clear();
    resampler.drain_from_consumer(&mut consumer, &mut pull_buffer);
    if !pull_buffer.is_empty() && !on_event(CaptureEvent::Chunk(pull_buffer.clone())) {
        return;
    }

    let mut tail = Vec::new();
    let finished_count = resampler.drain_finish(&mut consumer, &mut tail);
    if finished_count > 0 && !on_event(CaptureEvent::Chunk(tail)) {
        return;
    }

    let _ = on_event(CaptureEvent::Eof);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_capture_config_defaults() {
        let config = LiveCaptureConfig::default();
        assert_eq!(config.buffer_seconds, 5);
        assert_eq!(config.chunk_size_out, 1024);
        assert_eq!(config.watchdog_interval, Duration::from_millis(100));
    }

    #[test]
    fn capture_notifier_signals_and_resets() {
        let notifier = CaptureNotifier::default();
        notifier.notify();
        // wait should consume the ready flag immediately without blocking
        let start = std::time::Instant::now();
        notifier.wait(Duration::from_millis(50));
        assert!(start.elapsed() < Duration::from_millis(25));
    }

    #[test]
    fn capture_failure_records_first_error() {
        let failure = CaptureFailure::default();
        assert_eq!(failure.take(), None);
        failure.record("first error".to_string());
        failure.record("second error".to_string());
        assert_eq!(failure.take(), Some("first error".to_string()));
        assert_eq!(failure.take(), None);
    }

    #[test]
    fn capture_event_equality() {
        assert_eq!(
            CaptureEvent::Chunk(vec![0.5, 0.25]),
            CaptureEvent::Chunk(vec![0.5, 0.25])
        );
        assert_ne!(
            CaptureEvent::Chunk(vec![0.5]),
            CaptureEvent::Chunk(vec![0.25])
        );
        assert_eq!(CaptureEvent::Eof, CaptureEvent::Eof);
        assert_eq!(
            CaptureEvent::Error("fail".into()),
            CaptureEvent::Error("fail".into())
        );
    }
}
