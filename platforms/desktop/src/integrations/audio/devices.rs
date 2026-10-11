pub use sona_audio_capture::AudioDevice;

use super::types::CaptureKind;

pub fn get_system_audio_devices() -> Result<Vec<AudioDevice>, String> {
    sona_audio_capture::enumerate_output_devices().map_err(|e| e.to_string())
}

pub fn get_microphone_devices() -> Result<Vec<AudioDevice>, String> {
    sona_audio_capture::enumerate_input_devices().map_err(|e| e.to_string())
}

pub fn resolve_capture_device(
    kind: CaptureKind,
    device_name: &Option<String>,
) -> Result<(cpal::Device, String), String> {
    match kind {
        CaptureKind::Microphone => sona_audio_capture::find_input_device(device_name.as_deref())
            .map_err(|error| error.to_string()),
        CaptureKind::System => sona_audio_capture::find_output_device(device_name.as_deref())
            .map_err(|error| error.to_string()),
    }
}
