use std::fs;
use std::path::PathBuf;

use sona_core::export::ExportFormat;
use sona_core::preset_models::PresetModel;
use sona_core::preset_models::{DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID};
use sona_core::runtime_config::TranscribeConfigSection;
use sona_core::transcribe_runtime::{
    BatchTranscribeOptions, DEFAULT_BATCH_JOBS, DEFAULT_LANGUAGE, DEFAULT_THREADS,
    DEFAULT_VAD_BUFFER_SIZE, OutputTarget, plan_batch_output_files, resolve_batch_jobs,
    resolve_batch_transcribe_plan_with_install_checker, resolve_export_format,
    resolve_output_target, should_run_path_batch,
};
use tempfile::tempdir;

#[test]
fn parse_transcribe_config_file_reads_shared_and_section_values() {
    let config = sona_core::runtime_config::parse_transcribe_config_file(
        r#"
models_dir = "/shared/models"
gpu_acceleration = "cuda"
model_id = "legacy-model"

[transcribe]
language = "ja"
"#,
        "sona-cli.toml",
    )
    .unwrap();

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

fn temp_transcribe_options() -> BatchTranscribeOptions {
    BatchTranscribeOptions {
        input: PathBuf::from("sample.wav"),
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
        quiet: false,
        force: false,
    }
}

fn test_model_exists(model: &PresetModel, models_dir: &std::path::Path) -> bool {
    model.resolve_install_path(models_dir).exists()
}

fn installed_whisper_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("sample.wav");
    let models_dir = dir.path().join("models");
    fs::write(&input_path, "").unwrap();
    fs::create_dir_all(models_dir.join("sherpa-onnx-whisper-turbo")).unwrap();
    fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();
    (dir, input_path, models_dir)
}

fn installed_funasr_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempdir().unwrap();
    let input_path = dir.path().join("sample.wav");
    let models_dir = dir.path().join("models");
    fs::write(&input_path, "").unwrap();
    fs::create_dir_all(models_dir.join("sherpa-onnx-funasr-nano-int8-2025-12-30")).unwrap();
    fs::write(models_dir.join("silero_vad.onnx"), "").unwrap();
    fs::create_dir_all(
        models_dir.join("sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8"),
    )
    .unwrap();
    (dir, input_path, models_dir)
}

#[test]
fn batch_plan_cli_values_override_config_file_values() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir.clone());
    cli.vad_model_id = Some("silero-vad".to_string());
    cli.threads = Some(8);
    cli.enable_itn = Some(true);
    cli.hotwords = Some("cli-term".to_string());

    let resolved = resolve_batch_transcribe_plan_with_install_checker(
        cli,
        Some(TranscribeConfigSection {
            threads: Some(2),
            enable_itn: Some(false),
            hotwords: Some("config-term".to_string()),
            model_id: Some("ignored".to_string()),
            ..Default::default()
        }),
        test_model_exists,
    )
    .unwrap();

    assert_eq!(resolved.num_threads, 8);
    assert!(resolved.enable_itn);
    assert_eq!(resolved.hotwords.as_deref(), Some("cli-term"));
    assert_eq!(
        resolved.model_path,
        models_dir
            .join("sherpa-onnx-whisper-turbo")
            .to_string_lossy()
            .to_string()
    );
}

#[test]
fn batch_plan_defaults_gpu_language_threads_and_vad_buffer() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir);
    cli.vad_model_id = Some("silero-vad".to_string());

    let resolved =
        resolve_batch_transcribe_plan_with_install_checker(cli, None, test_model_exists).unwrap();

    assert_eq!(resolved.gpu_acceleration.as_deref(), Some("auto"));
    assert_eq!(resolved.language, DEFAULT_LANGUAGE);
    assert_eq!(resolved.num_threads, DEFAULT_THREADS);
    assert_eq!(resolved.vad_buffer, DEFAULT_VAD_BUFFER_SIZE);
}

#[test]
fn batch_plan_defaults_required_companions_when_omitted() {
    let (_dir, input_path, models_dir) = installed_funasr_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-funasr-nano-int8-2025-12-30".to_string());
    cli.models_dir = Some(models_dir.clone());

    let resolved =
        resolve_batch_transcribe_plan_with_install_checker(cli, None, test_model_exists).unwrap();

    assert_eq!(
        resolved.vad_model.as_deref(),
        Some(
            models_dir
                .join("silero_vad.onnx")
                .to_string_lossy()
                .as_ref()
        )
    );
    assert_eq!(
        resolved.punctuation_model.as_deref(),
        Some(
            models_dir
                .join("sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8")
                .to_string_lossy()
                .as_ref()
        )
    );
}

