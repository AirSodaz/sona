use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sona_core::automation::service::{AutomationFileSystem, AutomationIdGenerator};
use sona_core::automation::{
    AutomationRule, AutomationRuntimePathCollectionOutcome, AutomationRuntimeRuleConfig,
};
use sona_core::export::ExportFormat;
use sona_core::models::preset_models::{DEFAULT_SILERO_VAD_MODEL_ID, find_preset_model};
use sona_core::ports::fs::{FileSystem, FileSystemError, FileSystemOperation};
use sona_core::ports::runtime::{BatchTranscribePlanResolver, ModelCatalogProvider};
use sona_core::ports::time::UnixMillisClock;
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider};
use sona_core::runtime::diagnostics::{
    DiagnosticsConfigInput, DiagnosticsCoreInput, DiagnosticsEnrichmentRepository, DiagnosticsError,
};
use sona_core::runtime::environment::RuntimePathKind;
use sona_core::runtime::error::RuntimeConfigError;
use sona_core::tag::TagIdGenerator;
use sona_core::transcription::runtime::{
    BatchInputSource, BatchTranscribeOptions, LiveTranscribeOptions,
};
use sona_runtime_fs::{
    FsDiagnosticsEnrichmentRepository, FsSourcePathStatusProvider, NativeAutomationFileSystem,
    RealFileSystem, RuntimeBatchTranscribePlanResolver, RuntimeFsError,
    RuntimeModelCatalogProvider, SystemClock, UuidGenerator, build_diagnostics_snapshot,
    collect_automation_runtime_candidate_paths, ensure_directory_exists,
    is_preset_model_installed_at, load_legacy_settings_app_config, load_transcribe_config_file,
    load_transcribe_live_config_file, path_exists, plan_batch_output_files, remove_path_if_exists,
    resolve_batch_input_source, resolve_live_transcribe_plan_with_runtime_paths,
    resolve_runtime_path_status, select_desktop_models_dir_from_app_roots,
    validate_native_automation_rule_activation, write_cli_config_template_file,
    write_json_pretty_atomic, write_transcript_output_file,
};
use uuid::{Uuid, Version};

#[test]
fn native_automation_file_system_probes_and_creates_directories() {
    let dir = tempfile::tempdir().unwrap();
    let nested = dir.path().join("exports").join("nested");
    let regular_file = dir.path().join("existing.json");
    std::fs::write(&regular_file, b"{}").unwrap();
    let fs = NativeAutomationFileSystem;
    assert!(!AutomationFileSystem::path_exists(&fs, nested.to_string_lossy().as_ref()).unwrap());
    AutomationFileSystem::create_dir_all(&fs, nested.to_string_lossy().as_ref()).unwrap();
    assert!(AutomationFileSystem::path_exists(&fs, nested.to_string_lossy().as_ref()).unwrap());
    assert!(
        AutomationFileSystem::path_exists(&fs, regular_file.to_string_lossy().as_ref()).unwrap()
    );
    let error = AutomationFileSystem::create_dir_all(&fs, regular_file.to_string_lossy().as_ref())
        .unwrap_err();
    assert_eq!(error.operation, FileSystemOperation::CreateDirectory);
    assert_eq!(error.path, regular_file);
}

#[test]
fn runtime_capability_model_catalog_provider_reports_installed_models() {
    let directory = tempfile::tempdir().unwrap();
    let model = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    std::fs::write(model.resolve_install_path(directory.path()), b"model").unwrap();

    let snapshot = RuntimeModelCatalogProvider
        .build_model_catalog_snapshot(directory.path())
        .unwrap();

    assert!(
        snapshot
            .models
            .iter()
            .any(|entry| entry.id == DEFAULT_SILERO_VAD_MODEL_ID && entry.is_installed)
    );
}

#[test]
fn runtime_capability_batch_plan_resolver_preserves_resolution_errors() {
    let directory = tempfile::tempdir().unwrap();
    let missing_input = directory.path().join("missing.wav");

    let error = RuntimeBatchTranscribePlanResolver
        .resolve_batch_transcribe_plan(BatchTranscribeOptions {
            input: missing_input.clone(),
            output: None,
            format: None,
            language: None,
            model_id: None,
            models_dir: None,
            default_models_dir: None,
            vad_model_id: None,
            punctuation_model_id: None,
            threads: None,
            enable_itn: None,
            hotwords: None,
            gpu_acceleration: None,
            vad_buffer: None,
            save_wav: None,
            quiet: true,
            force: true,
        })
        .unwrap_err();

    assert!(
        error
            .to_string()
            .contains(&missing_input.to_string_lossy().to_string())
    );
}

