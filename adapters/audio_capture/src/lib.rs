pub mod device;
pub mod downmix;
pub mod error;
pub mod pipeline;
pub mod resampler;
pub mod stream;

pub use device::{
    AudioDevice, enumerate_input_device_names, enumerate_input_devices, enumerate_output_devices,
    find_input_device, find_output_device, resolve_device_name,
};
pub use downmix::{
    downmix_f32, downmix_i16, downmix_u16, push_downmixed_f32, push_downmixed_f32_checked,
    push_downmixed_i16, push_downmixed_i16_checked, push_downmixed_u16, push_downmixed_u16_checked,
};
pub use error::{AudioCaptureError, AudioCaptureResult};
pub use pipeline::{
    CaptureEvent, CaptureNotifier, LiveAudioCaptureHandle, LiveAudioCapturePipeline,
    LiveCaptureConfig,
};
pub use resampler::{AudioResampler, DEFAULT_CHUNK_SIZE_OUT, TARGET_SAMPLE_RATE};
pub use stream::{CaptureDirection, build_cpal_input_stream, open_device_stream};
