use std::fs;
use std::path::PathBuf;

use sona_core::export::ExportFormat;
use sona_core::transcribe_runtime::{
    BatchInputSource, DEFAULT_BATCH_JOBS, OutputTarget, load_transcribe_config_file,
    plan_batch_output_files, resolve_batch_input_source, resolve_batch_jobs, resolve_export_format,
    resolve_output_target, should_run_path_batch,
};
use tempfile::tempdir;

#[test]
fn load_transcribe_config_file_reads_shared_and_section_values() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("sona-cli.toml");
    fs::write(
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
fn export_format_and_output_target_follow_cli_defaults() {
    assert_eq!(
        resolve_export_format(None, None).unwrap(),
        ExportFormat::Json
    );
    assert_eq!(
        resolve_export_format(None, Some(PathBuf::from("out.srt").as_path())).unwrap(),
        ExportFormat::Srt
    );
    assert_eq!(
        resolve_export_format(Some("json"), Some(PathBuf::from("out.txt").as_path())).unwrap(),
        ExportFormat::Json
    );

    assert_eq!(resolve_output_target(None), OutputTarget::Stdout);
    assert_eq!(
        resolve_output_target(Some(PathBuf::from("out.json"))),
        OutputTarget::File(PathBuf::from("out.json"))
    );
}

#[test]
fn batch_jobs_default_to_one_and_reject_zero() {
    assert_eq!(resolve_batch_jobs(None).unwrap(), DEFAULT_BATCH_JOBS);
    assert_eq!(resolve_batch_jobs(Some(2)).unwrap(), 2);
    assert!(resolve_batch_jobs(Some(0)).unwrap_err().contains("--jobs"));
}

#[test]
fn should_run_path_batch_detects_multiple_inputs_and_globs() {
    assert!(!should_run_path_batch(&[PathBuf::from("sample.wav")]));
    assert!(should_run_path_batch(&[
        PathBuf::from("sample.wav"),
        PathBuf::from("second.wav")
    ]));
    assert!(should_run_path_batch(&[PathBuf::from("*.wav")]));
}

#[test]
fn resolve_batch_input_source_expands_globs_and_finds_common_parent() {
    let dir = tempdir().unwrap();
    let a = dir.path().join("a.wav");
    let b = dir.path().join("b.wav");
    fs::write(&a, "").unwrap();
    fs::write(&b, "").unwrap();

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
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("input");
    fs::create_dir_all(input_dir.join("nested")).unwrap();
    let top = input_dir.join("meeting.wav");
    let nested = input_dir.join("nested").join("call.mp3");
    fs::write(&top, "").unwrap();
    fs::write(&nested, "").unwrap();

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
fn batch_output_plans_preserve_relative_paths_when_recursive() {
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir_all(input_dir.join("nested")).unwrap();
    let meeting = input_dir.join("meeting.wav");
    let call = input_dir.join("nested").join("call.mp3");
    fs::write(&meeting, "").unwrap();
    fs::write(&call, "").unwrap();

    let plans = plan_batch_output_files(
        &[meeting, call],
        &input_dir,
        &output_dir,
        ExportFormat::Srt,
        true,
        false,
    )
    .unwrap();

    let outputs = plans
        .iter()
        .map(|plan| {
            plan.output_path
                .strip_prefix(&output_dir)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();

    assert_eq!(outputs, vec!["meeting.srt", "nested/call.srt"]);
}

#[test]
fn batch_output_plans_reject_duplicate_outputs_without_recursive_structure() {
    let dir = tempdir().unwrap();
    let input_dir = dir.path().join("input");
    let output_dir = dir.path().join("output");
    fs::create_dir_all(&input_dir).unwrap();
    let wav = input_dir.join("demo.wav");
    let mp4 = input_dir.join("demo.mp4");
    fs::write(&wav, "").unwrap();
    fs::write(&mp4, "").unwrap();

    let error = plan_batch_output_files(
        &[wav, mp4],
        &input_dir,
        &output_dir,
        ExportFormat::Json,
        false,
        false,
    )
    .unwrap_err();

    assert!(error.contains("would overwrite"));
    assert!(error.contains("demo.json"));
}
