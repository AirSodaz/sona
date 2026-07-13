use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::HeapRb;
use ringbuf::traits::Producer;
use ringbuf::traits::{Consumer, Split};
use rubato::{FftFixedOut, Resampler};
use std::io::Read;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

const TARGET_SAMPLE_RATE: usize = 16_000;
const RESAMPLER_OUTPUT_CHUNK: usize = 1024;
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

fn push_samples_to_ring(
    producer: &mut impl Producer<Item = f32>,
    samples: &[f32],
    overflow: &AtomicBool,
) {
    for sample in samples {
        if producer.try_push(*sample).is_err() {
            overflow.store(true, Ordering::Release);
            break;
        }
    }
}

fn resolve_device_name(
    devices: &[String],
    default_device: Option<&str>,
    requested_device: Option<&str>,
) -> Result<String, String> {
    if let Some(requested) = requested_device {
        return devices
            .iter()
            .find(|name| name.as_str() == requested)
            .cloned()
            .ok_or_else(|| format!("Input device not found: {requested}"));
    }
    default_device
        .map(str::to_string)
        .ok_or_else(|| "No default input device found".to_string())
}

pub(crate) fn microphone_device_names() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let mut devices = host
        .input_devices()
        .map_err(|error| format!("Failed to enumerate input devices: {error}"))?
        .map(|device| device.to_string())
        .collect::<Vec<_>>();
    devices.sort();
    devices.dedup();
    Ok(devices)
}