#[test]
fn batch_plan_config_can_override_required_companion_default() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir);

    let error = resolve_batch_transcribe_plan_with_install_checker(
        cli,
        Some(TranscribeConfigSection {
            vad_model_id: Some("custom-vad".to_string()),
            ..Default::default()
        }),
        test_model_exists,
    )
    .unwrap_err();

    assert!(error.contains("Unknown companion model id: custom-vad"));
    assert!(!error.contains(DEFAULT_SILERO_VAD_MODEL_ID));
}

#[test]
fn batch_plan_config_gpu_and_hotwords_are_used_when_cli_omits_them() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir);
    cli.vad_model_id = Some("silero-vad".to_string());

    let resolved = resolve_batch_transcribe_plan_with_install_checker(
        cli,
        Some(TranscribeConfigSection {
            gpu_acceleration: Some("cpu".to_string()),
            hotwords: Some("config-hotword".to_string()),
            ..Default::default()
        }),
        test_model_exists,
    )
    .unwrap();

    assert_eq!(resolved.gpu_acceleration.as_deref(), Some("cpu"));
    assert_eq!(resolved.hotwords.as_deref(), Some("config-hotword"));
}

#[test]
fn batch_plan_cli_gpu_overrides_config_file() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir);
    cli.vad_model_id = Some("silero-vad".to_string());
    cli.gpu_acceleration = Some("cuda".to_string());

    let resolved = resolve_batch_transcribe_plan_with_install_checker(
        cli,
        Some(TranscribeConfigSection {
            gpu_acceleration: Some("cpu".to_string()),
            ..Default::default()
        }),
        test_model_exists,
    )
    .unwrap();

    assert_eq!(resolved.gpu_acceleration.as_deref(), Some("cuda"));
}

#[test]
fn batch_plan_invalid_gpu_fails_before_model_resolution() {
    let mut cli = temp_transcribe_options();
    cli.model_id = Some("not-a-real-model".to_string());
    cli.gpu_acceleration = Some("vulkan".to_string());

    let error = resolve_batch_transcribe_plan_with_install_checker(cli, None, test_model_exists)
        .unwrap_err();

    assert!(error.contains("gpu_acceleration"));
    assert!(error.contains("auto, cpu, cuda, coreml, directml"));
    assert!(!error.contains("Unknown model id"));
}

#[test]
fn batch_plan_infers_export_format_from_output_path() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.output = Some(PathBuf::from("out.srt"));
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir);
    cli.vad_model_id = Some("silero-vad".to_string());

    let resolved =
        resolve_batch_transcribe_plan_with_install_checker(cli, None, test_model_exists).unwrap();
    assert_eq!(resolved.export_format, ExportFormat::Srt);
}

#[test]
fn batch_plan_format_flag_overrides_output_extension() {
    let (_dir, input_path, models_dir) = installed_whisper_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.output = Some(PathBuf::from("out.txt"));
    cli.format = Some("json".to_string());
    cli.model_id = Some("sherpa-onnx-whisper-turbo".to_string());
    cli.models_dir = Some(models_dir);
    cli.vad_model_id = Some("silero-vad".to_string());

    let resolved =
        resolve_batch_transcribe_plan_with_install_checker(cli, None, test_model_exists).unwrap();
    assert_eq!(resolved.export_format, ExportFormat::Json);
}

#[test]
fn batch_plan_defaults_required_punctuation_constant_when_needed() {
    let (_dir, input_path, models_dir) = installed_funasr_fixture();

    let mut cli = temp_transcribe_options();
    cli.input = input_path;
    cli.model_id = Some("sherpa-onnx-funasr-nano-int8-2025-12-30".to_string());
    cli.models_dir = Some(models_dir);

    let resolved =
        resolve_batch_transcribe_plan_with_install_checker(cli, None, test_model_exists).unwrap();
    assert!(
        resolved
            .punctuation_model
            .as_deref()
            .unwrap()
            .contains(DEFAULT_PUNCTUATION_MODEL_ID)
    );
}
