use std::path::PathBuf;

use sona_core::automation::{AutomationRuntimePathCollectionOutcome, AutomationRuntimeRuleConfig};
use sona_core::export::ExportFormat;
use sona_core::models::preset_models::{DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model};
use sona_core::ports::fs::FileSystem;
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider};
use sona_core::runtime::environment::RuntimePathKind;
use sona_core::transcription::runtime::BatchInputSource;
use sona_runtime_fs::{
    FsSourcePathStatusProvider, RealFileSystem, cli_shared_library_directory_candidates,
    collect_automation_runtime_candidate_paths, ensure_directory_exists,
    is_preset_model_installed_at, load_legacy_settings_app_config, load_transcribe_config_file,
    path_exists, plan_batch_output_files, remove_path_if_exists, resolve_batch_input_source,
    resolve_runtime_path_status, select_desktop_models_dir_from_app_roots,
    tauri_shared_library_directory_candidates, write_cli_config_template_file,
    write_json_pretty_atomic, write_transcript_output_file,
};

#[test]
fn path_exists_reports_existing_files_and_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("model.onnx");

    assert!(!path_exists(&file_path).unwrap());

    std::fs::write(&file_path, "model").unwrap();

    assert!(path_exists(&file_path).unwrap());
}

#[test]
fn write_cli_config_template_file_creates_parent_and_respects_force() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("nested").join("sona-cli.toml");

    write_cli_config_template_file(&config_path, "first", false).unwrap();
    let error = write_cli_config_template_file(&config_path, "second", false).unwrap_err();

    assert!(error.contains("--force"));
    assert_eq!(std::fs::read_to_string(&config_path).unwrap(), "first");

    write_cli_config_template_file(&config_path, "second", true).unwrap();

    assert_eq!(std::fs::read_to_string(config_path).unwrap(), "second");
}

#[test]
fn write_transcript_output_file_creates_parent_directory_and_writes_contents() {
    let dir = tempfile::tempdir().unwrap();
    let output_path = dir.path().join("nested").join("transcript.srt");

    write_transcript_output_file(&output_path, "hello transcript").unwrap();

    assert_eq!(
        std::fs::read_to_string(output_path).unwrap(),
        "hello transcript"
    );
}

#[test]
fn load_transcribe_config_file_reads_shared_and_section_values() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    std::fs::write(
        &config_path,
        r#"
models_dir = "/shared/models"
gpu_acceleration = "cuda"
model_id = "legacy-model"

[transcribe]
language = "ja"
"#,
    )
    .unwrap();

    let config = load_transcribe_config_file(&config_path).unwrap();

    assert_eq!(config.models_dir, Some(PathBuf::from("/shared/models")));
    assert_eq!(config.gpu_acceleration.as_deref(), Some("cuda"));
    assert_eq!(config.model_id.as_deref(), Some("legacy-model"));
    assert_eq!(config.language.as_deref(), Some("ja"));
}

#[test]
fn load_legacy_settings_app_config_unwraps_object_payloads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.json"),
        r#"{"sona-config":{"asr":{"providers":{"online":{"volcengine":{"apiKey":"legacy-key"}}}}}}"#,
    )
    .unwrap();

    let config = load_legacy_settings_app_config(dir.path())
        .unwrap()
        .unwrap();

    assert_eq!(
        config["asr"]["providers"]["online"]["volcengine"]["apiKey"],
        "legacy-key"
    );
    assert!(config.get("sona-config").is_none());
}

#[test]
fn load_legacy_settings_app_config_returns_none_for_missing_file() {
    let dir = tempfile::tempdir().unwrap();

    let config = load_legacy_settings_app_config(dir.path()).unwrap();

    assert!(config.is_none());
}

#[test]
fn resolve_batch_input_source_expands_globs_and_finds_common_parent() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    std::fs::write(&a, "").unwrap();
    std::fs::write(&b, "").unwrap();

    let source = resolve_batch_input_source(None, &[dir.path().join("*.wav")], false).unwrap();

    assert_eq!(
        source,
        BatchInputSource {
            inputs: vec![a, b],
            base_dir: dir.path().to_path_buf(),
            preserve_relative_paths: false,
        }
    );
}

