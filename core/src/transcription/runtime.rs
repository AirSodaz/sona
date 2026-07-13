use crate::export::ExportFormat;
use crate::models::paths::ModelsDirStatus;
use crate::models::preset_models::{
    DEFAULT_PUNCTUATION_MODEL_ID, DEFAULT_SILERO_VAD_MODEL_ID, PresetModel, find_preset_model,
};
use crate::runtime::config::{TranscribeConfigSection, TranscribeLiveConfigSection};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_BATCH_JOBS: usize = 1;
pub const DEFAULT_THREADS: i32 = 4;
pub const DEFAULT_LANGUAGE: &str = "auto";
pub const DEFAULT_VAD_BUFFER_SIZE: f32 = 5.0;
const SUPPORTED_BATCH_MEDIA_EXTENSIONS: &[&str] = &[
    "wav", "mp3", "m4a", "aiff", "flac", "ogg", "wma", "aac", "opus", "amr", "mp4", "webm", "mov",
    "mkv", "avi", "wmv", "flv", "3gp",
];
const GLOB_PATTERN_CHARS: &[char] = &['*', '?', '['];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OutputTarget {
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOutputPlan {
    pub input_path: PathBuf,
    pub output_path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchInputSource {
    pub inputs: Vec<PathBuf>,
    pub base_dir: PathBuf,
    pub preserve_relative_paths: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchTranscribeOptions {
    pub input: PathBuf,
    pub output: Option<PathBuf>,
    pub format: Option<String>,
    pub language: Option<String>,
    pub model_id: Option<String>,
    pub models_dir: Option<PathBuf>,
    pub default_models_dir: Option<PathBuf>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub hotwords: Option<String>,
    pub gpu_acceleration: Option<String>,
    pub vad_buffer: Option<f32>,
    pub save_wav: Option<PathBuf>,
    pub quiet: bool,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BatchTranscribePlan {
    pub input_path: PathBuf,
    pub save_to_path: Option<PathBuf>,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub model_type: String,
    pub file_config: Option<crate::models::config::ModelFileConfig>,
    pub hotwords: Option<String>,
    pub gpu_acceleration: Option<String>,
    pub export_format: ExportFormat,
    pub output_target: OutputTarget,
    pub quiet: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveTranscribeOptions {
    pub output: Option<PathBuf>,
    pub format: Option<String>,
    pub model_id: Option<String>,
    pub models_dir: Option<PathBuf>,
    pub default_models_dir: Option<PathBuf>,
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
    pub threads: Option<i32>,
    pub enable_itn: Option<bool>,
    pub language: Option<String>,
    pub hotwords: Option<String>,
    pub gpu_acceleration: Option<String>,
    pub vad_buffer: Option<f32>,
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveTranscribePlan {
    pub model_id: String,
    pub model_path: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub punctuation_model: Option<String>,
    pub vad_model: Option<String>,
    pub vad_buffer: f32,
    pub model_type: String,
    pub file_config: Option<crate::models::config::ModelFileConfig>,
    pub hotwords: Option<String>,
    pub gpu_acceleration: Option<String>,
    pub export_format: Option<ExportFormat>,
    pub output_path: Option<PathBuf>,
}

impl LiveTranscribePlan {
    pub fn to_local_streaming_request(
        &self,
        instance_id: impl Into<String>,
    ) -> crate::ports::asr::LocalSherpaStreamingRequest {
        crate::ports::asr::LocalSherpaStreamingRequest {
            instance_id: instance_id.into(),
            model_path: self.model_path.clone(),
            num_threads: self.num_threads,
            enable_itn: self.enable_itn,
            language: self.language.clone(),
            punctuation_model: self.punctuation_model.clone(),
            vad_model: self.vad_model.clone(),
            vad_buffer: self.vad_buffer,
            model_type: self.model_type.clone(),
            file_config: self.file_config.clone(),
            hotwords: self.hotwords.clone(),
            normalization_options: Default::default(),
            postprocess_options: Default::default(),
            gpu_acceleration: self.gpu_acceleration.clone(),
        }
    }
}

pub fn resolve_export_format(
    format: Option<&str>,
    output: Option<&Path>,
) -> Result<ExportFormat, String> {
    if let Some(value) = format {
        return ExportFormat::parse(value);
    }

    match output {
        Some(path) => ExportFormat::from_output_path(path),
        None => Ok(ExportFormat::Json),
    }
}

pub fn resolve_output_target(output: Option<PathBuf>) -> OutputTarget {
    match output {
        Some(path) => OutputTarget::File(path),
        None => OutputTarget::Stdout,
    }
}

pub fn resolve_batch_jobs(value: Option<usize>) -> Result<usize, String> {
    let jobs = value.unwrap_or(DEFAULT_BATCH_JOBS);
    if jobs == 0 {
        Err("--jobs must be greater than 0.".to_string())
    } else {
        Ok(jobs)
    }
}

pub fn should_run_path_batch(inputs: &[PathBuf]) -> bool {
    inputs.len() > 1
        || inputs
            .iter()
            .any(|input| path_contains_glob_pattern(input.as_path()))
}

pub fn plan_batch_output_files(
    inputs: &[PathBuf],
    input_dir: &Path,
    output_dir: &Path,
    format: ExportFormat,
    preserve_relative_paths: bool,
    force: bool,
) -> Result<Vec<BatchOutputPlan>, String> {
    let extension = export_format_name(format);
    let mut seen_outputs = HashSet::new();
    let mut plans = Vec::with_capacity(inputs.len());

    for input_path in inputs {
        let relative_output =
            batch_relative_output_path(input_path, input_dir, extension, preserve_relative_paths)?;
        let output_path = output_dir.join(relative_output);
        let output_key = output_path.to_string_lossy().to_ascii_lowercase();
        if !seen_outputs.insert(output_key) {
            return Err(format!(
                "Batch output path {} would overwrite another result. Use --recursive to preserve directories or remove duplicate input stems.",
                output_path.display()
            ));
        }
        let _ = force;

        plans.push(BatchOutputPlan {
            input_path: input_path.clone(),
            output_path,
        });
    }

    Ok(plans)
}

fn path_contains_glob_pattern(path: &Path) -> bool {
    path.to_string_lossy()
        .chars()
        .any(|character| GLOB_PATTERN_CHARS.contains(&character))
}

pub fn common_input_parent(inputs: &[PathBuf]) -> Result<PathBuf, String> {
    if inputs.is_empty() {
        return Err("Missing input file path.".to_string());
    }

    let mut common = match inputs[0].parent() {
        Some(parent) => parent
            .components()
            .filter(|component| !matches!(component, Component::CurDir))
            .collect::<PathBuf>(),
        None => return Err("Input path has no parent directory.".to_string()),
    };

    for path in &inputs[1..] {
        let parent = path
            .parent()
            .ok_or_else(|| "Input path has no parent directory.".to_string())?;

        let mut new_common = PathBuf::new();
        let mut common_components = common.components();
        let mut parent_components = parent
            .components()
            .filter(|component| !matches!(component, Component::CurDir));

        while let (Some(left), Some(right)) = (common_components.next(), parent_components.next()) {
            if left == right {
                new_common.push(left);
            } else {
                break;
            }
        }
        common = new_common;
    }

    Ok(common)
}

fn export_format_name(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Json => "json",
        ExportFormat::Txt => "txt",
        ExportFormat::Srt => "srt",
        ExportFormat::Vtt => "vtt",
        ExportFormat::Md => "md",
    }
}

pub fn is_supported_batch_media_path(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            let normalized = extension.trim_start_matches('.').to_ascii_lowercase();
            SUPPORTED_BATCH_MEDIA_EXTENSIONS.contains(&normalized.as_str())
        })
        .unwrap_or(false)
}

fn batch_relative_output_path(
    input_path: &Path,
    input_dir: &Path,
    extension: &str,
    preserve_relative_paths: bool,
) -> Result<PathBuf, String> {
    if preserve_relative_paths {
        let relative = input_path.strip_prefix(input_dir).map_err(|_| {
            format!(
                "Input file {} is not inside --input-dir {}.",
                input_path.display(),
                input_dir.display()
            )
        })?;
        let mut output = relative.to_path_buf();
        output.set_extension(extension);
        return Ok(output);
    }

    let stem = input_path.file_stem().ok_or_else(|| {
        format!(
            "Unable to derive output file name from {}.",
            input_path.display()
        )
    })?;
    let mut output = PathBuf::from(stem);
    output.set_extension(extension);
    Ok(output)
}

pub fn resolve_batch_transcribe_plan_with_install_checker(
    options: BatchTranscribeOptions,
    config: Option<TranscribeConfigSection>,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<BatchTranscribePlan, String> {
    resolve_batch_transcribe_plan_with_install_checker_and_models_dir_status(
        options,
        config,
        is_installed,
        |_| ModelsDirStatus::Missing,
    )
}

pub fn resolve_live_transcribe_plan_with_install_checker(
    options: LiveTranscribeOptions,
    config: Option<TranscribeLiveConfigSection>,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<LiveTranscribePlan, String> {
    resolve_live_transcribe_plan_with_install_checker_and_models_dir_status(
        options,
        config,
        is_installed,
        |_| ModelsDirStatus::Missing,
    )
}

pub fn resolve_live_transcribe_plan_with_install_checker_and_models_dir_status(
    options: LiveTranscribeOptions,
    config: Option<TranscribeLiveConfigSection>,
    is_installed: fn(&PresetModel, &Path) -> bool,
    models_dir_status: fn(&Path) -> ModelsDirStatus,
) -> Result<LiveTranscribePlan, String> {
    let config = config.unwrap_or_default();
    if options.output.is_none() && options.format.is_some() {
        return Err("--format requires --output for live transcription.".to_string());
    }
    let export_format = options
        .output
        .as_deref()
        .map(|path| resolve_export_format(options.format.as_deref(), Some(path)))
        .transpose()?;
    let resolved = resolve_local_transcribe_settings(
        LocalTranscribeSettings {
            model_id: options.model_id.or(config.model_id),
            models_dir: options.models_dir.or(config.models_dir),
            default_models_dir: options.default_models_dir,
            vad_model_id: options.vad_model_id.or(config.vad_model_id),
            punctuation_model_id: options.punctuation_model_id.or(config.punctuation_model_id),
            threads: options.threads.or(config.threads),
            enable_itn: options.enable_itn.or(config.enable_itn),
            language: options.language.or(config.language),
            hotwords: options.hotwords.or(config.hotwords),
            gpu_acceleration: options.gpu_acceleration.or(config.gpu_acceleration),
            vad_buffer: options.vad_buffer.or(config.vad_buffer_size),
        },
        "streaming",
        "Missing required streaming model. Pass --model-id or set model_id in --config.",
        is_installed,
        models_dir_status,
    )?;
    let _ = options.force;
    Ok(LiveTranscribePlan {
        model_id: resolved.model_id,
        model_path: resolved.model_path,
        num_threads: resolved.num_threads,
        enable_itn: resolved.enable_itn,
        language: resolved.language,
        punctuation_model: resolved.punctuation_model,
        vad_model: resolved.vad_model,
        vad_buffer: resolved.vad_buffer,
        model_type: resolved.model_type,
        file_config: resolved.file_config,
        hotwords: resolved.hotwords,
        gpu_acceleration: resolved.gpu_acceleration,
        export_format,
        output_path: options.output,
    })
}

pub fn resolve_batch_transcribe_plan_with_install_checker_and_models_dir_status(
    options: BatchTranscribeOptions,
    config: Option<TranscribeConfigSection>,
    is_installed: fn(&PresetModel, &Path) -> bool,
    models_dir_status: fn(&Path) -> ModelsDirStatus,
) -> Result<BatchTranscribePlan, String> {
    let config = config.unwrap_or_default();
    let output_target = resolve_output_target(options.output.clone());
    let export_format = resolve_export_format(
        options.format.as_deref().or(config.format.as_deref()),
        match &output_target {
            OutputTarget::Stdout => None,
            OutputTarget::File(path) => Some(path.as_path()),
        },
    )?;
    let resolved = resolve_local_transcribe_settings(
        LocalTranscribeSettings {
            model_id: options.model_id.or(config.model_id),
            models_dir: options.models_dir.or(config.models_dir),
            default_models_dir: options.default_models_dir,
            vad_model_id: options.vad_model_id.or(config.vad_model_id),
            punctuation_model_id: options.punctuation_model_id.or(config.punctuation_model_id),
            threads: options.threads.or(config.threads),
            enable_itn: options.enable_itn.or(config.enable_itn),
            language: options.language.or(config.language),
            hotwords: options.hotwords.or(config.hotwords),
            gpu_acceleration: options.gpu_acceleration.or(config.gpu_acceleration),
            vad_buffer: options.vad_buffer.or(config.vad_buffer_size),
        },
        "batch",
        "Missing required batch model. Pass --model-id or set model_id in --config.",
        is_installed,
        models_dir_status,
    )?;

    Ok(BatchTranscribePlan {
        input_path: options.input,
        save_to_path: options.save_wav,
        model_path: resolved.model_path,
        num_threads: resolved.num_threads,
        enable_itn: resolved.enable_itn,
        language: resolved.language,
        punctuation_model: resolved.punctuation_model,
        vad_model: resolved.vad_model,
        vad_buffer: resolved.vad_buffer,
        model_type: resolved.model_type,
        file_config: resolved.file_config,
        hotwords: resolved.hotwords,
        gpu_acceleration: resolved.gpu_acceleration,
        export_format,
        output_target,
        quiet: options.quiet || config.quiet.unwrap_or(false),
    })
}

struct LocalTranscribeSettings {
    model_id: Option<String>,
    models_dir: Option<PathBuf>,
    default_models_dir: Option<PathBuf>,
    vad_model_id: Option<String>,
    punctuation_model_id: Option<String>,
    threads: Option<i32>,
    enable_itn: Option<bool>,
    language: Option<String>,
    hotwords: Option<String>,
    gpu_acceleration: Option<String>,
    vad_buffer: Option<f32>,
}

struct ResolvedLocalTranscribeSettings {
    model_id: String,
    model_path: String,
    num_threads: i32,
    enable_itn: bool,
    language: String,
    punctuation_model: Option<String>,
    vad_model: Option<String>,
    vad_buffer: f32,
    model_type: String,
    file_config: Option<crate::models::config::ModelFileConfig>,
    hotwords: Option<String>,
    gpu_acceleration: Option<String>,
}

fn resolve_local_transcribe_settings(
    settings: LocalTranscribeSettings,
    mode: &'static str,
    missing_model_error: &'static str,
    is_installed: fn(&PresetModel, &Path) -> bool,
    models_dir_status: fn(&Path) -> ModelsDirStatus,
) -> Result<ResolvedLocalTranscribeSettings, String> {
    let gpu_acceleration =
        crate::runtime::gpu::resolve_gpu_acceleration(settings.gpu_acceleration)?;
    let model_id = settings
        .model_id
        .ok_or_else(|| missing_model_error.to_string())?;
    let model = resolve_model_for_mode(&model_id, mode)?;
    let models_dir = crate::models::paths::resolve_models_dir(
        settings.models_dir,
        settings.default_models_dir,
        models_dir_status,
    )?;
    let threads = settings.threads.unwrap_or(DEFAULT_THREADS);
    if threads <= 0 {
        return Err("threads must be greater than 0".to_string());
    }
    let vad_buffer = settings.vad_buffer.unwrap_or(DEFAULT_VAD_BUFFER_SIZE);
    if vad_buffer <= 0.0 {
        return Err("vad_buffer must be greater than 0".to_string());
    }
    let rules = model.resolved_rules();
    let model_path = require_installed_model(model, &models_dir, is_installed)?;
    let vad_model = if rules.requires_vad {
        let companion_id = settings
            .vad_model_id
            .unwrap_or_else(|| DEFAULT_SILERO_VAD_MODEL_ID.to_string());
        Some(require_installed_companion(
            &companion_id,
            &models_dir,
            is_installed,
        )?)
    } else {
        optional_installed_companion(settings.vad_model_id.as_deref(), &models_dir, is_installed)?
    };
    let punctuation_model = if rules.requires_punctuation {
        let companion_id = settings
            .punctuation_model_id
            .unwrap_or_else(|| DEFAULT_PUNCTUATION_MODEL_ID.to_string());
        Some(require_installed_companion(
            &companion_id,
            &models_dir,
            is_installed,
        )?)
    } else {
        optional_installed_companion(
            settings.punctuation_model_id.as_deref(),
            &models_dir,
            is_installed,
        )?
    };
    Ok(ResolvedLocalTranscribeSettings {
        model_id,
        model_path,
        num_threads: threads,
        enable_itn: settings.enable_itn.unwrap_or(false),
        language: settings
            .language
            .unwrap_or_else(|| DEFAULT_LANGUAGE.to_string()),
        punctuation_model,
        vad_model,
        vad_buffer,
        model_type: model.model_type.clone(),
        file_config: model.file_config.clone(),
        hotwords: settings.hotwords,
        gpu_acceleration,
    })
}

fn resolve_model_for_mode(
    model_id: &str,
    mode: &'static str,
) -> Result<&'static PresetModel, String> {
    let model =
        find_preset_model(model_id).ok_or_else(|| format!("Unknown model id: {model_id}"))?;
    if !model.supports_mode(mode) {
        return Err(format!(
            "Model '{model_id}' does not support {mode} transcription."
        ));
    }
    Ok(model)
}

fn require_installed_model(
    model: &PresetModel,
    models_dir: &Path,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<String, String> {
    let path = model.resolve_install_path(models_dir);
    if !is_installed(model, models_dir) {
        return Err(format!(
            "Model '{}' was not found at {}. Pass --models-dir explicitly if your desktop models live elsewhere.",
            model.id,
            path.display()
        ));
    }
    Ok(path.to_string_lossy().to_string())
}

fn require_installed_companion(
    model_id: &str,
    models_dir: &Path,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<String, String> {
    let model = find_preset_model(model_id)
        .ok_or_else(|| format!("Unknown companion model id: {model_id}"))?;
    let path = model.resolve_install_path(models_dir);
    if !is_installed(model, models_dir) {
        return Err(format!(
            "Companion model '{model_id}' was not found at {}. Pass --models-dir explicitly if your desktop models live elsewhere.",
            path.display()
        ));
    }
    Ok(path.to_string_lossy().to_string())
}

fn optional_installed_companion(
    model_id: Option<&str>,
    models_dir: &Path,
    is_installed: fn(&PresetModel, &Path) -> bool,
) -> Result<Option<String>, String> {
    model_id
        .map(|id| require_installed_companion(id, models_dir, is_installed))
        .transpose()
}
