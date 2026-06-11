use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::tempdir;

fn cli_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sona"));
    command.env("SONA_FORCE_CLI", "1");
    command
}

#[test]
fn help_is_printed_to_stdout() {
    let output = cli_command().arg("--help").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sona"));
    assert!(stdout.contains("transcribe"));
    assert!(stdout.contains("models"));
    assert!(stdout.contains("sona models list --type whisper --language zh"));
    assert!(stdout.contains("sona models delete sherpa-onnx-whisper-turbo"));
}

#[test]
fn help_shows_distinct_version_and_verbose_flags() {
    let output = cli_command().arg("--help").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("-V, --version"));
    assert!(stdout.contains("-v, --verbose"));
}

#[test]
fn version_is_printed_to_stdout() {
    let output = cli_command().arg("--version").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("sona {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(&expected));
}

#[test]
fn short_help_is_printed_to_stdout() {
    let output = cli_command().arg("-h").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sona"));
    assert!(stdout.contains("transcribe"));
}

#[test]
fn short_version_is_printed_to_stdout() {
    let output = cli_command().arg("-V").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected = format!("sona {}", env!("CARGO_PKG_VERSION"));
    assert!(stdout.contains(&expected));
}

#[test]
fn short_verbose_does_not_print_version() {
    let output = cli_command()
        .args(["-v", "models", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    let expected_version = format!("sona {}", env!("CARGO_PKG_VERSION"));
    assert!(!stdout.contains(&expected_version));
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
}

#[test]
fn transcribe_help_mentions_key_examples() {
    let output = cli_command()
        .args(["transcribe", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--model-id"));
    assert!(stdout.contains("--vad-model-id"));
    assert!(stdout.contains("--gpu-acceleration"));
    assert!(stdout.contains("sample.srt"));
    assert!(stdout.contains("Offline preset model id to use for transcription"));
}

#[test]
fn serve_help_mentions_runtime_defaults() {
    let output = cli_command().args(["serve", "--help"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--config"));
    assert!(stdout.contains("--gpu-acceleration"));
    assert!(stdout.contains("--vad-model-id"));
    assert!(stdout.contains("--punctuation-model-id"));
    assert!(stdout.contains("--max-concurrent"));
    assert!(stdout.contains("--max-queue-size"));
    assert!(stdout.contains("--max-upload-size-mb"));
    assert!(stdout.contains("--job-ttl-minutes"));
}

#[test]
fn serve_invalid_gpu_acceleration_returns_failure() {
    let output = cli_command()
        .args(["serve", "--gpu-acceleration", "vulkan"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gpu_acceleration"));
    assert!(stderr.contains("auto, cpu, cuda, coreml, directml"));
}

#[test]
fn models_help_mentions_list_download_and_delete() {
    let output = cli_command().args(["models", "--help"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("list"));
    assert!(stdout.contains("download"));
    assert!(stdout.contains("delete"));
}

#[test]
fn models_list_help_mentions_new_filters() {
    let output = cli_command()
        .args(["models", "list", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--type"));
    assert!(stdout.contains("--language"));
    assert!(stdout.contains("--installed"));
    assert!(stdout.contains("sona models list --mode offline --type whisper"));
}

#[test]
fn models_download_help_mentions_companions() {
    let output = cli_command()
        .args(["models", "download", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("silero-vad"));
    assert!(stdout.contains("--models-dir"));
}

#[test]
fn models_delete_help_mentions_confirmation() {
    let output = cli_command()
        .args(["models", "delete", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("<MODEL_ID>"));
    assert!(stdout.contains("--models-dir"));
    assert!(stdout.contains("--yes"));
}

#[test]
fn model_list_outputs_json() {
    let output = cli_command().args(["models", "list"]).output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("silero-vad"));
    assert!(stdout.contains("\"installed\""));
}

#[test]
fn model_list_can_filter_by_mode() {
    let output = cli_command()
        .args(["models", "list", "--mode", "streaming"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30"));
    assert!(!stdout.contains("sherpa-onnx-whisper-turbo"));
}

#[test]
fn model_list_can_filter_by_type() {
    let output = cli_command()
        .args(["models", "list", "--type", "whisper"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("sherpa-onnx-whisper-large-v3"));
    assert!(!stdout.contains("silero-vad"));
}

#[test]
fn model_list_can_filter_by_language() {
    let output = cli_command()
        .args(["models", "list", "--language", "yue"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17"));
    assert!(!stdout.contains("silero-vad"));
}

#[test]
fn model_list_can_filter_by_type_and_language() {
    let output = cli_command()
        .args(["models", "list", "--type", "whisper", "--language", "zh"])
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sherpa-onnx-whisper-turbo"));
    assert!(stdout.contains("sherpa-onnx-whisper-large-v3"));
    assert!(!stdout.contains("silero-vad"));
    assert!(!stdout.contains("sherpa-onnx-streaming-zipformer-zh-xlarge-int8-2025-06-30"));
}

#[test]
fn model_list_can_filter_installed_only() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(&models_dir).unwrap();
    fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("list")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--installed")
        .output()
        .unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("silero-vad"));
    assert!(!stdout.contains("sherpa-onnx-whisper-turbo"));
}

#[test]
fn missing_config_file_returns_failure() {
    let output = cli_command()
        .args([
            "transcribe",
            "sample.wav",
            "--config",
            "does-not-exist.toml",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to read config file"));
}

#[test]
fn unknown_model_id_returns_failure() {
    let output = cli_command()
        .args([
            "transcribe",
            "sample.wav",
            "--models-dir",
            "C:\\models",
            "--model-id",
            "not-a-real-model",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
}

#[test]
fn invalid_gpu_acceleration_returns_failure_before_model_resolution() {
    let output = cli_command()
        .args([
            "transcribe",
            "sample.wav",
            "--gpu-acceleration",
            "vulkan",
            "--model-id",
            "not-a-real-model",
        ])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("gpu_acceleration"));
    assert!(stderr.contains("auto, cpu, cuda, coreml, directml"));
    assert!(!stderr.contains("Unknown model id"));
}

#[test]
fn unknown_download_model_id_returns_failure() {
    let output = cli_command()
        .args(["models", "download", "not-a-real-model", "--quiet"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
}

#[test]
fn unknown_delete_model_id_returns_failure() {
    let output = cli_command()
        .args(["models", "delete", "not-a-real-model", "--yes"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown model id"));
}

#[test]
fn delete_installed_archive_model_with_yes_removes_only_target() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_path = models_dir.join("sherpa-onnx-whisper-turbo");
    let vad_path = models_dir.join("silero_vad.onnx");
    fs::create_dir_all(&model_path).unwrap();
    fs::write(model_path.join("model.onnx"), "").unwrap();
    fs::write(&vad_path, "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("delete")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!model_path.exists());
    assert!(vad_path.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Deleted sherpa-onnx-whisper-turbo"));
}

#[test]
fn delete_installed_single_file_model_with_yes_removes_file() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(&models_dir).unwrap();
    let vad_path = models_dir.join("silero_vad.onnx");
    fs::write(&vad_path, "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("delete")
        .arg("silero-vad")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    assert!(!vad_path.exists());
}

#[test]
fn delete_known_missing_model_succeeds_with_notice() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let output = cli_command()
        .arg("models")
        .arg("delete")
        .arg("silero-vad")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--yes")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Model silero-vad is not installed"));
}

#[test]
fn delete_interactive_confirmation_decline_leaves_model() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_path = models_dir.join("sherpa-onnx-whisper-turbo");
    fs::create_dir_all(&model_path).unwrap();

    let mut child = cli_command()
        .arg("models")
        .arg("delete")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"n\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(model_path.exists());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Delete cancelled"));
}

#[test]
fn delete_interactive_confirmation_accepts_y() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    let model_path = models_dir.join("sherpa-onnx-whisper-turbo");
    fs::create_dir_all(&model_path).unwrap();

    let mut child = cli_command()
        .arg("models")
        .arg("delete")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.as_mut().unwrap().write_all(b"y\n").unwrap();
    let output = child.wait_with_output().unwrap();

    assert!(output.status.success());
    assert!(!model_path.exists());
}

#[test]
fn repeated_download_of_installed_model_also_installs_required_vad() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();
    fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();

    let output = cli_command()
        .arg("models")
        .arg("download")
        .arg("sherpa-onnx-whisper-turbo")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--quiet")
        .output()
        .unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Installed sherpa-onnx-whisper-turbo"));
    assert!(stderr.contains("Installed silero-vad"));
}

#[test]
fn missing_required_vad_model_returns_failure() {
    let dir = tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();

    let output = cli_command()
        .arg("transcribe")
        .arg("sample.wav")
        .arg("--models-dir")
        .arg(&models_dir)
        .arg("--model-id")
        .arg("sherpa-onnx-whisper-turbo")
        .output()
        .unwrap();

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("requires a VAD model"));
}