#[test]
fn resolve_batch_input_source_collects_recursive_directory_inputs() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("input");
    std::fs::create_dir_all(input_dir.join("nested")).unwrap();
    let top = input_dir.join("meeting.wav");
    let nested = input_dir.join("nested").join("call.mp3");
    std::fs::write(&top, "").unwrap();
    std::fs::write(&nested, "").unwrap();

    let source = resolve_batch_input_source(Some(&input_dir), &[], true).unwrap();

    assert_eq!(
        source,
        BatchInputSource {
            inputs: vec![top, nested],
            base_dir: input_dir,
            preserve_relative_paths: true,
        }
    );
}

#[test]
fn batch_output_plans_reject_existing_outputs_without_force() {
    let dir = tempfile::tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    std::fs::create_dir_all(&input_dir).unwrap();
    std::fs::create_dir_all(&output_dir).unwrap();
    let meeting = input_dir.join("meeting.wav");
    std::fs::write(&meeting, "").unwrap();
    std::fs::write(output_dir.join("meeting.srt"), "").unwrap();

    let error = plan_batch_output_files(
        &[meeting],
        &input_dir,
        &output_dir,
        ExportFormat::Srt,
        false,
        false,
    )
    .unwrap_err();

    assert!(error.contains("Output file already exists"));
    assert!(error.contains("meeting.srt"));
}

#[test]
fn runtime_path_status_detects_file_directory_and_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("sample.txt");
    std::fs::write(&file_path, b"sample").unwrap();
    let missing_path = dir.path().join("missing.txt");

    let file_status = resolve_runtime_path_status(file_path.to_string_lossy().as_ref());
    let directory_status = resolve_runtime_path_status(dir.path().to_string_lossy().as_ref());
    let missing_status = resolve_runtime_path_status(missing_path.to_string_lossy().as_ref());

    assert_eq!(file_status.kind, RuntimePathKind::File);
    assert_eq!(file_status.error, None);
    assert_eq!(directory_status.kind, RuntimePathKind::Directory);
    assert_eq!(directory_status.error, None);
    assert_eq!(missing_status.kind, RuntimePathKind::Missing);
    assert_eq!(missing_status.error, None);
}

#[test]
fn ensure_directory_exists_creates_nested_directory() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("logs").join("current");

    ensure_directory_exists(&nested).unwrap();
    ensure_directory_exists(&nested).unwrap();

    assert!(nested.is_dir());
}

#[test]
fn fs_source_path_status_provider_reports_missing_directories_as_not_resumable() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("audio.wav");
    std::fs::write(&file_path, b"sample").unwrap();

    let provider = FsSourcePathStatusProvider;

    assert_eq!(
        provider.status_for_path(file_path.to_string_lossy().as_ref()),
        SourcePathStatus::File
    );
    assert_eq!(
        provider.status_for_path(dir.path().to_string_lossy().as_ref()),
        SourcePathStatus::Directory
    );
    assert_eq!(
        provider.status_for_path(dir.path().join("missing.wav").to_string_lossy().as_ref()),
        SourcePathStatus::Missing
    );
}

#[test]
fn automation_runtime_metadata_reports_file_snapshot_and_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("audio.wav");
    std::fs::write(&file_path, b"sample").unwrap();

    let metadata =
        sona_runtime_fs::automation_runtime_path_metadata(file_path.to_string_lossy().as_ref())
            .unwrap()
            .unwrap();
    let missing = sona_runtime_fs::automation_runtime_path_metadata(
        dir.path().join("missing.wav").to_string_lossy().as_ref(),
    )
    .unwrap();

    assert!(metadata.is_file);
    assert_eq!(metadata.size, 6);
    assert!(metadata.mtime_ms > 0);
    assert_eq!(missing, None);
}

