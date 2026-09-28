use cpal::traits::{DeviceTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::{Consumer, Split};
use sona_audio_capture::{AudioResampler, build_cpal_input_stream};
use std::io::Read;
use std::sync::{Arc, Mutex};

const STDIN_READ_BUFFER_SIZE: usize = 8192;
const INPUT_BUFFER_SECONDS: usize = 5;
const CAPTURE_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(5);

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

#[derive(Debug, PartialEq)]
pub(crate) enum LiveAudioChunk {
    PcmS16Le(Vec<u8>),
    Samples(Vec<f32>),
}

#[derive(Debug, PartialEq)]
pub(crate) enum LiveAudioMessage {
    Chunk(LiveAudioChunk),
    Eof,
    Error(String),
}

pub(crate) struct RunningAudioInput {
    pub(crate) receiver: tokio::sync::mpsc::Receiver<LiveAudioMessage>,
    stop_sender: Option<std::sync::mpsc::Sender<()>>,
    pub(crate) device_name: Option<String>,
    drain_on_stop: bool,
}

impl RunningAudioInput {
    pub(crate) fn from_parts(
        receiver: tokio::sync::mpsc::Receiver<LiveAudioMessage>,
        stop_sender: Option<std::sync::mpsc::Sender<()>>,
        device_name: Option<String>,
        drain_on_stop: bool,
    ) -> Self {
        Self {
            receiver,
            stop_sender,
            device_name,
            drain_on_stop,
        }
    }

    pub(crate) fn request_stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }

    pub(crate) fn should_drain_on_stop(&self) -> bool {
        self.drain_on_stop
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

pub(crate) fn start_microphone_input(
    requested_device: Option<&str>,
) -> Result<RunningAudioInput, String> {
    let (device, resolved_name) =
        sona_audio_capture::find_input_device(requested_device).map_err(|e| e.to_string())?;

    let (message_sender, receiver) = tokio::sync::mpsc::channel(16);
    let (stop_sender, stop_receiver) = std::sync::mpsc::channel();
    let (startup_sender, startup_receiver) = std::sync::mpsc::sync_channel(1);
    let capture_name = resolved_name.clone();
    std::thread::spawn(move || {
        run_microphone_capture(
            device,
            capture_name,
            message_sender,
            stop_receiver,
            startup_sender,
        );
    });
    startup_receiver
        .recv()
        .map_err(|error| format!("Microphone startup channel closed: {error}"))??;

    Ok(RunningAudioInput::from_parts(
        receiver,
        Some(stop_sender),
        Some(resolved_name),
        true,
    ))
}

fn run_microphone_capture(
    device: cpal::Device,
    device_name: String,
    sender: tokio::sync::mpsc::Sender<LiveAudioMessage>,
    stop_receiver: std::sync::mpsc::Receiver<()>,
    startup_sender: std::sync::mpsc::SyncSender<Result<(), String>>,
) {
    let supported_config = match device.default_input_config() {
        Ok(config) => config,
        Err(error) => {
            let _ = startup_sender.send(Err(format!(
                "Failed to get input config for {device_name}: {error}"
            )));
            return;
        }
    };
    let sample_format = supported_config.sample_format();
    let config: cpal::StreamConfig = supported_config.into();
    let sample_rate = config.sample_rate;
    let buffer = HeapRb::<f32>::new(sample_rate as usize * INPUT_BUFFER_SECONDS);
    let (producer, mut consumer) = buffer.split();
    let capture_failure = CaptureFailure::default();
    let callback_failure = capture_failure.clone();
    let stream_error = move |error| {
        callback_failure.record(format!("Microphone stream failed: {error}"));
    };

    let stream = match build_cpal_input_stream(
        &device,
        config,
        sample_format,
        producer,
        move || {},
        stream_error,
    ) {
        Ok(stream) => stream,
        Err(error) => {
            let _ = startup_sender.send(Err(format!(
                "Failed to build input stream for {device_name}: {error}"
            )));
            return;
        }
    };
    if let Err(error) = stream.play() {
        let _ = startup_sender.send(Err(format!(
            "Failed to start input stream for {device_name}: {error}"
        )));
        return;
    }
    if startup_sender.send(Ok(())).is_err() {
        return;
    }

    let mut resampler = match AudioResampler::new(sample_rate) {
        Ok(resampler) => resampler,
        Err(error) => {
            let _ = sender.blocking_send(LiveAudioMessage::Error(error.to_string()));
            return;
        }
    };
    loop {
        if forward_capture_failure(&capture_failure, &sender) {
            return;
        }
        if drain_microphone_samples(&mut consumer, &mut resampler, &sender).is_err() {
            return;
        }
        match stop_receiver.try_recv() {
            Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            Err(std::sync::mpsc::TryRecvError::Empty) => std::thread::sleep(CAPTURE_POLL_INTERVAL),
        }
    }
    drop(stream);
    if forward_capture_failure(&capture_failure, &sender) {
        return;
    }
    if drain_microphone_samples(&mut consumer, &mut resampler, &sender).is_err() {
        return;
    }
    let mut tail = Vec::new();
    let finished_count = resampler.drain_finish(&mut consumer, &mut tail);
    if finished_count > 0
        && sender
            .blocking_send(LiveAudioMessage::Chunk(LiveAudioChunk::Samples(tail)))
            .is_err()
    {
        return;
    }
    let _ = sender.blocking_send(LiveAudioMessage::Eof);
}

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

fn drain_microphone_samples(
    consumer: &mut impl Consumer<Item = f32>,
    resampler: &mut AudioResampler,
    sender: &tokio::sync::mpsc::Sender<LiveAudioMessage>,
) -> Result<(), ()> {
    let mut output = Vec::new();
    let produced = resampler.drain_from_consumer(consumer, &mut output);
    if produced > 0
        && sender
            .blocking_send(LiveAudioMessage::Chunk(LiveAudioChunk::Samples(output)))
            .is_err()
    {
        return Err(());
    }
    Ok(())
}

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
    fn device_selection_uses_exact_name_or_default() {
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
            resolve_device_name(&devices, Some("Laptop Mic"), Some("studio mic"))
                .unwrap_err()
                .to_string(),
            "Input device not found: studio mic"
        );
    }
}
