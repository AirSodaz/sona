use crate::cli_runtime::TranscribeConfigSection;
use crate::export::ExportFormat;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

pub const DEFAULT_BATCH_JOBS: usize = 1;
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

pub fn load_transcribe_config_file(path: &Path) -> Result<TranscribeConfigSection, String> {
    let contents = std::fs::read_to_string(path)
        .map_err(|error| format!("Failed to read config file {}: {error}", path.display()))?;
    let unified: crate::cli_runtime::UnifiedConfigFile = toml::from_str(&contents)
        .map_err(|error| format!("Failed to parse config file {}: {error}", path.display()))?;
    Ok(unified.into_transcribe_config())
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

pub fn resolve_batch_input_source(
    input_dir: Option<&Path>,
    inputs: &[PathBuf],
    recursive: bool,
) -> Result<BatchInputSource, String> {
    if let Some(input_dir) = input_dir {
        return Ok(BatchInputSource {
            inputs: collect_batch_input_files(input_dir, recursive)?,
            base_dir: input_dir.to_path_buf(),
            preserve_relative_paths: recursive,
        });
    }

    let inputs = expand_input_patterns(inputs)?;
    let base_dir = common_input_parent(&inputs)?;
    Ok(BatchInputSource {
        inputs,
        base_dir,
        preserve_relative_paths: false,
    })
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
        if !force && output_path.exists() {
            return Err(format!(
                "Output file already exists: {}. Use --force to overwrite.",
                output_path.display()
            ));
        }

        plans.push(BatchOutputPlan {
            input_path: input_path.clone(),
            output_path,
        });
    }

    Ok(plans)
}

fn expand_input_patterns(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, String> {
    let mut expanded = Vec::new();

    for input in inputs {
        if path_contains_glob_pattern(input) {
            let pattern = input.to_string_lossy().to_string();
            let mut matches = glob::glob(&pattern)
                .map_err(|error| format!("Invalid glob pattern {}: {error}", input.display()))?
                .map(|entry| {
                    entry.map_err(|error| {
                        format!("Failed to read glob match for {}: {error}", input.display())
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            matches.retain(|path| path.is_file());
            matches.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
            if matches.is_empty() {
                return Err(format!(
                    "No input files matched glob pattern: {}",
                    input.display()
                ));
            }
            expanded.extend(matches);
        } else {
            ensure_input_file_exists(input)?;
            expanded.push(input.clone());
        }
    }

    expanded.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    expanded.dedup_by(|left, right| {
        left.to_string_lossy()
            .eq_ignore_ascii_case(&right.to_string_lossy())
    });
    Ok(expanded)
}

fn path_contains_glob_pattern(path: &Path) -> bool {
    path.to_string_lossy()
        .chars()
        .any(|character| GLOB_PATTERN_CHARS.contains(&character))
}

fn common_input_parent(inputs: &[PathBuf]) -> Result<PathBuf, String> {
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

fn ensure_input_file_exists(input: &Path) -> Result<(), String> {
    if input.is_file() {
        Ok(())
    } else {
        Err(format!(
            "Input file must be an existing file: {}",
            input.display()
        ))
    }
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

fn collect_batch_input_files(input_dir: &Path, recursive: bool) -> Result<Vec<PathBuf>, String> {
    if !input_dir.is_dir() {
        return Err(format!(
            "--input-dir must be an existing directory: {}",
            input_dir.display()
        ));
    }

    let walker = walkdir::WalkDir::new(input_dir)
        .min_depth(1)
        .max_depth(if recursive { usize::MAX } else { 1 });
    let mut files = Vec::new();

    for entry in walker {
        let entry =
            entry.map_err(|error| format!("Failed to read {}: {error}", input_dir.display()))?;
        if entry.file_type().is_file() && is_supported_batch_media_path(entry.path()) {
            files.push(entry.path().to_path_buf());
        }
    }

    files.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    Ok(files)
}

fn is_supported_batch_media_path(path: &Path) -> bool {
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
