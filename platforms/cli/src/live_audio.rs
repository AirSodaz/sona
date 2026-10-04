use sona_audio_capture::{
    CaptureDirection, CaptureEvent, LiveAudioCapturePipeline, LiveCaptureConfig,
};
use std::io::Read;
use std::sync::Arc;

const STDIN_READ_BUFFER_SIZE: usize = 8192;
#[derive(Debug, PartialEq)]
pub enum LiveAudioChunk {
    PcmS16Le(Vec<u8>),
    Samples(Vec<f32>),
}

#[derive(Debug, PartialEq)]
pub enum LiveAudioMessage {
    Chunk(LiveAudioChunk),
    Eof,
    Error(String),
}

pub struct RunningAudioInput {
    pub(crate) receiver: tokio::sync::mpsc::Receiver<LiveAudioMessage>,
    stop_sender: Option<std::sync::mpsc::Sender<()>>,
    stop_handle: Option<Arc<dyn Fn() + Send + Sync>>,
    pub(crate) device_name: Option<String>,
    drain_on_stop: bool,
}

impl RunningAudioInput {
    pub fn from_parts(
        receiver: tokio::sync::mpsc::Receiver<LiveAudioMessage>,
        stop_sender: Option<std::sync::mpsc::Sender<()>>,
        device_name: Option<String>,
        drain_on_stop: bool,
    ) -> Self {
        Self {
            receiver,
            stop_sender,
            stop_handle: None,
            device_name,
            drain_on_stop,
        }
    }

    pub fn with_stop_handle(mut self, stop_handle: Arc<dyn Fn() + Send + Sync>) -> Self {
        self.stop_handle = Some(stop_handle);
        self
    }

    pub(crate) fn request_stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
        if let Some(handle) = self.stop_handle.as_ref() {
            handle();
        }
    }

    pub(crate) fn should_drain_on_stop(&self) -> bool {
        self.drain_on_stop
    }
}

impl Drop for RunningAudioInput {
    fn drop(&mut self) {
        self.request_stop();
        self.receiver.close();
        let _ = self.stop_handle.take();
    }
}

#[derive(Debug, Default)]
pub(crate) struct PcmS16LeDecoder {
    pending_byte: Option<u8>,
}

impl PcmS16LeDecoder {
    pub(crate) fn push(&mut self, bytes: &[u8]) -> Vec<u8> {
        let mut output = Vec::with_capacity(bytes.len() + usize::from(self.pending_byte.is_some()));
        if let Some(byte) = self.pending_byte.take() {
            output.push(byte);
        }
        output.extend_from_slice(bytes);
        if output.len() % 2 != 0 {
            self.pending_byte = output.pop();
        }
        output
    }

    pub(crate) fn finish(self) -> Result<(), String> {
        if self.pending_byte.is_some() {
            Err("stdin ended with an incomplete 16-bit PCM sample.".to_string())
        } else {
            Ok(())
        }
    }
}

pub(crate) fn spawn_stdin_reader<R>(mut reader: R) -> RunningAudioInput
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = tokio::sync::mpsc::channel(16);
    std::thread::spawn(move || {
        let mut decoder = PcmS16LeDecoder::default();
        let mut buffer = vec![0_u8; STDIN_READ_BUFFER_SIZE];
        loop {
            let read = match reader.read(&mut buffer) {
                Ok(read) => read,
                Err(error) => {
                    let _ = sender.blocking_send(LiveAudioMessage::Error(format!(
                        "Failed to read PCM from stdin: {error}"
                    )));
                    return;
                }
            };
            if read == 0 {
                let message = match decoder.finish() {
                    Ok(()) => LiveAudioMessage::Eof,
                    Err(error) => LiveAudioMessage::Error(error),
                };
                let _ = sender.blocking_send(message);
                return;
            }
            let pcm = decoder.push(&buffer[..read]);
            if !pcm.is_empty()
                && sender
                    .blocking_send(LiveAudioMessage::Chunk(LiveAudioChunk::PcmS16Le(pcm)))
                    .is_err()
            {
                return;
            }
        }
    });
    RunningAudioInput::from_parts(receiver, None, None, false)
}

pub(crate) fn microphone_device_names() -> Result<Vec<String>, String> {
    sona_audio_capture::enumerate_input_device_names().map_err(|e| e.to_string())
}

pub(crate) fn default_microphone_device_name() -> Option<String> {
    sona_audio_capture::default_input_device_name()
}

