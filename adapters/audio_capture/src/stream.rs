use crate::downmix::{
    push_downmixed_f32_checked, push_downmixed_i16_checked, push_downmixed_u16_checked,
};
use crate::error::{AudioCaptureError, AudioCaptureResult};
use cpal::SampleFormat;
use cpal::traits::DeviceTrait;
use ringbuf::traits::Producer;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

/// Builds and configures a real-time safe CPAL input stream.
///
/// The audio callback ONLY executes in-place zero-allocation mono downmixing into `raw_producer`
/// and signals `data_notifier`. It performs NO allocations, NO FFT, and NO blocking locks.
pub fn build_cpal_input_stream(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    sample_format: SampleFormat,
    mut raw_producer: impl Producer<Item = f32> + Send + 'static,
    data_notifier: impl Fn() + Send + Sync + 'static,
    error_callback: impl FnMut(cpal::Error) + Send + 'static,
    overflow: Arc<AtomicBool>,
) -> AudioCaptureResult<cpal::Stream> {
    if config.channels == 0 || config.channels > 64 {
        return Err(AudioCaptureError::ConfigFailed(format!(
            "Invalid audio channel count: {} (expected 1..=64)",
            config.channels
        )));
    }
    if config.sample_rate < 8000 || config.sample_rate > 384_000 {
        return Err(AudioCaptureError::ConfigFailed(format!(
            "Unsupported audio sample rate: {} Hz (expected 8000..=384000 Hz)",
            config.sample_rate
        )));
    }
    let channels = config.channels as usize;
    let stream = match sample_format {
        SampleFormat::F32 => {
            let overflow = Arc::clone(&overflow);
            device
                .build_input_stream(
                    config,
                    move |data: &[f32], _| {
                        push_downmixed_f32_checked(data, channels, &mut raw_producer, &overflow);
                        data_notifier();
                    },
                    error_callback,
                    None,
                )
                .map_err(|e| AudioCaptureError::BuildStreamFailed(e.to_string()))?
        }
        SampleFormat::I16 => {
            let overflow = Arc::clone(&overflow);
            device
                .build_input_stream(
                    config,
                    move |data: &[i16], _| {
                        push_downmixed_i16_checked(data, channels, &mut raw_producer, &overflow);
                        data_notifier();
                    },
                    error_callback,
                    None,
                )
                .map_err(|e| AudioCaptureError::BuildStreamFailed(e.to_string()))?
        }
        SampleFormat::U16 => {
            let overflow = Arc::clone(&overflow);
            device
                .build_input_stream(
                    config,
                    move |data: &[u16], _| {
                        push_downmixed_u16_checked(data, channels, &mut raw_producer, &overflow);
                        data_notifier();
                    },
                    error_callback,
                    None,
                )
                .map_err(|e| AudioCaptureError::BuildStreamFailed(e.to_string()))?
        }
        other => {
            return Err(AudioCaptureError::UnsupportedSampleFormat(format!(
                "{other:?}"
            )));
        }
    };

    Ok(stream)
}

/// Direction of audio capture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CaptureDirection {
    /// Microphone or line-in capture.
    Input,
    /// Loopback or system audio capture.
    Output,
}

/// Opens a real-time CPAL stream with automatic configuration negotiation and mono downmixing.
pub fn open_device_stream(
    device: &cpal::Device,
    direction: CaptureDirection,
    raw_producer: impl Producer<Item = f32> + Send + 'static,
    data_notifier: impl Fn() + Send + Sync + 'static,
    error_callback: impl FnMut(cpal::Error) + Send + 'static,
    overflow: Arc<AtomicBool>,
) -> AudioCaptureResult<(cpal::Stream, cpal::StreamConfig, SampleFormat)> {
    let supported_config = match direction {
        CaptureDirection::Input => device.default_input_config().map_err(|e| {
            AudioCaptureError::ConfigFailed(format!("Failed to get default input config: {e}"))
        })?,
        CaptureDirection::Output => device.default_output_config().map_err(|e| {
            AudioCaptureError::ConfigFailed(format!("Failed to get default output config: {e}"))
        })?,
    };
    let sample_format = supported_config.sample_format();
    let config: cpal::StreamConfig = supported_config.into();
    let stream = build_cpal_input_stream(
        device,
        config,
        sample_format,
        raw_producer,
        data_notifier,
        error_callback,
        overflow,
    )?;
    Ok((stream, config, sample_format))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpal::traits::HostTrait;
    use ringbuf::traits::{Consumer, Split};

    #[test]
    fn test_build_stream_rejects_zero_or_excessive_channels() {
        let host = cpal::default_host();
        if let Some(device) = host.default_input_device() {
            let rb = ringbuf::HeapRb::<f32>::new(16);
            let (prod, _cons) = rb.split();
            let config = cpal::StreamConfig {
                channels: 0,
                sample_rate: 44100,
                buffer_size: cpal::BufferSize::Default,
            };
            assert!(matches!(
                build_cpal_input_stream(
                    &device,
                    config,
                    SampleFormat::F32,
                    prod,
                    || {},
                    |_| {},
                    Arc::new(AtomicBool::new(false)),
                ),
                Err(AudioCaptureError::ConfigFailed(_))
            ));
        }
    }

    #[test]
    fn test_loopback_output_capture() {
        use cpal::traits::StreamTrait;
        let host = cpal::default_host();
        if let Some(device) = host.default_output_device() {
            let rb = ringbuf::HeapRb::<f32>::new(4096);
            let (prod, mut cons) = rb.split();
            let res = open_device_stream(
                &device,
                CaptureDirection::Output,
                prod,
                || {},
                |_err| {},
                Arc::new(AtomicBool::new(false)),
            );
            if let Ok((stream, _config, _format)) = res {
                assert!(stream.play().is_ok());
                std::thread::sleep(std::time::Duration::from_millis(150));
                let mut buf = [0.0f32; 256];
                let _popped = cons.pop_slice(&mut buf);
            }
        }
    }
}
