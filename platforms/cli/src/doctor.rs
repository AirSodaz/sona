use clap::Args;
use serde::Serialize;
use std::path::{Path, PathBuf};

use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
#[command(
    about = "Check system dependencies, audio devices, and models directory",
    after_help = "Examples:\n  sona-cli doctor\n  sona-cli doctor --json\n  sona-cli doctor --models-dir ./models"
)]
pub struct DoctorArgs {
    /// Override the models directory to inspect.
    #[arg(long, help = "Override the target models directory")]
    pub models_dir: Option<PathBuf>,
    /// Optional config file to inspect.
    #[arg(short = 'c', long = "config", value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Custom path to the ffmpeg executable.
    #[arg(long = "ffmpeg-path", value_name = "PATH")]
    pub ffmpeg_path: Option<String>,
    /// Print machine-readable JSON.
    #[arg(short = 'j', long, help = "Print machine-readable JSON")]
    pub json: bool,
    /// Return non-zero exit code if health check fails or has warnings.
    #[arg(
        long,
        help = "Return non-zero exit code if health check fails or has warnings"
    )]
    pub strict: bool,
}

#[derive(Debug, Serialize)]
pub struct DoctorReport {
    pub all_ok: bool,
    pub ffmpeg: DoctorFfmpegStatus,
    pub audio_input: DoctorAudioStatus,
    pub models: DoctorModelsStatus,
    pub hardware_acceleration: DoctorHardwareStatus,
    pub config: DoctorConfigStatus,
}