pub(crate) fn start_microphone_input(
    requested_device: Option<&str>,
) -> Result<RunningAudioInput, String> {
    let (device, resolved_name) =
        sona_audio_capture::find_input_device(requested_device).map_err(|e| e.to_string())?;

    let (message_sender, receiver) = tokio::sync::mpsc::channel(128);
    let sender = message_sender.clone();
    let pipeline = LiveAudioCapturePipeline::start(
        &device,
        &resolved_name,
        CaptureDirection::Input,
        LiveCaptureConfig::default(),
        move |event| match event {
            CaptureEvent::Chunk(samples) => {
                match sender.try_send(LiveAudioMessage::Chunk(LiveAudioChunk::Samples(samples))) {
                    Ok(()) => true,
                    Err(tokio::sync::mpsc::error::TrySendError::Full(_)) => {
                        log::warn!(
                            "[CLI Audio] Audio chunk buffer full; dropping frame to avoid blocking capture thread"
                        );
                        true
                    }
                    Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => false,
                }
            }
            CaptureEvent::Error(err) => {
                log::error!("[CLI Audio] Audio capture device error: {err}");
                let _ = sender.blocking_send(LiveAudioMessage::Error(err));
                false
            }
            CaptureEvent::Eof => {
                let _ = sender.blocking_send(LiveAudioMessage::Eof);
                false
            }
        },
    )
    .map_err(|e| e.to_string())?;

    let stop_handle = Arc::new(move || {
        pipeline.stop();
    });

    Ok(
        RunningAudioInput::from_parts(receiver, None, Some(resolved_name), true)
            .with_stop_handle(stop_handle),
    )
}

#[cfg(test)]
#[derive(Clone, Default)]
struct CaptureFailure(Arc<std::sync::Mutex<Option<String>>>);

#[cfg(test)]
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

#[cfg(test)]
fn forward_capture_failure(
    failure: &CaptureFailure,
    sender: &tokio::sync::mpsc::Sender<LiveAudioMessage>,
) -> bool {
    let Some(error) = failure.take() else {
        return false;
    };
    let _ = sender.blocking_send(LiveAudioMessage::Error(error));
    true
}

#[cfg(test)]
use sona_audio_capture::{AudioResampler, CaptureNotifier};
#[cfg(test)]
use std::time::Duration;

#[cfg(test)]
pub(crate) fn downmix_f32(samples: &[f32], channels: usize) -> Result<Vec<f32>, String> {
    sona_audio_capture::downmix_f32(samples, channels).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) fn downmix_i16(samples: &[i16], channels: usize) -> Result<Vec<f32>, String> {
    sona_audio_capture::downmix_i16(samples, channels).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) fn downmix_u16(samples: &[u16], channels: usize) -> Result<Vec<f32>, String> {
    sona_audio_capture::downmix_u16(samples, channels).map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) struct MonoResampler(AudioResampler);

#[cfg(test)]
impl MonoResampler {
    pub(crate) fn new(input_sample_rate: u32) -> Result<Self, String> {
        AudioResampler::new(input_sample_rate)
            .map(Self)
            .map_err(|e| e.to_string())
    }

    pub(crate) fn push(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        self.0.process_chunk(samples).map_err(|e| e.to_string())
    }