#[test]
fn automation_runtime_candidate_paths_skip_excluded_directory() {
    let dir = tempfile::tempdir().unwrap();
    let watch_dir = dir.path().join("watch");
    let export_dir = watch_dir.join("exports");
    std::fs::create_dir_all(&export_dir).unwrap();
    let candidate = watch_dir.join("meeting.wav");
    std::fs::write(&candidate, b"one").unwrap();
    std::fs::write(export_dir.join("skip.wav"), b"two").unwrap();
    std::fs::write(watch_dir.join("notes.txt"), b"three").unwrap();

    let rule = AutomationRuntimeRuleConfig {
        rule_id: "rule-1".to_string(),
        watch_directory: watch_dir.to_string_lossy().into_owned(),
        recursive: true,
        exclude_directory: export_dir.to_string_lossy().into_owned(),
        debounce_ms: 5,
        stable_window_ms: 10,
    };

    let paths = collect_automation_runtime_candidate_paths(&rule).unwrap();
    let result = sona_core::automation::collect_runtime_rule_path_result(
        &rule,
        paths[0].as_str(),
        sona_runtime_fs::automation_runtime_path_metadata(paths[0].as_str()),
    );

    assert_eq!(paths, vec![candidate.to_string_lossy().into_owned()]);
    assert_eq!(
        result.outcome,
        AutomationRuntimePathCollectionOutcome::Candidate
    );
}

#[test]
fn preset_model_install_status_uses_filesystem_shape() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path();
    let model = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    let install_path = model.resolve_install_path(models_dir);

    std::fs::write(&install_path, []).unwrap();
    assert!(!is_preset_model_installed_at(model, models_dir));

    std::fs::write(&install_path, b"model bytes").unwrap();
    assert!(is_preset_model_installed_at(model, models_dir));

    std::fs::remove_file(&install_path).unwrap();
    std::fs::create_dir_all(&install_path).unwrap();
    assert!(!is_preset_model_installed_at(model, models_dir));
}

#[test]
fn selects_existing_models_dir_from_candidate_roots() {
    let dir = tempfile::tempdir().unwrap();
    let first_root = dir.path().join("Sona");
    let preferred_root = dir.path().join("com.asoda.sona");
    std::fs::create_dir_all(preferred_root.join("models")).unwrap();

    let result = select_desktop_models_dir_from_app_roots([first_root, preferred_root.clone()]);

    assert_eq!(result, Some(preferred_root.join("models")));
}

#[test]
fn real_file_system_supports_atomic_json_helpers() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested").join("data.json");

    write_json_pretty_atomic(&path, &serde_json::json!({"key": "value"})).unwrap();

    let fs = RealFileSystem;
    let contents = fs.read_to_string(path.as_path()).unwrap();
    assert!(contents.contains("\"key\""));
    assert!(fs.metadata(&path).unwrap().unwrap().is_file);
}

#[test]
fn real_file_system_atomic_json_helpers_overwrite_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.json");
    std::fs::write(&path, "{\"key\":\"old\"}").unwrap();

    write_json_pretty_atomic(&path, &serde_json::json!({"key": "new"})).unwrap();

    let contents = std::fs::read_to_string(&path).unwrap();
    assert!(contents.contains("\"new\""));
    assert!(!contents.contains("\"old\""));
}

#[test]
fn real_file_system_remove_path_if_exists_handles_files_and_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.bin");
    std::fs::write(&path, b"data").unwrap();

    remove_path_if_exists(&path).unwrap();
    assert!(!path.exists());

    remove_path_if_exists(&path).unwrap();
}

#[test]
fn cli_shared_library_candidates_match_standalone_resource_layouts() {
    let exe_dir = PathBuf::from("/opt/sona/bin");

    let candidates = cli_shared_library_directory_candidates(&exe_dir);

    assert_eq!(
        candidates,
        vec![
            exe_dir.join("../shared_libs"),
            exe_dir.join("shared_libs"),
            exe_dir.join("../resources/shared_libs"),
            exe_dir.join("resources/shared_libs"),
        ]
    );
}

#[test]
fn tauri_shared_library_candidates_match_desktop_bundle_layouts() {
    let exe_dir = PathBuf::from("/opt/sona/bin");

    let candidates = tauri_shared_library_directory_candidates(&exe_dir);

    assert_eq!(
        candidates,
        vec![
            exe_dir.join("resources").join("shared_libs"),
            exe_dir.join("..").join("resources").join("shared_libs"),
            exe_dir
                .join("..")
                .join("..")
                .join("resources")
                .join("shared_libs"),
            exe_dir
                .join("..")
                .join("..")
                .join("..")
                .join("resources")
                .join("shared_libs"),
        ]
    );
}
