#![allow(deprecated)]

mod dashboard_time;
mod diagnostics;
mod diagnostics_time;
mod storage_usage_time;

pub use dashboard_time::dashboard_snapshot_time_now;
pub use diagnostics::{FsDiagnosticsEnrichmentRepository, build_diagnostics_snapshot};
pub use diagnostics_time::diagnostics_scanned_at_now;
pub use storage_usage_time::storage_usage_generated_at_now;

use serde::Serialize;
use sona_core::automation::service::{
    AutomationFileSystem, AutomationIdGenerator, AutomationValidationService,
};
use sona_core::automation::{
    AutomationError, AutomationRule, AutomationRuleValidationResult, AutomationRuntimePathMetadata,
    AutomationRuntimeRuleConfig, should_consider_runtime_candidate_path,
};
use sona_core::export::ExportFormat;
use sona_core::history::HistoryIdGenerator;
use sona_core::models::catalog::ModelSummary;
use sona_core::models::paths::{ModelsDirStatus, status_of};
use sona_core::models::preset_models::{PresetModel, preset_models};
use sona_core::ports::fs::{FileMetadata, FileSystem, FileSystemError, FileSystemOperation};
use sona_core::ports::runtime::{
    BatchTranscribePlanResolver, ModelCatalogProvider, RuntimeCapabilityError,
};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::project::ProjectIdGenerator;
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider};
use sona_core::runtime::config::{
    ServeConfigSection, TranscribeConfigSection, TranscribeLiveConfigSection,
};
use sona_core::runtime::environment::{RuntimePathKind, RuntimePathStatus};
use sona_core::runtime::error::{RuntimeConfigError, RuntimeValidationError};
use sona_core::tag::TagIdGenerator;
use sona_core::transcription::runtime::{
    BatchInputSource, BatchOutputPlan, BatchTranscribeOptions, BatchTranscribePlan,
    LiveTranscribeOptions, LiveTranscribePlan,
};
use std::collections::HashSet;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const GLOB_PATTERN_CHARS: &[char] = &['*', '?', '['];

pub struct RealFileSystem;

pub struct NativeAutomationFileSystem;

pub struct UuidGenerator;

pub struct SystemClock;

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum RuntimeFsError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),

    #[error("Failed to serialize data for {path}: {reason}")]
    Serialization { path: PathBuf, reason: String },

    #[error(transparent)]
    Config(#[from] RuntimeConfigError),

    #[error(transparent)]
    Validation(#[from] RuntimeValidationError),

    #[error("{context} already exists: {path}. {hint}")]
    AlreadyExists {
        context: &'static str,
        path: PathBuf,
        hint: &'static str,
    },
}

impl RuntimeFsError {
    fn validation(subject: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Validation(RuntimeValidationError::new(subject, message))
    }
}

impl AutomationFileSystem for NativeAutomationFileSystem {
    fn path_exists(&self, path: &str) -> Result<bool, FileSystemError> {
        RealFileSystem
            .metadata(Path::new(path))
            .map(|metadata| metadata.is_some())
    }

    fn create_dir_all(&self, path: &str) -> Result<(), FileSystemError> {
        RealFileSystem.create_dir_all(Path::new(path))
    }
}

pub fn validate_native_automation_rule_activation(
    rule: &AutomationRule,
    global_config: &serde_json::Value,
    tags: &[serde_json::Value],
) -> Result<AutomationRuleValidationResult, AutomationError> {
    AutomationValidationService::new(&NativeAutomationFileSystem).validate_rule_activation(
        rule,
        global_config,
        tags,
    )
}

impl AutomationIdGenerator for UuidGenerator {
    fn generate_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}

impl HistoryIdGenerator for UuidGenerator {
    fn generate_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}

impl ProjectIdGenerator for UuidGenerator {
    fn generate_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}

impl TagIdGenerator for UuidGenerator {
    fn generate_id(&self) -> String {
        Uuid::new_v4().to_string()
    }
}

impl UnixMillisClock for SystemClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        unix_millis_from_system_time(SystemTime::now())
    }
}