#[test]
fn native_automation_validation_entrypoint_probes_paths_and_prepares_export_directory() {
    let dir = tempfile::tempdir().unwrap();
    let watch_directory = dir.path().join("watch");
    let export_directory = dir.path().join("export");
    let model_path = dir.path().join("model.onnx");
    std::fs::create_dir(&watch_directory).unwrap();
    std::fs::write(&model_path, b"model").unwrap();
    let rule: AutomationRule = serde_json::from_value(serde_json::json!({
        "name": "Rule",
        "saveHistory": true,
        "tagIds": [],
        "watchDirectory": watch_directory,
        "stageConfig": {},
        "exportConfig": {
            "directory": export_directory,
            "mode": "original"
        }
    }))
    .unwrap();

    let result = validate_native_automation_rule_activation(
        &rule,
        &serde_json::json!({"offlineModelPath": model_path}),
        &[],
    )
    .unwrap();

    assert!(result.valid, "unexpected validation result: {result:?}");
    assert!(export_directory.is_dir());
}

#[test]
fn uuid_generator_returns_distinct_uuid_v4_strings() {
    let generator: &dyn AutomationIdGenerator = &UuidGenerator;

    let first = generator.generate_id();
    let second = generator.generate_id();

    assert!(!first.is_empty());
    assert!(!second.is_empty());
    assert_ne!(first, second);
    assert_eq!(
        Uuid::parse_str(&first).unwrap().get_version(),
        Some(Version::Random)
    );
    assert_eq!(
        Uuid::parse_str(&second).unwrap().get_version(),
        Some(Version::Random)
    );
}

#[test]
fn uuid_generator_implements_tag_id_port() {
    let id = TagIdGenerator::generate_id(&UuidGenerator);
    assert_eq!(uuid::Uuid::parse_str(&id).unwrap().get_version_num(), 4);
}

#[test]
fn load_transcribe_live_config_file_reads_live_section() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("sona-cli.toml");
    std::fs::write(
        &path,
        r#"
models_dir = "shared-models"

[transcribe_live]
model_id = "live-model"
input = "stdin"
"#,
    )
    .unwrap();

    let config = load_transcribe_live_config_file(&path).unwrap();

    assert_eq!(config.models_dir, Some(PathBuf::from("shared-models")));
    assert_eq!(config.model_id.as_deref(), Some("live-model"));
    assert_eq!(config.input.as_deref(), Some("stdin"));
}

#[test]
fn live_plan_rejects_existing_output_before_model_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("live.srt");
    std::fs::write(&output, "existing").unwrap();

    let error = resolve_live_transcribe_plan_with_runtime_paths(
        LiveTranscribeOptions {
            output: Some(output.clone()),
            format: None,
            model_id: None,
            models_dir: None,
            default_models_dir: None,
            vad_model_id: None,
            punctuation_model_id: None,
            threads: None,
            enable_itn: None,
            language: None,
            hotwords: None,
            gpu_acceleration: None,
            vad_buffer: None,
            force: false,
        },
        None,
    )
    .unwrap_err();

    assert!(matches!(
        error,
        RuntimeFsError::AlreadyExists { path, .. } if path == output
    ));
}

fn accepts_clock(_: &dyn UnixMillisClock) {}

#[test]
fn system_clock_implements_shared_unix_millis_port() {
    accepts_clock(&SystemClock);
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let actual = UnixMillisClock::now_ms(&SystemClock).unwrap();
    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    assert!((before..=after).contains(&actual));
}

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

    assert!(matches!(
        error,
        RuntimeFsError::AlreadyExists { path, hint, .. }
            if path == config_path && hint.contains("--force")
    ));
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
fn load_transcribe_config_file_preserves_parse_category_and_source_path() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("invalid.toml");
    std::fs::write(&config_path, "[transcribe\nlanguage = 42").unwrap();

    let error = load_transcribe_config_file(&config_path).unwrap_err();

    assert!(matches!(
        error,
        RuntimeFsError::Config(RuntimeConfigError::Parse {
            source_label,
            reason,
        }) if source_label == config_path.display().to_string() && !reason.is_empty()
    ));
}

#[test]
fn load_transcribe_config_file_preserves_filesystem_operation_and_path() {
    let dir = tempfile::tempdir().unwrap();
    let config_path = dir.path().join("missing.toml");

    let error = load_transcribe_config_file(&config_path).unwrap_err();

    assert!(matches!(
        error,
        RuntimeFsError::FileSystem(FileSystemError {
            operation: FileSystemOperation::ReadText,
            path,
            ..
        }) if path == config_path
    ));
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

    assert!(matches!(
        error,
        RuntimeFsError::AlreadyExists { path, .. }
            if path.file_name().and_then(|name| name.to_str()) == Some("meeting.srt")
    ));
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
fn diagnostics_repository_collects_unicode_catalog_and_trimmed_path_statuses() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("模型-目录-🌍");
    std::fs::create_dir_all(&models_dir).unwrap();
    let vad_model = find_preset_model(DEFAULT_SILERO_VAD_MODEL_ID).unwrap();
    std::fs::write(vad_model.resolve_install_path(&models_dir), b"model bytes").unwrap();
    let live_dir = dir.path().join("实时-模型");
    std::fs::create_dir_all(&live_dir).unwrap();
    let vad_path = dir.path().join("vad-探针.onnx");
    std::fs::write(&vad_path, b"vad").unwrap();
    let missing_batch = dir.path().join("缺失-batch");
    let config = DiagnosticsConfigInput {
        streaming_model_path: format!("  {}  ", live_dir.to_string_lossy()),
        batch_model_path: missing_batch.to_string_lossy().into_owned(),
        vad_model_path: vad_path.to_string_lossy().into_owned(),
        punctuation_model_path: "   ".to_string(),
        microphone_id: "default".to_string(),
    };
    let repository = FsDiagnosticsEnrichmentRepository::new(models_dir.clone());

    let measurements = repository.collect_measurements(&config).unwrap();

    assert_eq!(
        measurements.model_catalog.models_dir,
        models_dir.to_string_lossy()
    );
    assert!(
        measurements
            .model_catalog
            .models
            .iter()
            .any(|model| model.id == DEFAULT_SILERO_VAD_MODEL_ID && model.is_installed)
    );
    assert_eq!(
        measurements.path_statuses.live_model.as_ref().unwrap().path,
        live_dir.to_string_lossy()
    );
    assert_eq!(
        measurements.path_statuses.live_model.as_ref().unwrap().kind,
        RuntimePathKind::Directory
    );
    assert_eq!(
        measurements
            .path_statuses
            .batch_model
            .as_ref()
            .unwrap()
            .kind,
        RuntimePathKind::Missing
    );
    assert_eq!(
        measurements.path_statuses.vad.as_ref().unwrap().kind,
        RuntimePathKind::File
    );
    assert!(measurements.path_statuses.punctuation.is_none());
}

