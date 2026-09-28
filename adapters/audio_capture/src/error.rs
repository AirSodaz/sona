use thiserror::Error;

#[derive(Debug, Error)]
pub enum AudioCaptureError {
    #[error("No audio input device found")]
    NoInputDeviceFound,

    #[error("No audio output device found")]
    NoOutputDeviceFound,

    #[error("Input device not found: {0}")]
    DeviceNotFound(String),

    #[error("Failed to enumerate audio devices: {0}")]
    EnumerationFailed(String),

    #[error("{0}")]
    ConfigFailed(String),

    #[error("Failed to build audio stream: {0}")]
    BuildStreamFailed(String),

    #[error("Failed to play audio stream: {0}")]
    PlayStreamFailed(String),

    #[error("Unsupported audio sample format: {0}")]
    UnsupportedSampleFormat(String),

    #[error("Failed to initialize audio resampler: {0}")]
    ResamplerInitFailed(String),

    #[error("Audio resampling processing failed: {0}")]
    ResamplingFailed(String),

    #[error("Audio input buffer overflowed")]
    BufferOverflow,

    #[error("Audio channel closed or startup failed: {0}")]
    ChannelClosed(String),
}

pub type AudioCaptureResult<T> = Result<T, AudioCaptureError>;