fn unix_millis_from_system_time(time: SystemTime) -> Result<u64, ClockError> {
    let duration = time
        .duration_since(UNIX_EPOCH)
        .map_err(|error| ClockError::BeforeUnixEpoch(error.to_string()))?;
    unix_millis_from_duration(duration)
}

fn unix_millis_from_duration(duration: Duration) -> Result<u64, ClockError> {
    u64::try_from(duration.as_millis()).map_err(|error| ClockError::OutOfRange(error.to_string()))
}

#[cfg(test)]
mod clock_tests {
    use super::{unix_millis_from_duration, unix_millis_from_system_time};
    use sona_core::ports::time::ClockError;
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn system_time_conversion_classifies_pre_epoch_and_out_of_range_values() {
        let before_epoch = UNIX_EPOCH.checked_sub(Duration::from_millis(1)).unwrap();
        assert!(matches!(
            unix_millis_from_system_time(before_epoch),
            Err(ClockError::BeforeUnixEpoch(_))
        ));

        let too_large = Duration::from_millis(u64::MAX)
            .checked_add(Duration::from_millis(1))
            .unwrap();
        assert!(matches!(
            unix_millis_from_duration(too_large),
            Err(ClockError::OutOfRange(_))
        ));
    }
}

impl FileSystem for RealFileSystem {
    fn create_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::create_dir_all(path)
            .map_err(|error| file_system_error(FileSystemOperation::CreateDirectory, path, error))
    }

    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError> {
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent)?;
        }
        fs::write(path, contents)
            .map_err(|error| file_system_error(FileSystemOperation::WriteFile, path, error))
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        fs::read(path)
            .map_err(|error| file_system_error(FileSystemOperation::ReadFile, path, error))
    }

    fn read_to_string(&self, path: &Path) -> Result<String, FileSystemError> {
        fs::read_to_string(path)
            .map_err(|error| file_system_error(FileSystemOperation::ReadText, path, error))
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        fs::rename(from, to).map_err(|error| {
            FileSystemError::with_target(FileSystemOperation::Rename, from, to, error.to_string())
        })
    }

    fn remove_file(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_file(path)
            .map_err(|error| file_system_error(FileSystemOperation::RemoveFile, path, error))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_dir_all(path)
            .map_err(|error| file_system_error(FileSystemOperation::RemoveDirectory, path, error))
    }

    fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>, FileSystemError> {
        match fs::metadata(path) {
            Ok(metadata) => Ok(Some(FileMetadata {
                is_file: metadata.is_file(),
                is_dir: metadata.is_dir(),
            })),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(file_system_error(
                FileSystemOperation::Metadata,
                path,
                error,
            )),
        }
    }
}

fn file_system_error(
    operation: FileSystemOperation,
    path: &Path,
    error: std::io::Error,
) -> FileSystemError {
    FileSystemError::new(operation, path, error.to_string())
}

pub fn write_json_pretty_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), RuntimeFsError> {
    write_json_pretty_atomic_with(&RealFileSystem, path, value)
}

pub fn remove_path_if_exists(path: &Path) -> Result<(), FileSystemError> {
    remove_path_if_exists_with(&RealFileSystem, path)
}

pub fn ensure_directory_exists(path: &Path) -> Result<(), FileSystemError> {
    RealFileSystem.create_dir_all(path)
}

pub fn path_exists(path: &Path) -> Result<bool, FileSystemError> {
    RealFileSystem
        .metadata(path)
        .map(|metadata| metadata.is_some())
}

pub fn write_transcript_output_file(path: &Path, output: &str) -> Result<(), FileSystemError> {
    RealFileSystem.write_file(path, output.as_bytes())
}

pub fn write_cli_config_template_file(
    path: &Path,
    content: &str,
    force: bool,
) -> Result<(), RuntimeFsError> {
    let fs = RealFileSystem;

    if fs.metadata(path)?.is_some() && !force {
        return Err(RuntimeFsError::AlreadyExists {
            context: "Config file",
            path: path.to_path_buf(),
            hint: "Use --force to overwrite.",
        });
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs.create_dir_all(parent)?;
    }

    fs.write_file(path, content.as_bytes())?;
    Ok(())
}

