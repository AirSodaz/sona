use crate::error::{AudioCaptureError, AudioCaptureResult};
use cpal::traits::HostTrait;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct AudioDevice {
    pub name: String,
}

impl AudioDevice {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }
}

pub fn enumerate_input_devices() -> AudioCaptureResult<Vec<AudioDevice>> {
    let names = enumerate_input_device_names()?;
    Ok(names.into_iter().map(AudioDevice::new).collect())
}

pub fn enumerate_input_device_names() -> AudioCaptureResult<Vec<String>> {
    let host = cpal::default_host();
    let mut devices = host
        .input_devices()
        .map_err(|e| AudioCaptureError::EnumerationFailed(e.to_string()))?
        .map(|device| device.to_string())
        .collect::<Vec<_>>();
    devices.sort();
    devices.dedup();
    Ok(devices)
}

pub fn enumerate_output_devices() -> AudioCaptureResult<Vec<AudioDevice>> {
    let host = cpal::default_host();
    let mut devices = host
        .output_devices()
        .map_err(|e| AudioCaptureError::EnumerationFailed(e.to_string()))?
        .map(|device| AudioDevice::new(device.to_string()))
        .collect::<Vec<_>>();
    devices.sort_by(|a, b| a.name.cmp(&b.name));
    devices.dedup_by(|a, b| a.name == b.name);
    Ok(devices)
}

pub fn resolve_device_name(
    devices: &[String],
    default_device: Option<&str>,
    requested_device: Option<&str>,
) -> AudioCaptureResult<String> {
    resolve_device_name_or(
        devices,
        default_device,
        requested_device,
        AudioCaptureError::NoInputDeviceFound,
    )
}

pub fn resolve_device_name_or(
    devices: &[String],
    default_device: Option<&str>,
    requested_device: Option<&str>,
    missing_default_error: AudioCaptureError,
) -> AudioCaptureResult<String> {
    if let Some(requested) = requested_device {
        return devices
            .iter()
            .find(|name| name.as_str() == requested)
            .cloned()
            .ok_or_else(|| AudioCaptureError::DeviceNotFound(requested.to_string()));
    }
    default_device
        .map(str::to_string)
        .ok_or(missing_default_error)
}

pub fn find_input_device(
    requested_name: Option<&str>,
) -> AudioCaptureResult<(cpal::Device, String)> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| AudioCaptureError::EnumerationFailed(e.to_string()))?
        .collect::<Vec<_>>();

    let device_names = devices.iter().map(ToString::to_string).collect::<Vec<_>>();
    let default_name = host.default_input_device().map(|device| device.to_string());
    let resolved_name =
        resolve_device_name(&device_names, default_name.as_deref(), requested_name)?;

    let device = devices
        .into_iter()
        .find(|device| device.to_string() == resolved_name)
        .or_else(|| {
            host.default_input_device()
                .filter(|device| device.to_string() == resolved_name)
        })
        .ok_or_else(|| AudioCaptureError::DeviceNotFound(resolved_name.clone()))?;

    Ok((device, resolved_name))
}

pub fn find_output_device(
    requested_name: Option<&str>,
) -> AudioCaptureResult<(cpal::Device, String)> {
    let host = cpal::default_host();
    let devices = host
        .output_devices()
        .map_err(|e| AudioCaptureError::EnumerationFailed(e.to_string()))?
        .collect::<Vec<_>>();

    let device_names = devices.iter().map(ToString::to_string).collect::<Vec<_>>();
    let default_name = host
        .default_output_device()
        .map(|device| device.to_string());
    let resolved_name = resolve_device_name_or(
        &device_names,
        default_name.as_deref(),
        requested_name,
        AudioCaptureError::NoOutputDeviceFound,
    )?;
    let device = devices
        .into_iter()
        .find(|device| device.to_string() == resolved_name)
        .or_else(|| {
            host.default_output_device()
                .filter(|device| device.to_string() == resolved_name)
        })
        .ok_or_else(|| AudioCaptureError::DeviceNotFound(resolved_name.clone()))?;

    Ok((device, resolved_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_device_name_fallback_input_vs_output() {
        let devices = vec!["Speaker".to_string(), "Headphones".to_string()];
        assert_eq!(
            resolve_device_name(&devices, Some("Speaker"), None).unwrap(),
            "Speaker"
        );
        assert!(matches!(
            resolve_device_name(&devices, None, None).unwrap_err(),
            AudioCaptureError::NoInputDeviceFound
        ));
        assert!(matches!(
            resolve_device_name_or(&devices, None, None, AudioCaptureError::NoOutputDeviceFound)
                .unwrap_err(),
            AudioCaptureError::NoOutputDeviceFound
        ));
    }
}