    pub(crate) fn finish(self) -> Result<Vec<f32>, String> {
        self.0.finish().map_err(|e| e.to_string())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::HeapRb;
    use ringbuf::traits::Split;
    use sona_audio_capture::{push_downmixed_f32_checked, resolve_device_name};
    use std::io::Cursor;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn pcm_decoder_preserves_samples_split_across_read_boundaries() {
        let mut decoder = PcmS16LeDecoder::default();

        assert_eq!(decoder.push(&[0x00]), Vec::<u8>::new());
        assert_eq!(
            decoder.push(&[0x40, 0x00, 0x80]),
            vec![0x00, 0x40, 0x00, 0x80]
        );
        assert!(decoder.finish().is_ok());
    }

    #[test]
    fn pcm_decoder_rejects_incomplete_final_sample() {
        let mut decoder = PcmS16LeDecoder::default();
        assert!(decoder.push(&[0xff]).is_empty());

        assert_eq!(
            decoder.finish().unwrap_err(),
            "stdin ended with an incomplete 16-bit PCM sample."
        );
    }

    #[test]
    fn sample_formats_are_normalized_and_channels_are_averaged() {
        assert_eq!(
            downmix_f32(&[1.0, -1.0, 0.5, 0.5], 2).unwrap(),
            vec![0.0, 0.5]
        );
        assert_eq!(
            downmix_i16(&[i16::MAX, i16::MIN], 2).unwrap(),
            vec![-1.0 / 65_536.0]
        );
        assert_eq!(downmix_u16(&[u16::MIN, u16::MAX], 2).unwrap(), vec![0.0]);
        assert_eq!(
            downmix_f32(&[1.0], 0).unwrap_err(),
            "audio input reported zero channels"
        );
    }

    #[test]
    fn resampler_flushes_audio_shorter_than_one_full_input_window() {
        let mut resampler = MonoResampler::new(48_000).unwrap();
        let output = resampler.push(&vec![0.25; 480]).unwrap();
        assert!(output.is_empty());

        let tail = resampler.finish().unwrap();
        assert!(!tail.is_empty());
        assert!(tail.iter().all(|sample| sample.is_finite()));
    }

    #[tokio::test]
    async fn stdin_reader_emits_even_pcm_chunks_then_eof() {
        let mut input = spawn_stdin_reader(Cursor::new(vec![0x00, 0x40, 0x00, 0x80]));

        assert_eq!(
            input.receiver.recv().await.unwrap(),
            LiveAudioMessage::Chunk(LiveAudioChunk::PcmS16Le(vec![0x00, 0x40, 0x00, 0x80]))
        );
        assert_eq!(input.receiver.recv().await.unwrap(), LiveAudioMessage::Eof);
    }

    #[tokio::test]
    async fn stdin_reader_reports_incomplete_final_sample() {
        let mut input = spawn_stdin_reader(Cursor::new(vec![0xff]));

        assert_eq!(
            input.receiver.recv().await.unwrap(),
            LiveAudioMessage::Error(
                "stdin ended with an incomplete 16-bit PCM sample.".to_string()
            )
        );
    }

    #[test]
    fn ring_buffer_overflow_is_reported() {
        let buffer = HeapRb::<f32>::new(2);
        let (mut producer, _consumer) = buffer.split();
        let overflow = AtomicBool::new(false);

        push_downmixed_f32_checked(&[0.1, 0.2, 0.3], 1, &mut producer, &overflow);

        assert!(overflow.load(Ordering::Acquire));
    }

    #[test]
    fn microphone_stream_error_is_retained_while_audio_channel_is_full() {
        let (sender, _receiver) = tokio::sync::mpsc::channel(1);
        sender.try_send(LiveAudioMessage::Eof).unwrap();
        let failure = CaptureFailure::default();

        failure.record("Microphone stream failed: device disconnected".to_string());

        assert_eq!(
            failure.take().as_deref(),
            Some("Microphone stream failed: device disconnected")
        );
        assert!(failure.take().is_none());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn microphone_stream_error_waits_for_space_in_a_full_audio_channel() {
        let (sender, mut receiver) = tokio::sync::mpsc::channel(1);
        sender.send(LiveAudioMessage::Eof).await.unwrap();
        let failure = CaptureFailure::default();
        failure.record("Microphone stream failed: device disconnected".to_string());
        let forward =
            tokio::task::spawn_blocking(move || forward_capture_failure(&failure, &sender));

        tokio::task::yield_now().await;
        assert!(!forward.is_finished());
        assert_eq!(receiver.recv().await, Some(LiveAudioMessage::Eof));
        assert!(forward.await.unwrap());
        assert_eq!(
            receiver.recv().await,
            Some(LiveAudioMessage::Error(
                "Microphone stream failed: device disconnected".to_string()
            ))
        );
    }

    #[test]
    fn device_selection_uses_exact_or_index_or_default() {
        let devices = vec!["Laptop Mic".to_string(), "Studio Mic".to_string()];

        assert_eq!(
            resolve_device_name(&devices, Some("Laptop Mic"), Some("Studio Mic")).unwrap(),
            "Studio Mic"
        );
        assert_eq!(
            resolve_device_name(&devices, Some("Laptop Mic"), None).unwrap(),
            "Laptop Mic"
        );
        assert_eq!(
            resolve_device_name(&devices, Some("Laptop Mic"), Some("1")).unwrap(),
            "Studio Mic"
        );
        assert_eq!(
            resolve_device_name(&devices, Some("Laptop Mic"), Some("studio mic")).unwrap(),
            "Studio Mic"
        );
        assert_eq!(
            resolve_device_name(&devices, Some("Laptop Mic"), Some("Unknown Mic"))
                .unwrap_err()
                .to_string(),
            "Input device not found: Unknown Mic"
        );
    }

    #[test]
    fn capture_notifier_wakes_immediately_on_notify() {
        let notifier = Arc::new(CaptureNotifier::default());
        let notifier_clone = notifier.clone();
        let start = std::time::Instant::now();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(10));
            notifier_clone.notify();
        });
        notifier.wait(Duration::from_secs(5));
        assert!(start.elapsed() < Duration::from_millis(500));
    }

    #[test]
    fn capture_notifier_pre_notified_does_not_wait() {
        let notifier = CaptureNotifier::default();
        notifier.notify();
        let start = std::time::Instant::now();
        notifier.wait(Duration::from_secs(5));
        assert!(start.elapsed() < Duration::from_millis(50));
    }
}