pub(crate) fn start_microphone_input(
    requested_device: Option<&str>,
) -> Result<RunningAudioInput, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|error| format!("Failed to enumerate input devices: {error}"))?
        .collect::<Vec<_>>();
    let device_names = devices.iter().map(ToString::to_string).collect::<Vec<_>>();
    let default_name = host.default_input_device().map(|device| device.to_string());
    let resolved_name =
        resolve_device_name(&device_names, default_name.as_deref(), requested_device)?;

    let device = devices
        .into_iter()
        .find(|device| device.to_string() == resolved_name)
        .or_else(|| {
            host.default_input_device()
                .filter(|device| device.to_string() == resolved_name)
        })
        .ok_or_else(|| format!("Input device disappeared before capture: {resolved_name}"))?;

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
    let channels = config.channels as usize;
    let buffer = HeapRb::<f32>::new(sample_rate as usize * INPUT_BUFFER_SECONDS);
    let (mut producer, mut consumer) = buffer.split();
    let overflow = std::sync::Arc::new(AtomicBool::new(false));
    let capture_failure = CaptureFailure::default();
    let callback_failure = capture_failure.clone();
    let stream_error = move |error| {
        callback_failure.record(format!("Microphone stream failed: {error}"));
    };

    let stream_result = match sample_format {
        SampleFormat::F32 => {
            let overflow = overflow.clone();
            device.build_input_stream(
                config,
                move |data: &[f32], _| {
                    if let Ok(samples) = downmix_f32(data, channels) {
                        push_samples_to_ring(&mut producer, &samples, &overflow);
                    }
                },
                stream_error,
                None,
            )
        }
        SampleFormat::I16 => {
            let overflow = overflow.clone();
            device.build_input_stream(
                config,
                move |data: &[i16], _| {
                    if let Ok(samples) = downmix_i16(data, channels) {
                        push_samples_to_ring(&mut producer, &samples, &overflow);
                    }
                },
                stream_error,
                None,
            )
        }
        SampleFormat::U16 => {
            let overflow = overflow.clone();
            device.build_input_stream(
                config,
                move |data: &[u16], _| {
                    if let Ok(samples) = downmix_u16(data, channels) {
                        push_samples_to_ring(&mut producer, &samples, &overflow);
                    }
                },
                stream_error,
                None,
            )
        }
        _ => {
            let _ = startup_sender.send(Err(format!(
                "Unsupported input sample format for {device_name}: {sample_format:?}"
            )));
            return;
        }
    };
    let stream = match stream_result {
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

    let mut resampler = match MonoResampler::new(sample_rate) {
        Ok(resampler) => resampler,
        Err(error) => {
            let _ = sender.blocking_send(LiveAudioMessage::Error(error));
            return;
        }
    };
    loop {
        if forward_capture_failure(&capture_failure, &sender) {
            return;
        }
        if overflow.load(Ordering::Acquire) {
            let _ = sender.blocking_send(LiveAudioMessage::Error(
                "Microphone input buffer overflowed; transcription cannot continue reliably."
                    .to_string(),
            ));
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
    match resampler.finish() {
        Ok(samples) if !samples.is_empty() => {
            if sender
                .blocking_send(LiveAudioMessage::Chunk(LiveAudioChunk::Samples(samples)))
                .is_err()
            {
                return;
            }
        }
        Ok(_) => {}
        Err(error) => {
            let _ = sender.blocking_send(LiveAudioMessage::Error(error));
            return;
        }
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
    resampler: &mut MonoResampler,
    sender: &tokio::sync::mpsc::Sender<LiveAudioMessage>,
) -> Result<(), ()> {
    while consumer.occupied_len() > 0 {
        let mut samples = vec![0.0; consumer.occupied_len().min(4096)];
        let read = consumer.pop_slice(&mut samples);
        samples.truncate(read);
        let output = match resampler.push(&samples) {
            Ok(output) => output,
            Err(error) => {
                let _ = sender.blocking_send(LiveAudioMessage::Error(error));
                return Err(());
            }
        };
        if !output.is_empty()
            && sender
                .blocking_send(LiveAudioMessage::Chunk(LiveAudioChunk::Samples(output)))
                .is_err()
        {
            return Err(());
        }
    }
    Ok(())
}

fn downmix<T>(
    samples: &[T],
    channels: usize,
    normalize: impl Fn(&T) -> f32,
) -> Result<Vec<f32>, String> {
    if channels == 0 {
        return Err("audio input reported zero channels".to_string());
    }
    Ok(samples
        .chunks(channels)
        .map(|frame| frame.iter().map(&normalize).sum::<f32>() / frame.len() as f32)
        .collect())
}

pub(crate) fn downmix_f32(samples: &[f32], channels: usize) -> Result<Vec<f32>, String> {
    downmix(samples, channels, |sample| *sample)
}

pub(crate) fn downmix_i16(samples: &[i16], channels: usize) -> Result<Vec<f32>, String> {
    downmix(samples, channels, |sample| *sample as f32 / 32_768.0)
}

pub(crate) fn downmix_u16(samples: &[u16], channels: usize) -> Result<Vec<f32>, String> {
    downmix(samples, channels, |sample| {
        (*sample as f32 - 32_767.5) / 32_767.5
    })
}

pub(crate) struct MonoResampler {
    inner: Option<FftFixedOut<f32>>,
    pending: Vec<f32>,
}

impl MonoResampler {
    pub(crate) fn new(input_sample_rate: u32) -> Result<Self, String> {
        if input_sample_rate == 0 {
            return Err("audio input reported a zero sample rate".to_string());
        }
        let inner = if input_sample_rate as usize == TARGET_SAMPLE_RATE {
            None
        } else {
            Some(
                FftFixedOut::<f32>::new(
                    input_sample_rate as usize,
                    TARGET_SAMPLE_RATE,
                    RESAMPLER_OUTPUT_CHUNK,
                    2,
                    1,
                )
                .map_err(|error| format!("Failed to create microphone resampler: {error}"))?,
            )
        };
        Ok(Self {
            inner,
            pending: Vec::new(),
        })
    }

    pub(crate) fn push(&mut self, samples: &[f32]) -> Result<Vec<f32>, String> {
        let Some(resampler) = self.inner.as_mut() else {
            return Ok(samples.to_vec());
        };
        self.pending.extend_from_slice(samples);
        let mut output = Vec::new();
        loop {
            let input_frames = resampler.input_frames_next();
            if self.pending.len() < input_frames {
                break;
            }
            let input = self.pending.drain(..input_frames).collect::<Vec<_>>();
            let rendered = resampler
                .process(&[input], None)
                .map_err(|error| format!("Failed to resample microphone audio: {error}"))?;
            output.extend_from_slice(&rendered[0]);
        }
        Ok(output)
    }

    pub(crate) fn finish(mut self) -> Result<Vec<f32>, String> {
        let Some(resampler) = self.inner.as_mut() else {
            return Ok(Vec::new());
        };
        let mut output = Vec::new();
        if !self.pending.is_empty() {
            let input = std::mem::take(&mut self.pending);
            let rendered = resampler
                .process_partial(Some(&[input]), None)
                .map_err(|error| format!("Failed to flush microphone resampler: {error}"))?;
            output.extend_from_slice(&rendered[0]);
        }
        let delayed = resampler
            .process_partial::<Vec<f32>>(None, None)
            .map_err(|error| format!("Failed to flush microphone resampler: {error}"))?;
        output.extend_from_slice(&delayed[0]);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ringbuf::HeapRb;
    use ringbuf::traits::Split;
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

        push_samples_to_ring(&mut producer, &[0.1, 0.2, 0.3], &overflow);

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
            resolve_device_name(&devices, Some("Laptop Mic"), Some("studio mic")).unwrap_err(),
            "Input device not found: studio mic"
        );
    }
}
