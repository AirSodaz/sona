use sona_sherpa_onnx::audio::LiveWavRecorder;

pub enum RecorderCommand {
    Start {
        owner: String,
        path: String,
        completed: tokio::sync::oneshot::Sender<Result<(), String>>,
    },
    Stop {
        owner: String,
        completed: tokio::sync::oneshot::Sender<String>,
    },
    SetPaused {
        owner: String,
        paused: bool,
    },
}

pub struct CaptureWriterState {
    pub writer: LiveWavRecorder,
    pub path: String,
    pub paused: bool,
}

pub fn resolve_recording_output_path<F>(
    output_path: Option<String>,
    fallback: F,
) -> Result<String, String>
where
    F: FnOnce() -> Result<String, String>,
{
    let path = match output_path {
        Some(path) => path,
        None => fallback()?,
    };

    let path_buf = std::path::Path::new(&path);
    if path_buf
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "Invalid recording output path containing parent directory traversal: {path}"
        ));
    }
    if path_buf
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|s| s.to_ascii_lowercase())
        != Some("wav".to_string())
    {
        return Err(format!(
            "Invalid recording output path: must have a .wav extension: {path}"
        ));
    }

    if let Some(parent) = std::path::Path::new(&path).parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create recording directory {}: {e}",
                parent.display()
            )
        })?;
    }

    Ok(path)
}

pub async fn queue_recording_start(
    recorder_tx: Option<&tokio::sync::mpsc::Sender<RecorderCommand>>,
    should_record: bool,
    capture_label: &str,
    instance_id: &str,
    output_path: Option<String>,
    fallback_path: impl FnOnce() -> Result<String, String>,
) -> Result<(), String> {
    if !should_record {
        return Ok(());
    }

    let Some(tx) = recorder_tx else {
        return Err(format!(
            "{} recorder is unavailable while starting capture",
            capture_label
        ));
    };

    let wav_filepath = resolve_recording_output_path(output_path, fallback_path)?;
    let (completed, completion) = tokio::sync::oneshot::channel();
    tx.try_send(RecorderCommand::Start {
        owner: instance_id.to_string(),
        path: wav_filepath,
        completed,
    })
    .map_err(|err| format!("Failed to queue {} recorder start: {}", capture_label, err))?;

    tokio::time::timeout(std::time::Duration::from_secs(5), completion)
        .await
        .map_err(|_| format!("{} recorder start acknowledgement timed out", capture_label))?
        .map_err(|_| format!("{} recorder task dropped response channel", capture_label))?
}
