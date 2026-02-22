use std::path::PathBuf;
use std::process::Stdio;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::io::{AsyncReadExt, BufReader, AsyncBufReadExt};
use tokio::process::{Command, Child};
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

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
    // Attempt to resolve "sidecar/dist/ffmpeg" (or with .exe on Windows)
    let mut path_str = "sidecar/dist/ffmpeg".to_string();
    if cfg!(target_os = "windows") {
        path_str.push_str(".exe");
    }

    // Debug logging for path resolution
    println!("[Audio] Resolving FFmpeg path: {}", path_str);

    let resource_path = app.path().resolve(&path_str, tauri::path::BaseDirectory::Resource)
        .map_err(|e| format!("Failed to resolve ffmpeg path: {}", e))?;

    println!("[Audio] Resolved path: {:?}", resource_path);

    if !resource_path.exists() {
        // Fallback check: maybe it's flattened?
        let flat_name = if cfg!(target_os = "windows") { "ffmpeg.exe" } else { "ffmpeg" };
        let flat_path = app.path().resolve(flat_name, tauri::path::BaseDirectory::Resource)
            .map_err(|e| format!("Failed to resolve flat path: {}", e))?;

        if flat_path.exists() {
            println!("[Audio] Found FFmpeg at flat path: {:?}", flat_path);
            return Ok(flat_path);
        }

        return Err(format!("FFmpeg binary not found at: {:?} or {:?}", resource_path, flat_path));
    }

    Ok(resource_path)
}

#[tauri::command]
pub async fn get_audio_devices(app: AppHandle) -> Result<Vec<AudioDevice>, String> {
    let ffmpeg_path = get_ffmpeg_path(&app)?;
    let mut devices = Vec::new();

    println!("[Audio] Listing devices using FFmpeg at: {:?}", ffmpeg_path);

    #[cfg(target_os = "windows")]
    {
        // Windows: ffmpeg -list_devices true -f dshow -i dummy
        let output = Command::new(&ffmpeg_path)
            .args(&["-list_devices", "true", "-f", "dshow", "-i", "dummy"])
            .output()
            .await
            .map_err(|e| e.to_string())?;

        // combine stdout and stderr just in case
        let combined = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        println!("[Audio] Device list output:\n{}", combined);

        for line in combined.lines() {
            // Check for "[dshow @" prefix and "(audio)" suffix
            // Example: [dshow @ ...] "Mic Name" (audio)
            if line.contains("[dshow @") && line.contains("(audio)") {
                if let Some(start_quote) = line.find('"') {
                    if let Some(end_quote) = line[start_quote+1..].find('"') {
                        let name = &line[start_quote+1..start_quote+1+end_quote];
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

        let combined = format!("{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        println!("[Audio] Device list output:\n{}", combined);

        let mut in_audio_section = false;

        for line in combined.lines() {
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
                if let Some(bracket_start) = line.find('[') {
                    if let Some(bracket_end) = line.rfind(']') {
                        if bracket_end > bracket_start && bracket_end + 2 < line.len() {
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
        println!("[Audio] Listing PulseAudio sources...");
        // 1. Try pactl
        let mut pactl_success = false;
        if let Ok(output) = Command::new("pactl")
            .args(&["list", "short", "sources"])
            .output()
            .await
        {
            if output.status.success() {
                pactl_success = true;
                let stdout = String::from_utf8_lossy(&output.stdout);
                println!("[Audio] pactl output:\n{}", stdout);
                for line in stdout.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 {
                        let name = parts[1];
                        devices.push(AudioDevice {
                            id: name.to_string(),
                            label: name.to_string(),
                        });
                    }
                }
            }
        }

        if !pactl_success {
             println!("[Audio] pactl failed or not found. Falling back to default.");
             devices.push(AudioDevice {
                id: "default".to_string(),
                label: "Default (PulseAudio)".to_string(),
            });
        }
    }

    // Always add a "default" device if list is empty to allow trying
    if devices.is_empty() {
        println!("[Audio] No devices found. Adding generic default.");
        #[cfg(target_os = "windows")]
        devices.push(AudioDevice { id: "default".to_string(), label: "Default Device".to_string() });
        #[cfg(target_os = "macos")]
        devices.push(AudioDevice { id: ":0".to_string(), label: "Default Input".to_string() });
        #[cfg(target_os = "linux")]
        devices.push(AudioDevice { id: "default".to_string(), label: "Default".to_string() });
    }

    println!("[Audio] Returning devices: {:?}", devices);
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

    // Use Vec<String> to own arguments
    let mut args: Vec<String> = Vec::new();

    #[cfg(target_os = "windows")]
    {
        args.push("-f".to_string());
        args.push("dshow".to_string());
        args.push("-i".to_string());
        if device_id == "default" {
             return Err("Cannot capture default dshow device without name".to_string());
        }
        args.push(format!("audio={}", device_id)); // device_id is name
    }

    #[cfg(target_os = "macos")]
    {
        args.push("-f".to_string());
        args.push("avfoundation".to_string());
        args.push("-i".to_string());
        if !device_id.starts_with(":") {
             args.push(format!(":{}", device_id));
        } else {
             args.push(device_id);
        }
    }

    #[cfg(target_os = "linux")]
    {
        args.push("-f".to_string());
        args.push("pulse".to_string());
        args.push("-i".to_string());
        args.push(device_id); // device_id is source name or "default"
    }

    // Common output args: raw PCM s16le 16kHz mono to stdout
    args.push("-ac".to_string());
    args.push("1".to_string());
    args.push("-ar".to_string());
    args.push("16000".to_string());
    args.push("-f".to_string());
    args.push("s16le".to_string());
    args.push("-".to_string());

    println!("[Audio] Spawning FFmpeg: {:?} {:?}", ffmpeg_path, args);

    // Spawn process
    let mut cmd = Command::new(ffmpeg_path);
    cmd.args(&args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = cmd.spawn().map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to open stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to open stderr")?;

    // Spawn a task to read stdout and emit events
    let app_handle = app.clone();
    tokio::spawn(async move {
        let mut reader = BufReader::new(stdout);
        let mut buffer = [0u8; 4096]; // 4KB chunks

        loop {
            match reader.read(&mut buffer).await {
                Ok(0) => {
                    println!("[Audio] FFmpeg stdout EOF");
                    break;
                },
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
        let _ = app_handle.emit("audio-capture-stopped", ());
    });

    // Spawn a task to read stderr
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            println!("[FFmpeg] {}", line);
        }
    });

    *process_guard = Some(child);
    Ok(())
}

#[tauri::command]
pub async fn stop_audio_capture(state: State<'_, AudioCaptureState>) -> Result<(), String> {
    let mut process_guard = state.process.lock().await;

    if let Some(mut child) = process_guard.take() {
        println!("[Audio] Stopping capture process...");
        let _ = child.kill().await;
        return Ok(());
    }

    Ok(())
}
