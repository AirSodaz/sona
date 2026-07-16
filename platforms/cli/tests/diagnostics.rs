use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sona_core::models::preset_models::find_preset_model;
use sona_core::runtime::diagnostics::DiagnosticsCoreSnapshot;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const LIVE_MODEL_ID: &str = "sherpa-onnx-streaming-paraformer-trilingual-zh-cantonese-en";
const BATCH_MODEL_ID: &str = "sherpa-onnx-whisper-turbo";

fn write_input(path: &Path, models_dir: &Path) {
    write_input_with_permission(path, models_dir, "granted");
}

fn write_input_with_permission(path: &Path, models_dir: &Path, permission_state: &str) {
    let live_path = find_preset_model(LIVE_MODEL_ID)
        .unwrap()
        .resolve_install_path(models_dir);
    let batch_path = find_preset_model(BATCH_MODEL_ID)
        .unwrap()
        .resolve_install_path(models_dir);
    fs::write(
        path,
        serde_json::to_vec_pretty(&json!({
            "config": {
                "streamingModelPath": live_path,
                "batchModelPath": batch_path,
                "vadModelPath": "",
                "punctuationModelPath": "",
                "microphoneId": "cli-default"
            },
            "permissionState": permission_state,
            "microphoneProbe": {"options": [], "available": true, "errorMessage": null},
            "systemAudioProbe": {"options": [], "available": false, "errorMessage": "unsupported"},
            "voiceTypingReadiness": {"state": "cli-ready", "lastErrorMessage": null},
            "runtimeEnvironment": {
                "ffmpegPath": "cli://ffmpeg",
                "ffmpegExists": false,
                "logDirPath": "cli://logs"
            }
        }))
        .unwrap(),
    )
    .unwrap();
}

fn run_snapshot(app_data_dir: &str, input: &str, json: bool) -> sona_cli::CliOutput {
    let mut args = vec![
        "sona-cli",
        "diagnostics",
        "snapshot",
        "--app-data-dir",
        app_data_dir,
        "--input",
        input,
    ];
    if json {
        args.push("--json");
    }
    sona_cli::run_cli_from_args(args).unwrap()
}

fn file_hashes(root: &Path) -> BTreeMap<PathBuf, String> {
    fn visit(root: &Path, current: &Path, files: &mut BTreeMap<PathBuf, String>) {
        let mut entries = fs::read_dir(current)
            .unwrap()
            .map(|entry| entry.unwrap())
            .collect::<Vec<_>>();
        entries.sort_by_key(|entry| entry.file_name());
        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.insert(
                    path.strip_prefix(root).unwrap().to_path_buf(),
                    hex::encode(Sha256::digest(fs::read(&path).unwrap())),
                );
            }
        }
    }

    let mut files = BTreeMap::new();
    visit(root, root, &mut files);
    files
}

#[test]
fn diagnostics_snapshot_requires_app_data_and_input() {
    for args in [
        vec![
            "sona-cli",
            "diagnostics",
            "snapshot",
            "--input",
            "input.json",
        ],
        vec![
            "sona-cli",
            "diagnostics",
            "snapshot",
            "--app-data-dir",
            "app-data",
        ],
    ] {
        let error = sona_cli::run_cli_from_args(args).unwrap_err();
        assert!(matches!(error, sona_cli::CliError::Usage(_)));
        assert_eq!(error.exit_code(), 2);
    }
}

#[test]
fn diagnostics_snapshot_missing_input_does_not_create_app_data() {
    let root = tempfile::tempdir().unwrap();
    let missing_app_data = root.path().join("missing-app-data");
    let missing_input = root.path().join("missing-input.json");

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "diagnostics",
        "snapshot",
        "--app-data-dir",
        missing_app_data.to_string_lossy().as_ref(),
        "--input",
        missing_input.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Io(_)));
    assert!(!missing_app_data.exists());
}

#[test]
fn diagnostics_snapshot_rejects_invalid_json_before_creating_models_dir() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    let input = root.path().join("invalid.json");
    fs::write(&input, b"{").unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "diagnostics",
        "snapshot",
        "--app-data-dir",
        app_data_dir.to_string_lossy().as_ref(),
        "--input",
        input.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Validation(_)));
    assert!(!app_data_dir.exists());
}

#[test]
fn diagnostics_snapshot_classifies_non_utf8_json_as_validation_error() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    let input = root.path().join("invalid-utf8.json");
    fs::write(&input, [0xff_u8, 0xfe]).unwrap();

    let error = sona_cli::run_cli_from_args([
        "sona-cli",
        "diagnostics",
        "snapshot",
        "--app-data-dir",
        app_data_dir.to_string_lossy().as_ref(),
        "--input",
        input.to_string_lossy().as_ref(),
    ])
    .unwrap_err();

    assert!(matches!(error, sona_cli::CliError::Validation(_)));
    assert!(!app_data_dir.exists());
}