#[derive(Debug, Serialize)]
pub struct DoctorFfmpegStatus {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorAudioStatus {
    pub available: bool,
    pub device_count: usize,
    pub default_device: Option<String>,
    pub devices: Vec<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorModelsStatus {
    pub path: String,
    pub exists: bool,
    pub writable: bool,
    pub installed_count: usize,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorHardwareStatus {
    pub available_modes: Vec<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct DoctorConfigStatus {
    pub path: Option<String>,
    pub found: bool,
    pub valid: bool,
    pub message: String,
}

pub async fn run_doctor(args: DoctorArgs) -> CliResult<CliOutput> {
    let report = inspect_system(&args).await;

    if args.strict && !report.all_ok {
        return Err(CliError::Other(
            "Doctor health check failed: environment has warnings or missing dependencies."
                .to_string(),
        ));
    }

    if args.json {
        let output = serde_json::to_string_pretty(&report)
            .map_err(|error| CliError::Serialize(error.to_string()))?;
        return Ok(CliOutput::stdout(output));
    }

    let mut lines = Vec::new();
    lines.push("Sona CLI System Health Check:".to_string());
    lines.push("--------------------------------------------------".to_string());

    // 1. Built-in decoder and FFmpeg
    lines
        .push("  [OK]   Built-in Audio Decoder: Ready (MP3, WAV, M4A, AAC, FLAC, OGG)".to_string());
    if report.ffmpeg.found {
        let ver = report
            .ffmpeg
            .version
            .as_deref()
            .unwrap_or("unknown version");
        let path = report.ffmpeg.path.as_deref().unwrap_or("-");
        lines.push(format!("  [OK]   FFmpeg: {path} ({ver})"));
    } else if args.ffmpeg_path.is_some() {
        lines.push(format!("  [WARN] FFmpeg: {}", report.ffmpeg.message));
    } else {
        lines.push("  [INFO] FFmpeg: Not installed (optional; video extraction and extended formats disabled)".to_string());
        lines.push(format!("         Hint: {}", ffmpeg_install_suggestion()));
    }
    // 2. Audio input
    if report.audio_input.available {
        let def = report
            .audio_input
            .default_device
            .as_deref()
            .unwrap_or("none");
        lines.push(format!(
            "  [OK]   Audio Input: {} device(s) found (default: {def})",
            report.audio_input.device_count
        ));
    } else {
        lines.push(format!(
            "  [WARN] Audio Input: {}",
            report.audio_input.message
        ));
        lines.push("         Hint: Connect a microphone or grant microphone permissions in system settings".to_string());
    }

    // 3. Models directory
    if report.models.exists && report.models.installed_count > 0 {
        lines.push(format!(
            "  [OK]   Models: {} ({} preset model(s) installed)",
            report.models.path, report.models.installed_count
        ));
    } else if report.models.exists {
        lines.push(format!(
            "  [INFO] Models: {} (0 preset models installed)",
            report.models.path
        ));
        lines.push("         Hint: Run 'sona-cli models download whisper-turbo' or 'sona-cli models list --recommended'".to_string());
    } else {
        lines.push(format!("  [WARN] Models: {}", report.models.message));
        lines.push("         Hint: Create the models directory or run 'sona-cli models download whisper-turbo'".to_string());
    }

    // 4. Hardware acceleration
    lines.push(format!(
        "  [OK]   Hardware Acceleration: {} ({})",
        report.hardware_acceleration.message,
        report.hardware_acceleration.available_modes.join(", ")
    ));
    // 5. Config
    if report.config.found {
        if report.config.valid {
            lines.push(format!(
                "  [OK]   Configuration: {} (valid)",
                report.config.path.as_deref().unwrap_or("-")
            ));
        } else {
            lines.push(format!("  [FAIL] Configuration: {}", report.config.message));
        }
    } else {
        lines.push(format!("  [INFO] Configuration: {}", report.config.message));
    }

    lines.push("--------------------------------------------------".to_string());
    if report.all_ok {
        lines.push("Environment is ready for speech recognition.".to_string());
    } else {
        lines.push("Environment has warnings. Some optional features may be limited.".to_string());
    }
    Ok(CliOutput::stdout(lines.join("\n")))
}

async fn inspect_system(args: &DoctorArgs) -> DoctorReport {
    // 1. FFmpeg
    let ffmpeg_status = inspect_ffmpeg(args.ffmpeg_path.as_deref());

    // 2. Audio input
    let audio_status = inspect_audio();

    // 3. Models
    let models_status = inspect_models(args.models_dir.as_deref());

    // 4. Hardware acceleration
    let hw_status = inspect_hardware().await;
    // 5. Config
    let config_status = inspect_config(args.config.as_deref());
    let ffmpeg_ok = ffmpeg_status.found || args.ffmpeg_path.is_none();
    let all_ok = ffmpeg_ok
        && audio_status.available
        && models_status.exists
        && (!config_status.found || config_status.valid);
    DoctorReport {
        all_ok,
        ffmpeg: ffmpeg_status,
        audio_input: audio_status,
        models: models_status,
        hardware_acceleration: hw_status,
        config: config_status,
    }
}
async fn inspect_hardware() -> DoctorHardwareStatus {
    let mut available_modes = vec!["auto".to_string(), "cpu".to_string()];
    let mut details = Vec::new();

    #[cfg(target_os = "macos")]
    {
        if std::env::consts::ARCH == "aarch64" {
            available_modes.push("metal".to_string());
            details.push("Apple Silicon Metal acceleration supported");
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        let cuda_available = sona_sherpa_onnx::gpu::check_gpu_availability()
            .await
            .unwrap_or(false);
        if cuda_available {
            available_modes.push("cuda".to_string());
            details.push("NVIDIA CUDA detected and available");
        }
        if check_vulkan_availability().await {
            available_modes.push("vulkan".to_string());
            details.push("Vulkan GPU acceleration detected and available");
        }
    }

    let message = if details.is_empty() {
        "CPU mode active (no dedicated GPU detected)".to_string()
    } else {
        details.join("; ")
    };

    DoctorHardwareStatus {
        available_modes,
        message,
    }
}

#[cfg(not(target_os = "macos"))]
async fn check_vulkan_availability() -> bool {
    // 1. Check if vulkaninfo can query physical devices successfully
    let vulkaninfo_ok = tokio::process::Command::new("vulkaninfo")
        .arg("--summary")
        .output()
        .await
        .is_ok_and(|output| output.status.success());
    if vulkaninfo_ok {
        return true;
    }
    // 2. On Windows, check for vulkan-1.dll loader
    #[cfg(target_os = "windows")]
    {
        let system32_vulkan_ok = std::env::var("SystemRoot").is_ok_and(|root| {
            std::path::Path::new(&root)
                .join("System32")
                .join("vulkan-1.dll")
                .is_file()
        });
        if system32_vulkan_ok
            || std::path::Path::new("C:\\Windows\\System32\\vulkan-1.dll").is_file()
        {
            return true;
        }
    }
    // 3. On Linux, check common libvulkan paths
    #[cfg(target_os = "linux")]
    {
        for path in [
            "/usr/lib/x86_64-linux-gnu/libvulkan.so.1",
            "/usr/lib/libvulkan.so.1",
            "/usr/lib64/libvulkan.so.1",
            "/usr/local/lib/libvulkan.so.1",
        ] {
            if std::path::Path::new(path).is_file() {
                return true;
            }
        }
    }
    false
}

fn inspect_ffmpeg(custom_path: Option<&str>) -> DoctorFfmpegStatus {
    let custom_p = custom_path.map(Path::new);
    match sona_core::ports::asr::resolve_ffmpeg_path(custom_p) {
        Ok(path) if path.is_file() => {
            let version = std::process::Command::new(&path)
                .arg("-version")
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        let text = String::from_utf8_lossy(&out.stdout);
                        text.lines().next().map(|line| line.trim().to_string())
                    } else {
                        None
                    }
                });
            DoctorFfmpegStatus {
                found: true,
                path: Some(path.display().to_string()),
                version,
                message: "FFmpeg executable found and responsive.".to_string(),
            }
        }
        _ => {
            let message = if let Some(custom) = custom_path {
                format!("Custom FFmpeg executable not found at '{custom}'.")
            } else {
                "FFmpeg is not installed (optional). Built-in decoder handles standard audio (MP3, WAV, M4A, FLAC, OGG).".to_string()
            };
            DoctorFfmpegStatus {
                found: false,
                path: None,
                version: None,
                message,
            }
        }
    }
}

fn inspect_audio() -> DoctorAudioStatus {
    match crate::live_audio::microphone_device_names() {
        Ok(devices) => {
            let default_device = crate::live_audio::default_microphone_device_name();
            let count = devices.len();
            let available = count > 0;
            let message = if available {
                format!("{count} audio capture device(s) found.")
            } else {
                "No audio capture devices found. Microphone recording unavailable.".to_string()
            };
            DoctorAudioStatus {
                available,
                device_count: count,
                default_device,
                devices,
                message,
            }
        }
        Err(err) => DoctorAudioStatus {
            available: false,
            device_count: 0,
            default_device: None,
            devices: Vec::new(),
            message: format!("Failed to query audio devices: {err}"),
        },
    }
}

fn inspect_models(models_dir_override: Option<&Path>) -> DoctorModelsStatus {
    let resolved_dir = match models_dir_override {
        Some(dir) => dir.to_path_buf(),
        None => match std::env::var_os("SONA_MODELS_DIR") {
            Some(env_dir) if !env_dir.is_empty() => PathBuf::from(env_dir),
            _ => crate::desktop_paths::default_models_dir()
                .unwrap_or_else(|| PathBuf::from("./models")),
        },
    };

    let exists = resolved_dir.is_dir();
    let writable = if exists {
        let probe_filename = format!(".sona_probe_write_{}", std::process::id());
        let probe_path = resolved_dir.join(probe_filename);
        match std::fs::write(&probe_path, b"") {
            Ok(_) => {
                let _ = std::fs::remove_file(&probe_path);
                true
            }
            Err(_) => false,
        }
    } else {
        false
    };

    let installed_count = if exists {
        sona_runtime_fs::list_models(&resolved_dir)
            .into_iter()
            .filter(|m| m.installed)
            .count()
    } else {
        0
    };

    let message = if !exists {
        format!(
            "Models directory does not exist at {}. Create it or run 'sona-cli models download <MODEL>'.",
            resolved_dir.display()
        )
    } else if !writable {
        format!(
            "Models directory at {} is not writable.",
            resolved_dir.display()
        )
    } else if installed_count == 0 {
        format!(
            "Models directory at {} has no installed preset models.",
            resolved_dir.display()
        )
    } else {
        format!(
            "Models directory at {} is accessible with {installed_count} installed model(s).",
            resolved_dir.display()
        )
    };

    DoctorModelsStatus {
        path: resolved_dir.display().to_string(),
        exists,
        writable,
        installed_count,
        message,
    }
}

fn inspect_config(config_override: Option<&Path>) -> DoctorConfigStatus {
    let override_buf = config_override.map(|p| p.to_path_buf());
    let resolved_path = crate::init_config::resolve_config_path(override_buf.as_ref());

    match resolved_path {
        Some(path) => {
            let mut errors = Vec::new();
            if let Err(err) = sona_runtime_fs::load_transcribe_config_file(&path) {
                errors.push(format!("[transcribe]: {err}"));
            }
            if let Err(err) = sona_runtime_fs::load_transcribe_live_config_file(&path) {
                errors.push(format!("[transcribe_live]: {err}"));
            }
            if let Err(err) = sona_runtime_fs::load_serve_config_file(&path) {
                errors.push(format!("[serve]: {err}"));
            }

            if errors.is_empty() {
                DoctorConfigStatus {
                    path: Some(path.display().to_string()),
                    found: true,
                    valid: true,
                    message: format!("Valid configuration at {}", path.display()),
                }
            } else {
                DoctorConfigStatus {
                    path: Some(path.display().to_string()),
                    found: true,
                    valid: false,
                    message: format!("Configuration error in {}: {}", path.display(), errors.join("; ")),
                }
            }
        }
        None => DoctorConfigStatus {
            path: None,
            found: false,
            valid: true,
            message: "No config file found (defaults will be used; create one with 'sona-cli config init').".to_string(),
        },
    }
}

fn ffmpeg_install_suggestion() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "Install via 'winget install Gyan.FFmpeg' or 'scoop install ffmpeg', or pass --ffmpeg-path <PATH>"
    }
    #[cfg(target_os = "macos")]
    {
        "Install via 'brew install ffmpeg', or pass --ffmpeg-path <PATH>"
    }
    #[cfg(target_os = "linux")]
    {
        "Install via 'sudo apt install ffmpeg' or 'sudo dnf install ffmpeg', or pass --ffmpeg-path <PATH>"
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "Install FFmpeg from https://ffmpeg.org/download.html and add it to PATH, or pass --ffmpeg-path <PATH>"
    }
}