#[test]
fn diagnostics_repository_creates_models_directory_and_maps_failures() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("new-models");
    let repository = FsDiagnosticsEnrichmentRepository::new(models_dir.clone());
    let config = DiagnosticsConfigInput::default();

    repository.collect_measurements(&config).unwrap();
    assert!(models_dir.is_dir());

    let blocked = dir.path().join("blocked-models");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let error = FsDiagnosticsEnrichmentRepository::new(blocked)
        .collect_measurements(&config)
        .unwrap_err();

    assert!(matches!(error, DiagnosticsError::Repository(_)));
}

#[test]
fn diagnostics_adapter_entrypoint_builds_snapshot_and_preserves_typed_errors() {
    let input = || -> DiagnosticsCoreInput {
        serde_json::from_value(serde_json::json!({
            "config": {
                "streamingModelPath": "",
                "batchModelPath": ""
            },
            "permissionState": "unknown",
            "microphoneProbe": {"options": [], "available": false, "errorMessage": null},
            "systemAudioProbe": {"options": [], "available": false, "errorMessage": null},
            "voiceTypingReadiness": {"state": "unknown", "lastErrorMessage": null}
        }))
        .unwrap()
    };
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let snapshot = build_diagnostics_snapshot(models_dir.clone(), input()).unwrap();

    assert!(models_dir.is_dir());
    assert!(snapshot.scanned_at.ends_with('Z'));

    let blocked = dir.path().join("blocked-models");
    std::fs::write(&blocked, b"not a directory").unwrap();
    let error = build_diagnostics_snapshot(blocked, input()).unwrap_err();

    assert!(matches!(error, DiagnosticsError::Repository(_)));
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
fn real_file_system_errors_preserve_operations_and_paths() {
    let dir = tempfile::tempdir().unwrap();
    let fs = RealFileSystem;
    let blocked_directory = dir.path().join("blocked-directory");
    std::fs::write(&blocked_directory, b"file").unwrap();
    let missing_file = dir.path().join("missing.bin");
    let rename_target = dir.path().join("renamed.bin");

    assert_file_system_error(
        fs.create_dir_all(&blocked_directory).unwrap_err(),
        FileSystemOperation::CreateDirectory,
        &blocked_directory,
        None,
    );
    assert_file_system_error(
        fs.write_file(dir.path(), b"contents").unwrap_err(),
        FileSystemOperation::WriteFile,
        dir.path(),
        None,
    );
    assert_file_system_error(
        fs.read_file(&missing_file).unwrap_err(),
        FileSystemOperation::ReadFile,
        &missing_file,
        None,
    );
    assert_file_system_error(
        fs.read_to_string(&missing_file).unwrap_err(),
        FileSystemOperation::ReadText,
        &missing_file,
        None,
    );
    assert_file_system_error(
        fs.rename(&missing_file, &rename_target).unwrap_err(),
        FileSystemOperation::Rename,
        &missing_file,
        Some(&rename_target),
    );
    assert_file_system_error(
        fs.remove_file(&missing_file).unwrap_err(),
        FileSystemOperation::RemoveFile,
        &missing_file,
        None,
    );
    assert_file_system_error(
        fs.remove_dir_all(&missing_file).unwrap_err(),
        FileSystemOperation::RemoveDirectory,
        &missing_file,
        None,
    );
    assert_file_system_error(
        fs.metadata(Path::new("\0invalid")).unwrap_err(),
        FileSystemOperation::Metadata,
        Path::new("\0invalid"),
        None,
    );
}

fn assert_file_system_error(
    error: FileSystemError,
    operation: FileSystemOperation,
    path: &Path,
    target: Option<&Path>,
) {
    assert_eq!(error.operation, operation);
    assert_eq!(error.path, path);
    assert_eq!(error.target.as_deref(), target);
    assert!(!error.reason.is_empty());
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
