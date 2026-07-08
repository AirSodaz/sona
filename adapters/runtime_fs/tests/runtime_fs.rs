use std::path::PathBuf;

use sona_core::export::ExportFormat;
use sona_core::ports::fs::FileSystem;
use sona_core::preset_models::{DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model};
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider};
use sona_core::runtime::RuntimePathKind;
use sona_core::transcribe_runtime::BatchInputSource;
use sona_runtime_fs::{
    FsSourcePathStatusProvider, RealFileSystem, is_preset_model_installed_at,
    load_transcribe_config_file, plan_batch_output_files, resolve_batch_input_source,
    resolve_runtime_path_status, select_desktop_models_dir_from_app_roots,
    write_json_pretty_atomic,
};

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