fn write_json_pretty_atomic_with<T: Serialize + ?Sized>(
    fs: &dyn FileSystem,
    path: &Path,
    value: &T,
) -> Result<(), RuntimeFsError> {
    let serialized =
        serde_json::to_vec_pretty(value).map_err(|error| RuntimeFsError::Serialization {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    write_binary_atomic(fs, path, &serialized)
}

fn write_binary_atomic(
    fs: &dyn FileSystem,
    path: &Path,
    contents: &[u8],
) -> Result<(), RuntimeFsError> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        Uuid::new_v4()
    ));

    fs.write_file(&temp_path, contents)?;

    replace_path_atomically(fs, &temp_path, path)
}

fn replace_path_atomically(
    fs: &dyn FileSystem,
    temp_path: &Path,
    final_path: &Path,
) -> Result<(), RuntimeFsError> {
    let backup_name = format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4()
    );
    let backup_path = final_path.with_extension(&backup_name);
    let had_existing = fs.metadata(final_path)?.is_some();

    if had_existing {
        fs.rename(final_path, &backup_path)?;
    }

    match fs.rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                // Best-effort: stray .bak-* is harmless, never fail a successful write.
                let _ = remove_path_if_exists_with(fs, &backup_path);
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && fs.metadata(final_path).ok().flatten().is_none() {
                let _ = fs.rename(&backup_path, final_path);
            }
            let _ = remove_path_if_exists_with(fs, temp_path);
            Err(RuntimeFsError::FileSystem(error))
        }
    }
}

fn remove_path_if_exists_with(fs: &dyn FileSystem, path: &Path) -> Result<(), FileSystemError> {
    match fs.metadata(path)? {
        Some(meta) if meta.is_dir => fs.remove_dir_all(path),
        Some(_) => fs.remove_file(path),
        None => Ok(()),
    }
}

pub fn load_transcribe_config_file(path: &Path) -> Result<TranscribeConfigSection, RuntimeFsError> {
    let contents = RealFileSystem.read_to_string(path)?;
    Ok(sona_core::runtime::config::parse_transcribe_config_file(
        &contents,
        &path.display().to_string(),
    )?)
}

pub fn load_transcribe_live_config_file(
    path: &Path,
) -> Result<TranscribeLiveConfigSection, RuntimeFsError> {
    let contents = RealFileSystem.read_to_string(path)?;
    Ok(
        sona_core::runtime::config::parse_transcribe_live_config_file(
            &contents,
            &path.display().to_string(),
        )?,
    )
}

pub fn load_serve_config_file(path: &Path) -> Result<ServeConfigSection, RuntimeFsError> {
    let contents = RealFileSystem.read_to_string(path)?;
    Ok(sona_core::runtime::config::parse_serve_config_file(
        &contents,
        &path.display().to_string(),
    )?)
}

pub fn load_legacy_settings_app_config(
    app_data_dir: &Path,
) -> Result<Option<serde_json::Value>, RuntimeFsError> {
    let settings_path = app_data_dir.join("settings.json");
    if RealFileSystem.metadata(&settings_path)?.is_none() {
        return Ok(None);
    }
    let contents = RealFileSystem.read_to_string(&settings_path)?;
    let parsed =
        serde_json::from_str(&contents).map_err(|error| RuntimeFsError::Serialization {
            path: settings_path,
            reason: error.to_string(),
        })?;
    Ok(Some(sona_core::runtime::serve::app_config_payload_owned(
        parsed,
    )))
}

pub fn default_desktop_app_data_roots() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
            return Vec::new();
        };
        vec![base.join("com.asoda.sona"), base.join("Sona")]
    }

    #[cfg(target_os = "macos")]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let base = home.join("Library").join("Application Support");
        vec![base.join("com.asoda.sona"), base.join("Sona")]
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            return vec![data_home.join("com.asoda.sona"), data_home.join("Sona")];
        }
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let base = home.join(".local").join("share");
        vec![base.join("com.asoda.sona"), base.join("Sona")]
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