#[test]
fn diagnostics_snapshot_renders_exact_table_columns() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    let models_dir = app_data_dir.join("models");
    fs::create_dir_all(&models_dir).unwrap();
    let input = root.path().join("input.json");
    write_input(&input, &models_dir);

    let output = run_snapshot(
        app_data_dir.to_string_lossy().as_ref(),
        input.to_string_lossy().as_ref(),
        false,
    );
    let lines = output.stdout.lines().collect::<Vec<_>>();

    assert_eq!(output.stderr, "");
    assert_eq!(lines.len(), 3);
    assert_eq!(
        lines[0].split_whitespace().collect::<Vec<_>>(),
        [
            "SCANNED",
            "LIVE_MODEL",
            "BATCH_MODEL",
            "ONBOARDING",
            "PUNCTUATION",
            "PERMISSION",
            "MIC",
            "SYSTEM_AUDIO"
        ]
    );
    let values = lines[2].split_whitespace().collect::<Vec<_>>();
    assert_eq!(values.len(), 8);
    assert_eq!(values[1], LIVE_MODEL_ID);
    assert_eq!(values[2], BATCH_MODEL_ID);
    assert_eq!(&values[3..], ["true", "true", "granted", "true", "false"]);
}

#[test]
fn diagnostics_snapshot_sanitizes_permission_state_for_terminal_table() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    let models_dir = app_data_dir.join("models");
    fs::create_dir_all(&models_dir).unwrap();
    let input = root.path().join("input.json");
    write_input_with_permission(&input, &models_dir, "granted\n\u{1b}[31m");

    let output = run_snapshot(
        app_data_dir.to_string_lossy().as_ref(),
        input.to_string_lossy().as_ref(),
        false,
    );

    assert_eq!(output.stdout.lines().count(), 3);
    assert!(output.stdout.contains(r"granted\n\u{1b}[31m"));
    assert!(!output.stdout.contains('\u{1b}'));
}

#[test]
fn diagnostics_snapshot_outputs_complete_pretty_json() {
    let root = tempfile::tempdir().unwrap();
    let app_data_dir = root.path().join("app-data");
    let models_dir = app_data_dir.join("models");
    fs::create_dir_all(&models_dir).unwrap();
    let input = root.path().join("input.json");
    write_input(&input, &models_dir);

    let output = run_snapshot(
        app_data_dir.to_string_lossy().as_ref(),
        input.to_string_lossy().as_ref(),
        true,
    );
    let typed: DiagnosticsCoreSnapshot = serde_json::from_str(&output.stdout).unwrap();
    let snapshot = serde_json::to_value(typed).unwrap();

    assert!(output.stdout.starts_with("{\n"));
    assert!(output.stdout.contains("\n  \"selectedModels\""));
    assert_eq!(snapshot["selectedModels"]["live"]["id"], LIVE_MODEL_ID);
    assert_eq!(snapshot["modelRules"]["live"]["requiresPunctuation"], true);
    assert_eq!(snapshot["runtimeEnvironment"]["ffmpegPath"], "cli://ffmpeg");
    assert_eq!(snapshot["voiceTypingReadiness"]["state"], "cli-ready");
}

#[test]
fn diagnostics_snapshot_accepts_relative_unicode_paths_without_source_changes() {
    let current = std::env::current_dir().unwrap();
    let parent = tempfile::tempdir_in(&current).unwrap();
    let app_data_dir = parent.path().join("诊断-CLI-🌍");
    let models_dir = app_data_dir.join("models");
    fs::create_dir_all(&models_dir).unwrap();
    fs::write(app_data_dir.join("sentinel.txt"), b"unchanged").unwrap();
    let input = parent.path().join("输入-诊断.json");
    write_input(&input, &models_dir);
    let relative_app = app_data_dir.strip_prefix(&current).unwrap();
    let relative_input = input.strip_prefix(&current).unwrap();
    let before = file_hashes(&app_data_dir);

    let output = run_snapshot(
        relative_app.to_string_lossy().as_ref(),
        relative_input.to_string_lossy().as_ref(),
        true,
    );
    let snapshot: Value = serde_json::from_str(&output.stdout).unwrap();

    assert_eq!(snapshot["selectedModels"]["batch"]["id"], BATCH_MODEL_ID);
    assert_eq!(file_hashes(&app_data_dir), before);
}
