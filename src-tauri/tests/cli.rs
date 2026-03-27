use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn cli_command() -> Command {
    Command::new(env!("CARGO_BIN_EXE_sona-cli"))
}

#[test]
fn help_is_printed_to_stdout() {
    let output = cli_command().arg("--help").output().unwrap();
    assert!(output.status.success());

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("sona-cli"));
    assert!(stdout.contains("transcribe"));
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
