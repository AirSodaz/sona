#[cfg(target_os = "windows")]
struct ComGuard {
    need_uninitialize: bool,
}

#[cfg(target_os = "windows")]
impl ComGuard {
    unsafe fn enter() -> Self {
        use windows::Win32::System::Com::CoInitialize;
        // CoInitialize returns Ok(()) on S_OK (0) or S_FALSE (1).
        // If it was already initialized with a different concurrency model (RPC_E_CHANGED_MODE),
        // it returns an Err, in which case we do not uninitialize.
        let hr = unsafe { CoInitialize(None) };
        Self {
            need_uninitialize: hr.is_ok(),
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.need_uninitialize {
            use windows::Win32::System::Com::CoUninitialize;
            unsafe {
                CoUninitialize();
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn set_mute_windows(mute: bool) -> Result<(), String> {
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        IMMDeviceEnumerator, MMDeviceEnumerator, eConsole, eRender,
    };
    use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance};

    let _com_guard = unsafe { ComGuard::enter() };

    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).map_err(|e| e.to_string())?;

        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|e| e.to_string())?;

        let volume: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|e| e.to_string())?;

        volume
            .SetMute(mute, std::ptr::null())
            .map_err(|e: windows::core::Error| e.to_string())?;
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn set_mute_macos(mute: bool) -> Result<(), String> {
    use std::process::Command;

    let state = if mute { "true" } else { "false" };
    let output = Command::new("osascript")
        .arg("-e")
        .arg(format!("set volume output muted {}", state))
        .output()
        .map_err(|e| e.to_string())?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn set_mute_linux(mute: bool) -> Result<(), String> {
    use std::process::Command;

    let state = if mute { "1" } else { "0" };
    let pactl_res = Command::new("pactl")
        .args(["set-sink-mute", "@DEFAULT_SINK@", state])
        .output();
    if pactl_res.map(|out| out.status.success()).unwrap_or(false) {
        return Ok(());
    }

    let amixer_state = if mute { "mute" } else { "unmute" };
    if Command::new("amixer")
        .args(["-D", "pulse", "set", "Master", amixer_state])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }
    if Command::new("amixer")
        .args(["set", "Master", amixer_state])
        .output()
        .map(|out| out.status.success())
        .unwrap_or(false)
    {
        return Ok(());
    }

    Err("Failed to set mute state on Linux".to_string())
}

pub async fn set_system_audio_mute(mute: bool) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    return set_mute_windows(mute);

    #[cfg(target_os = "macos")]
    return set_mute_macos(mute);

    #[cfg(target_os = "linux")]
    return set_mute_linux(mute);

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    Err("Unsupported platform".to_string())
}
