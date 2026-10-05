use crate::error::{AudioCaptureError, AudioCaptureResult};
use cpal::traits::HostTrait;

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AudioDevice {
    pub name: String,
}

impl AudioDevice {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: sanitize_device_name(&name.into()),
        }
    }
}

pub fn sanitize_device_name(name: &str) -> String {
    let mut sanitized = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_control() {
            sanitized.extend(character.escape_default());
        } else {
            sanitized.push(character);
        }
    }
    sanitized
}

pub const NO_INPUT_DEVICES_FOUND: &str = "No audio input devices found.";

pub fn format_device_list(devices: &[String], default_device: Option<&str>) -> String {
    if devices.is_empty() {
        return format!("{NO_INPUT_DEVICES_FOUND}\n");
    }
    let entries = devices
        .iter()
        .enumerate()
        .map(|(idx, device)| {
            let clean_device = sanitize_device_name(device);
            if default_device == Some(device.as_str())
                || default_device == Some(clean_device.as_str())
            {
                format!("[{idx}] {clean_device} [default]")
            } else {
                format!("[{idx}] {clean_device}")
            }
        })
        .collect::<Vec<_>>();
    format!("{}\n", entries.join("\n"))
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
        .map(|device| sanitize_device_name(&device.to_string()))
        .collect::<Vec<_>>();
    devices.sort();
    devices.dedup();
    Ok(devices)
}

pub fn default_input_device_name() -> Option<String> {
    cpal::default_host()
        .default_input_device()
        .map(|device| sanitize_device_name(&device.to_string()))
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
    if let Some(raw_requested) = requested_device {
        let requested = raw_requested.trim();
        if requested.is_empty() {
            return default_device
                .map(str::to_string)
                .ok_or(missing_default_error);
        }
        // 1. Exact match (try raw first, then trimmed)
        if let Some(exact) = devices
            .iter()
            .find(|name| name.as_str() == raw_requested || name.as_str() == requested)
        {
            return Ok(exact.clone());
        }
        // 2. Numeric index match (e.g. "0", "1")
        if let Some(indexed) = requested
            .parse::<usize>()
            .ok()
            .and_then(|index| devices.get(index))
        {
            return Ok(indexed.clone());
        }
        // 3. Case-insensitive substring match
        let requested_lower = requested.to_lowercase();
        let matches: Vec<&String> = devices
            .iter()
            .filter(|name| name.to_lowercase().contains(&requested_lower))
            .collect();
        if matches.len() == 1 {
            return Ok(matches[0].clone());
        } else if matches.len() > 1 {
            return Err(AudioCaptureError::DeviceNotFound(format!(
                "Ambiguous device query '{requested}'. Multiple matching devices: {}",
                matches
                    .iter()
                    .map(|s| format!("'{}'", s))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }
        return Err(AudioCaptureError::DeviceNotFound(requested.to_string()));
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

    let mut device_names = devices
        .iter()
        .map(|device| sanitize_device_name(&device.to_string()))
        .collect::<Vec<_>>();
    device_names.sort();
    device_names.dedup();
    let default_name = host
        .default_input_device()
        .map(|device| sanitize_device_name(&device.to_string()));
    let resolved_name =
        resolve_device_name(&device_names, default_name.as_deref(), requested_name)?;

    let device = devices
        .into_iter()
        .find(|device| sanitize_device_name(&device.to_string()) == resolved_name)
        .or_else(|| {
            host.default_input_device()
                .filter(|device| sanitize_device_name(&device.to_string()) == resolved_name)
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

    let mut device_names = devices
        .iter()
        .map(|device| sanitize_device_name(&device.to_string()))
        .collect::<Vec<_>>();
    device_names.sort();
    device_names.dedup();
    let default_name = host
        .default_output_device()
        .map(|device| sanitize_device_name(&device.to_string()));
    let resolved_name = resolve_device_name_or(
        &device_names,
        default_name.as_deref(),
        requested_name,
        AudioCaptureError::NoOutputDeviceFound,
    )?;
    let device = devices
        .into_iter()
        .find(|device| sanitize_device_name(&device.to_string()) == resolved_name)
        .or_else(|| {
            host.default_output_device()
                .filter(|device| sanitize_device_name(&device.to_string()) == resolved_name)
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

    #[test]
    fn test_resolve_device_name_by_index_and_substring() {
        let devices = vec![
            "Microphone (Realtek High Definition Audio)".to_string(),
            "USB Audio Device".to_string(),
            "USB Audio Device 2".to_string(),
        ];
        // Numeric index
        assert_eq!(
            resolve_device_name(&devices, None, Some("0")).unwrap(),
            "Microphone (Realtek High Definition Audio)"
        );
        assert_eq!(
            resolve_device_name(&devices, None, Some("1")).unwrap(),
            "USB Audio Device"
        );
        // Substring case-insensitive match
        assert_eq!(
            resolve_device_name(&devices, None, Some("realtek")).unwrap(),
            "Microphone (Realtek High Definition Audio)"
        );
        // Ambiguous substring
        let err = resolve_device_name(&devices, None, Some("usb")).unwrap_err();
        assert!(err.to_string().contains("Ambiguous device query"));
        // Not found
        assert!(matches!(
            resolve_device_name(&devices, None, Some("bluetooth")).unwrap_err(),
            AudioCaptureError::DeviceNotFound(_)
        ));
        // Empty or whitespace query falls back to default device
        assert_eq!(
            resolve_device_name(&devices, Some("USB Audio Device"), Some("")).unwrap(),
            "USB Audio Device"
        );
        assert_eq!(
            resolve_device_name(&devices, Some("USB Audio Device"), Some("   ")).unwrap(),
            "USB Audio Device"
        );
    }

    #[test]
    fn test_sanitize_device_name() {
        assert_eq!(
            sanitize_device_name("Mic\x1b[31m\r\n\tTest"),
            "Mic\\u{1b}[31m\\r\\n\\tTest"
        );
        assert_eq!(sanitize_device_name("Clean Microphone"), "Clean Microphone");
    }

    #[test]
    fn test_format_device_list() {
        assert_eq!(
            format_device_list(&[], None),
            "No audio input devices found.\n"
        );
        let devices = vec!["Mic A".to_string(), "Mic B".to_string()];
        assert_eq!(
            format_device_list(&devices, Some("Mic A")),
            "[0] Mic A [default]\n[1] Mic B\n"
        );
        assert_eq!(format_device_list(&devices, None), "[0] Mic A\n[1] Mic B\n");
    }
}