pub fn select_desktop_models_dir_from_app_roots<I>(app_roots: I) -> Option<PathBuf>
where
    I: IntoIterator<Item = PathBuf>,
{
    let app_roots = app_roots.into_iter().collect::<Vec<_>>();

    app_roots
        .iter()
        .map(|path| path.join("models"))
        .find(|path| path.exists())
        .or_else(|| app_roots.into_iter().next().map(|path| path.join("models")))
}

pub fn default_desktop_models_dir() -> Option<PathBuf> {
    select_desktop_models_dir_from_app_roots(default_desktop_app_data_roots())
}

pub fn models_dir_status(path: &Path) -> ModelsDirStatus {
    match path.metadata() {
        Ok(metadata) => status_of(true, metadata.is_dir()),
        Err(_) => status_of(false, false),
    }
}

pub fn resolve_runtime_path_status(path: &str) -> RuntimePathStatus {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::File,
            error: None,
        },
        Ok(metadata) if metadata.is_dir() => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Directory,
            error: None,
        },
        Ok(_) => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Unknown,
            error: Some("Path exists but is neither a regular file nor directory.".to_string()),
        },
        Err(error) if error.kind() == ErrorKind::NotFound => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Missing,
            error: None,
        },
        Err(error) => RuntimePathStatus {
            path: path.to_string(),
            kind: RuntimePathKind::Unknown,
            error: Some(error.to_string()),
        },
    }
}

pub fn automation_runtime_path_metadata(
    file_path: &str,
) -> Result<Option<AutomationRuntimePathMetadata>, FileSystemError> {
    let path = Path::new(file_path);
    let metadata = match fs::metadata(file_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(file_system_error(
                FileSystemOperation::Metadata,
                path,
                error,
            ));
        }
    };

    let mtime_ms = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0);

    Ok(Some(AutomationRuntimePathMetadata {
        is_file: metadata.is_file(),
        size: metadata.len(),
        mtime_ms,
    }))
}

pub fn collect_automation_runtime_candidate_paths(
    rule: &AutomationRuntimeRuleConfig,
) -> Result<Vec<String>, FileSystemError> {
    let watch_directory = rule.watch_directory.trim();
    if watch_directory.is_empty() {
        return Ok(Vec::new());
    }

    let mut walker = walkdir::WalkDir::new(watch_directory).follow_links(false);
    if !rule.recursive {
        walker = walker.max_depth(1);
    }

    let mut paths = Vec::new();

    for entry in walker.into_iter() {
        let entry = entry.map_err(|error| {
            FileSystemError::new(
                FileSystemOperation::ReadDirectory,
                error.path().unwrap_or_else(|| Path::new(watch_directory)),
                error.to_string(),
            )
        })?;
        if !entry.file_type().is_file() {
            continue;
        }

        let file_path = entry.path().to_string_lossy().into_owned();
        if should_consider_runtime_candidate_path(rule, &file_path) {
            paths.push(file_path);
        }
    }

    Ok(paths)
}

#[derive(Clone, Copy, Debug, Default)]
pub struct FsSourcePathStatusProvider;

impl SourcePathStatusProvider for FsSourcePathStatusProvider {
    fn status_for_path(&self, path: &str) -> SourcePathStatus {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => SourcePathStatus::File,
            Ok(metadata) if metadata.is_dir() => SourcePathStatus::Directory,
            Ok(_) => SourcePathStatus::Unknown,
            Err(error) if error.kind() == ErrorKind::NotFound => SourcePathStatus::Missing,
            Err(_) => SourcePathStatus::Unknown,
        }
    }
}

pub fn resolve_batch_input_source(
    input_dir: Option<&Path>,
    inputs: &[PathBuf],
    recursive: bool,
) -> Result<BatchInputSource, RuntimeFsError> {
    if let Some(input_dir) = input_dir {
        return Ok(BatchInputSource {
            inputs: collect_batch_input_files(input_dir, recursive)?,
            base_dir: input_dir.to_path_buf(),
            preserve_relative_paths: recursive,
        });
    }

    let inputs = expand_input_patterns(inputs)?;
    let base_dir = sona_core::transcription::runtime::common_input_parent(&inputs)?;
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
) -> Result<Vec<BatchOutputPlan>, RuntimeFsError> {
    let plans = sona_core::transcription::runtime::plan_batch_output_files(
        inputs,
        input_dir,
        output_dir,
        format,
        preserve_relative_paths,
        force,
    )?;

    if !force {
        for plan in &plans {
            if RealFileSystem.metadata(&plan.output_path)?.is_some() {
                return Err(RuntimeFsError::AlreadyExists {
                    context: "Output file",
                    path: plan.output_path.clone(),
                    hint: "Use --force to overwrite.",
                });
            }
        }
    }

    Ok(plans)
}

