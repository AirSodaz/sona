use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncReadExt, BufReader, AsyncBufReadExt};
use tokio::process::{Command, Child};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct AudioDevice {
    pub id: String,
    pub label: String,
}

pub struct AudioCaptureState {
    pub process: Mutex<Option<Child>>,
}

impl AudioCaptureState {
    pub fn new() -> Self {
        Self {
            process: Mutex::new(None),
        }
    }
}

/// Resolves the path to the bundled FFmpeg binary.
fn get_ffmpeg_path(app: &AppHandle) -> Result<PathBuf, String> {
    // In Tauri v2, resources are resolved relative to the resource directory.
    // The sidecar build puts ffmpeg in `sidecar/dist/ffmpeg(.exe)`.
    // This path must match what is in tauri.conf.json resources.

    // Attempt to resolve "sidecar/dist/ffmpeg" (or with .exe on Windows)
    let mut path_str = "sidecar/dist/ffmpeg".to_string();
    if cfg!(target_os = "windows") {
        path_str.push_str(".exe");
    }

    let resource_path = app.path().resolve(&path_str, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve ffmpeg path: {}", e))?;

    if !resource_path.exists() {
        return Err(format!("FFmpeg binary not found at: {:?}", resource_path));
    }

    Ok(resource_path)
}

#[tauri::command]
pub async fn get_audio_devices(app: AppHandle) -> Result<Vec<AudioDevice>, String> {
    let ffmpeg_path = get_ffmpeg_path(&app)?;
    let mut devices = Vec::new();

    #[cfg(target_os = "windows")]
    {
        // Windows: ffmpeg -list_devices true -f dshow -i dummy
        let output = Command::new(&ffmpeg_path)
            .args(&["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut in_audio_section = false;

        for line in stderr.lines() {
            if line.contains("DirectShow audio devices") {
                in_audio_section = true;
                continue;
            }
            if line.contains("DirectShow video devices") {
                in_audio_section = false;
                continue;
            }

            if in_audio_section {
                // Line format: [dshow @ ...]  "Device Name"
                // We verify it starts with [dshow
                if let Some(start_quote) = line.find('"') {
                    if let Some(end_quote) = line[start_quote+1..].find('"') {
                        let name = &line[start_quote+1..start_quote+1+end_quote];
                        // Skip if name is empty or looks like alternative name
                        if !name.trim().is_empty() {
                            devices.push(AudioDevice {
                                id: name.to_string(), // For dshow, id is the name
                                label: name.to_string(),
                            });
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: ffmpeg -f avfoundation -list_devices true -i ""
        let output = Command::new(&ffmpeg_path)
            .args(&["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut in_audio_section = false;

        for line in stderr.lines() {
            if line.contains("AVFoundation audio devices:") {
                in_audio_section = true;
                continue;
            }
            if line.contains("AVFoundation video devices:") {
                in_audio_section = false;
                continue;
            }

            if in_audio_section {
                // Line format: [AVFoundation indev @ ...] [INDEX] Name
                // Example: [AVFoundation indev @ 0x...] [0] MacBook Pro Microphone
                // We need to find [INDEX]
                if let Some(bracket_start) = line.find('[') {
                    // There might be multiple brackets (log prefix). The index is usually the last one before name?
                    // Actually, standard format is `... [Index] Name`
                    // Let's look for `] ` pattern
                    if let Some(bracket_end) = line.rfind(']') {
                        if bracket_end > bracket_start && bracket_end + 2 < line.len() {
                            // Extract index
                            let slice = &line[..bracket_end];
                            if let Some(last_open) = slice.rfind('[') {
                                let index_str = &slice[last_open+1..];
                                if let Ok(index) = index_str.parse::<usize>() {
                                    let name = &line[bracket_end+1..].trim();
                                    devices.push(AudioDevice {
                                        id: index.to_string(),
                                        label: name.to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: Try pulse first using pactl if available, otherwise just return Default
        // Parsing ffmpeg output for pulse is hard.
        // Let's try to run `pactl list short sources`
        match Command::new("pactl")
            .args(&["list", "short", "sources"])
            .output()
            .await
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                for line in stdout.lines() {
                    // Format: ID Name Module SampleSpec State
                    // 0	alsa_output.pci...	module-alsa-card.c	s16le 2ch 44100Hz	SUSPENDED
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[1];
                        // Filter out monitors if we only want inputs?
                        // "monitor" usually means system audio loopback!
                        // If user wants system audio, we SHOULD include monitors.
                        devices.push(AudioDevice {
                            id: name.to_string(),
                            label: name.to_string(),
                        });
                    }
                }
            },
            Err(_) => {
                // Fallback: just add "default"
                devices.push(AudioDevice {
                    id: "default".to_string(),
                    label: "Default".to_string(),
                });
            }
        }
    }

    Ok(devices)
}

#[tauri::command]
pub async fn start_audio_capture(
    app: AppHandle,
    state: State<'_, AudioCaptureState>,
    device_id: String,
) -> Result<(), String> {
    let mut process_guard = state.process.lock().await;

    if process_guard.is_some() {
        return Err("Capture already running".to_string());
    }

    let ffmpeg_path = get_ffmpeg_path(&app)?;

    let mut args = Vec::new();

    #[cfg(target_os = "windows")]
    {
        args.push("-f");
        args.push("dshow");
        args.push("-i");
        args.push(&format!("audio={}", device_id)); // device_id is name
    }

    #[cfg(target_os = "macos")]
    {
        args.push("-f");
        args.push("avfoundation");
        args.push("-i");
        args.push(&format!(":{}", device_id)); // device_id is index
    }

    #[cfg(target_os = "linux")]
    {
        args.push("-f");
        args.push("pulse");
        args.push("-i");
        args.push(&device_id); // device_id is source name
    }

    // Common output args: raw PCM s16le 16kHz mono to stdout
    args.extend_from_slice(&[
        "-ac", "1",
        "-ar", "16000",
        "-f", "s16le",
        "-"
    ]);

    // Spawn process
    // Use unsafe block for windows-specific flag if needed to hide window?
    // Tauri Command usually handles this.
    // We are using tokio::process::Command directly.

    let mut cmd = Command::new(ffmpeg_path);
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to open stderr")?; // Capture stderr to log errors if needed

    // Spawn a task to read stdout and emit events
    let app_handle = app.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buffer = [0u8; 4096]; // 4KB chunks

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => break, // EOF
                Ok(n) => {
                    let chunk = buffer[0..n].to_vec();
                    // Emit to frontend
                    if let Err(e) = app_handle.emit("audio-packet", chunk) {
                        eprintln!("Failed to emit audio packet: {}", e);
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("Error reading audio stream: {}", e);
                    break;
                }
            }
        }
        // Process finished
        let _ = app_handle.emit("audio-capture-stopped", ());
    });

    // Spawn a task to read stderr (optional logging)
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            // eprintln!("[FFmpeg] {}", line); // Log only if debug?
        }
    });

    *process_guard = Some(child);
    Ok(())
}

#[tauri::command]
pub async fn stop_audio_capture(state: State<'_, AudioCaptureState>) -> Result<(), String> {
    let mut process_guard = state.process.lock().await;

    if let Some(mut child) = process_guard.take() {
        let _ = child.kill().await;
        return Ok(());
    }

    Ok(())
}