pub fn resolve_batch_transcribe_plan_with_runtime_paths(
    options: BatchTranscribeOptions,
    config: Option<TranscribeConfigSection>,
) -> Result<BatchTranscribePlan, RuntimeFsError> {
    resolve_batch_transcribe_plan_with_runtime_paths_and_models_dir_status(
        options,
        config,
        models_dir_status,
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeBatchTranscribePlanResolver;

impl BatchTranscribePlanResolver for RuntimeBatchTranscribePlanResolver {
    fn resolve_batch_transcribe_plan(
        &self,
        options: BatchTranscribeOptions,
    ) -> Result<BatchTranscribePlan, RuntimeCapabilityError> {
        resolve_batch_transcribe_plan_with_runtime_paths(options, None).map_err(|error| {
            RuntimeCapabilityError::BatchPlan {
                reason: error.to_string(),
            }
        })
    }
}

pub fn resolve_batch_transcribe_plan_with_runtime_paths_and_models_dir_status(
    options: BatchTranscribeOptions,
    config: Option<TranscribeConfigSection>,
    models_dir_status: fn(&Path) -> ModelsDirStatus,
) -> Result<BatchTranscribePlan, RuntimeFsError> {
    ensure_input_file_exists(&options.input)?;
    ensure_output_can_be_written(options.output.as_ref(), options.force)?;
    sona_core::transcription::runtime::resolve_batch_transcribe_plan_with_install_checker_and_models_dir_status(
        options,
        config,
        is_preset_model_installed_at,
        models_dir_status,
    )
    .map_err(RuntimeFsError::from)
}

pub fn resolve_live_transcribe_plan_with_runtime_paths(
    options: LiveTranscribeOptions,
    config: Option<TranscribeLiveConfigSection>,
) -> Result<LiveTranscribePlan, RuntimeFsError> {
    resolve_live_transcribe_plan_with_runtime_paths_and_models_dir_status(
        options,
        config,
        models_dir_status,
    )
}

pub fn resolve_live_transcribe_plan_with_runtime_paths_and_models_dir_status(
    options: LiveTranscribeOptions,
    config: Option<TranscribeLiveConfigSection>,
    models_dir_status: fn(&Path) -> ModelsDirStatus,
) -> Result<LiveTranscribePlan, RuntimeFsError> {
    ensure_output_can_be_written(options.output.as_ref(), options.force)?;
    sona_core::transcription::runtime::resolve_live_transcribe_plan_with_install_checker_and_models_dir_status(
        options,
        config,
        is_preset_model_installed_at,
        models_dir_status,
    )
    .map_err(RuntimeFsError::from)
}

pub fn is_preset_model_installed_at(model: &PresetModel, models_dir: &Path) -> bool {
    is_preset_model_install_path_complete(model, &model.resolve_install_path(models_dir))
}

pub fn build_model_catalog_snapshot(
    models_dir: &Path,
) -> sona_core::models::preset_models::ModelCatalogSnapshot {
    let installed_model_ids = installed_model_ids_for_models_dir(models_dir);
    sona_core::models::preset_models::build_model_catalog_snapshot_with_installed_ids(
        models_dir,
        &installed_model_ids,
    )
}

#[derive(Clone, Copy, Debug, Default)]
pub struct RuntimeModelCatalogProvider;

impl ModelCatalogProvider for RuntimeModelCatalogProvider {
    fn build_model_catalog_snapshot(
        &self,
        models_dir: &Path,
    ) -> Result<sona_core::models::preset_models::ModelCatalogSnapshot, RuntimeCapabilityError>
    {
        Ok(build_model_catalog_snapshot(models_dir))
    }
}

pub fn list_models(models_dir: &Path) -> Vec<ModelSummary> {
    let installed_model_ids = installed_model_ids_for_models_dir(models_dir);
    sona_core::models::catalog::list_models_with_installed_ids(models_dir, &installed_model_ids)
}

fn expand_input_patterns(inputs: &[PathBuf]) -> Result<Vec<PathBuf>, RuntimeFsError> {
    let mut expanded = Vec::new();

    for input in inputs {
        if path_contains_glob_pattern(input) {
            let pattern = input.to_string_lossy().to_string();
            let mut matches = glob::glob(&pattern)
                .map_err(|error| {
                    RuntimeFsError::validation(
                        "batch_input_glob",
                        format!("Invalid glob pattern {}: {error}", input.display()),
                    )
                })?
                .map(|entry| {
                    entry.map_err(|error| {
                        RuntimeFsError::FileSystem(FileSystemError::new(
                            FileSystemOperation::ReadDirectory,
                            error.path(),
                            format!("Failed to read glob match for {}: {error}", input.display()),
                        ))
                    })
                })
                .collect::<Result<Vec<_>, _>>()?;
            matches.retain(|path| path.is_file());
            matches.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
            if matches.is_empty() {
                return Err(RuntimeFsError::validation(
                    "batch_input_glob",
                    format!("No input files matched glob pattern: {}", input.display()),
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

fn collect_batch_input_files(
    input_dir: &Path,
    recursive: bool,
) -> Result<Vec<PathBuf>, RuntimeFsError> {
    if !RealFileSystem
        .metadata(input_dir)?
        .is_some_and(|metadata| metadata.is_dir)
    {
        return Err(RuntimeFsError::validation(
            "input_dir",
            format!(
                "--input-dir must be an existing directory: {}",
                input_dir.display()
            ),
        ));
    }

    let walker = walkdir::WalkDir::new(input_dir)
        .min_depth(1)
        .max_depth(if recursive { usize::MAX } else { 1 });
    let mut files = Vec::new();

    for entry in walker {
        let entry = entry.map_err(|error| {
            RuntimeFsError::FileSystem(FileSystemError::new(
                FileSystemOperation::ReadDirectory,
                error.path().unwrap_or(input_dir),
                format!("Failed to read {}: {error}", input_dir.display()),
            ))
        })?;
        if entry.file_type().is_file()
            && sona_core::transcription::runtime::is_supported_batch_media_path(entry.path())
        {
            files.push(entry.path().to_path_buf());
        }
    }

    files.sort_by_key(|path| path.to_string_lossy().to_ascii_lowercase());
    Ok(files)
}

fn path_contains_glob_pattern(path: &Path) -> bool {
    path.to_string_lossy()
        .chars()
        .any(|character| GLOB_PATTERN_CHARS.contains(&character))
}

fn ensure_input_file_exists(input: &Path) -> Result<(), RuntimeFsError> {
    if RealFileSystem
        .metadata(input)?
        .is_some_and(|metadata| metadata.is_file)
    {
        Ok(())
    } else {
        Err(RuntimeFsError::validation(
            "input",
            format!("Input file must be an existing file: {}", input.display()),
        ))
    }
}

fn ensure_output_can_be_written(
    output: Option<&PathBuf>,
    force: bool,
) -> Result<(), RuntimeFsError> {
    match output {
        None => Ok(()),
        Some(_) if force => Ok(()),
        Some(path) if RealFileSystem.metadata(path)?.is_none() => Ok(()),
        Some(path) => Err(RuntimeFsError::AlreadyExists {
            context: "Output file",
            path: path.clone(),
            hint: "Use --force to overwrite.",
        }),
    }
}

fn installed_model_ids_for_models_dir(models_dir: &Path) -> HashSet<String> {
    preset_models()
        .iter()
        .filter(|model| is_preset_model_installed_at(model, models_dir))
        .map(|model| model.id.clone())
        .collect()
}

fn is_preset_model_install_path_complete(model: &PresetModel, install_path: &Path) -> bool {
    let Ok(metadata) = install_path.metadata() else {
        return false;
    };

    if model.is_archive() {
        return true;
    }

    metadata.is_file() && metadata.len() > 0
}
