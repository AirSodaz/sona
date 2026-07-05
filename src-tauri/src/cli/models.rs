use crate::cli::transcribe::{OutputTarget, write_output};
use crate::cli::{CliError, CliResult};
use crate::core::downloads::sha256_file;
use crate::core::preset_models::{
    PresetModel, find_preset_model, is_preset_model_installed_at, preset_models,
};
use clap::{Args, Subcommand};

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliModelSummary {
    pub id: String,
    pub name: String,
    pub model_type: String,
    pub language: String,
    pub size: String,
    pub modes: Vec<String>,
    pub installed: bool,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ResolvedModelDownload {
    pub model: PresetModel,
    pub models_dir: PathBuf,
    pub download_path: PathBuf,
    pub install_path: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RequiredCompanionModels {
    pub vad_model_id: Option<String>,
    pub punctuation_model_id: Option<String>,
}

#[derive(Debug, Args)]

pub struct ModelsArgs {
    #[command(subcommand)]
    command: ModelCommands,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    /// Lists preset models known to the CLI.
    #[command(
        after_help = "Examples:\n  sona models list\n  sona models list --mode offline --type whisper\n  sona models list --language zh --installed"
    )]
    List(ModelListArgs),
    /// Downloads a preset model into the models directory.
    #[command(
        after_help = "Examples:\n  sona models download sherpa-onnx-whisper-turbo\n  sona models download silero-vad --models-dir ./models"
    )]
    Download(ModelDownloadArgs),
    /// Deletes an installed preset model from the models directory.
    #[command(
        after_help = "Examples:\n  sona models delete sherpa-onnx-whisper-turbo\n  sona models delete silero-vad --models-dir ./models --yes"
    )]
    Delete(ModelDeleteArgs),
}

#[derive(Debug, Args)]
#[command(about = "List preset models with optional filters")]
pub struct ModelListArgs {
    /// Models directory containing installed presets.
    #[arg(
        long,
        help = "Override the models directory used to detect installed models"
    )]
    models_dir: Option<PathBuf>,
    /// Filter by supported mode.
    #[arg(
        long,
        value_name = "MODE",
        help = "Filter by mode: streaming or offline"
    )]
    mode: Option<String>,
    /// Filter by model type.
    #[arg(
        long = "type",
        value_name = "TYPE",
        help = "Filter by type, for example whisper, vad, punctuation"
    )]
    model_type: Option<String>,
    /// Filter by language token.
    #[arg(
        long,
        value_name = "LANG",
        help = "Filter by language token, for example zh, en, ja, yue"
    )]
    language: Option<String>,
    /// Show only installed models.
    #[arg(
        long,
        help = "Only include models already present in the models directory"
    )]
    installed: bool,
    /// Prints JSON instead of the default table output.
    #[arg(long, help = "Print machine-readable JSON")]
    json: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Download a preset model and any required companion models",
    after_help = "Required companion models are downloaded automatically when the preset needs VAD or punctuation."
)]
pub struct ModelDownloadArgs {
    /// Preset model id to download.
    #[arg(help = "Preset model id, for example sherpa-onnx-whisper-turbo or silero-vad")]
    model_id: String,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the target models directory")]
    models_dir: Option<PathBuf>,
    /// Suppresses progress logs.
    #[arg(long, help = "Hide per-download progress output")]
    quiet: bool,
    /// Skips the interactive confirmation prompt when overwriting invalid files.
    #[arg(
        long,
        help = "Overwrite invalid files without prompting for confirmation"
    )]
    pub yes: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Delete an installed preset model",
    after_help = "Companion models are not deleted automatically. Pass --yes to skip the confirmation prompt."
)]
pub struct ModelDeleteArgs {
    /// Preset model id to delete.
    #[arg(help = "Preset model id, for example sherpa-onnx-whisper-turbo or silero-vad")]
    model_id: String,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the models directory")]
    models_dir: Option<PathBuf>,
    /// Skips the interactive confirmation prompt.
    #[arg(long, help = "Delete without prompting for confirmation")]
    yes: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct ModelListEntry {
    id: String,
    name: String,
    #[serde(rename = "type")]
    model_type: String,
    language: String,
    size: String,
    modes: Vec<String>,
    installed: bool,
    install_path: String,
}

pub async fn run_models(args: ModelsArgs) -> CliResult<()> {
    match args.command {
        ModelCommands::List(args) => run_model_list(args),
        ModelCommands::Download(args) => run_model_download(args).await,
        ModelCommands::Delete(args) => run_model_delete(args),
    }
}

fn run_model_list(args: ModelListArgs) -> CliResult<()> {
    let models = filter_models(list_models(args.models_dir.clone())?, &args);
    let output = if args.json {
        serde_json::to_string_pretty(
            &models
                .into_iter()
                .map(ModelListEntry::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| CliError::Other(format!("Failed to serialize model list: {error}")))?
    } else {
        render_model_table(&models)
    };
    write_output(&OutputTarget::Stdout, &output)
}

fn filter_models(models: Vec<CliModelSummary>, args: &ModelListArgs) -> Vec<CliModelSummary> {
    let language_filter = args.language.as_deref().map(str::to_lowercase);
    models
        .into_iter()
        .filter(|model| {
            args.mode
                .as_deref()
                .map(|mode| model.modes.iter().any(|item| item == mode))
                .unwrap_or(true)
        })
        .filter(|model| {
            args.model_type
                .as_deref()
                .map(|model_type| model.model_type == model_type)
                .unwrap_or(true)
        })
        .filter(|model| {
            language_filter
                .as_deref()
                .map(|language| {
                    model
                        .language
                        .split(',')
                        .any(|item| item.trim().eq_ignore_ascii_case(language))
                })
                .unwrap_or(true)
        })
        .filter(|model| !args.installed || model.installed)
        .collect()
}

impl From<CliModelSummary> for ModelListEntry {
    fn from(model: CliModelSummary) -> Self {
        Self {
            id: model.id,
            name: model.name,
            model_type: model.model_type,
            language: model.language,
            size: model.size,
            modes: model.modes,
            installed: model.installed,
            install_path: model.install_path.to_string_lossy().to_string(),
        }
    }
}

fn render_model_table(models: &[CliModelSummary]) -> String {
    let rows = models
        .iter()
        .map(|model| {
            [
                model.id.clone(),
                model.model_type.clone(),
                model.language.clone(),
                model.size.clone(),
                if model.installed { "yes" } else { "no" }.to_string(),
                model.modes.join(","),
            ]
        })
        .collect::<Vec<_>>();
    let headers = ["ID", "Type", "Language", "Size", "Installed", "Modes"];
    let mut widths = headers.map(str::len);

    for row in &rows {
        for (index, value) in row.iter().enumerate() {
            widths[index] = widths[index].max(value.len());
        }
    }

    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in rows {
        let refs = [
            row[0].as_str(),
            row[1].as_str(),
            row[2].as_str(),
            row[3].as_str(),
            row[4].as_str(),
            row[5].as_str(),
        ];
        append_table_row(&mut output, &refs, &widths);
    }
    output
}

fn append_table_row(output: &mut String, values: &[&str; 6], widths: &[usize; 6]) {
    for (index, value) in values.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&format!("{value:<width$}", width = widths[index]));
    }
    output.push('\n');
}

fn append_table_separator(output: &mut String, widths: &[usize; 6]) {
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            output.push_str("  ");
        }
        output.push_str(&"-".repeat(*width));
    }
    output.push('\n');
}

async fn run_model_download(args: ModelDownloadArgs) -> CliResult<()> {
    let quiet = args.quiet;
    let yes = args.yes;
    let models_dir = args.models_dir;
    let resolved = resolve_model_download(&args.model_id, models_dir.clone())?;
    download_one_model(&resolved.model.id, &resolved, yes, quiet).await?;

    let RequiredCompanionModels {
        vad_model_id,
        punctuation_model_id,
    } = required_companion_models(&resolved.model);

    if let Some(vad_model_id) = vad_model_id {
        let vad_resolved = resolve_model_download(&vad_model_id, models_dir.clone())?;
        download_one_model(&vad_model_id, &vad_resolved, yes, quiet).await?;
    }
    if let Some(punctuation_model_id) = punctuation_model_id {
        let punctuation_resolved = resolve_model_download(&punctuation_model_id, models_dir)?;
        download_one_model(&punctuation_model_id, &punctuation_resolved, yes, quiet).await?;
    }

    Ok(())
}

fn run_model_delete(args: ModelDeleteArgs) -> CliResult<()> {
    let resolved = resolve_model_download(&args.model_id, args.models_dir)?;

    if !is_preset_model_installed_at(&resolved.model, &resolved.models_dir)
        && !resolved.install_path.exists()
    {
        eprintln!(
            "Model {} is not installed at {}",
            resolved.model.id,
            resolved.install_path.display()
        );
        return Ok(());
    }

    if !args.yes && !confirm_model_delete(&resolved.model.id, &resolved.install_path)? {
        eprintln!("Delete cancelled.");
        return Ok(());
    }

    remove_model_install_path(&resolved.install_path)?;
    eprintln!(
        "Deleted {} from {}",
        resolved.model.id,
        resolved.install_path.display()
    );
    Ok(())
}

fn confirm_model_delete(model_id: &str, install_path: &Path) -> CliResult<bool> {
    if !io::stdin().is_terminal() && std::env::var("SONA_TEST_NON_INTERACTIVE_OK").is_err() {
        return Err(CliError::Validation(
            "Cannot prompt for confirmation in non-interactive shell. Use --yes to override."
                .to_string(),
        ));
    }

    eprint!(
        "Delete model {model_id} at {}? [y/N] ",
        install_path.display()
    );
    io::stderr()
        .flush()
        .map_err(|error| CliError::Io(format!("Failed to flush confirmation prompt: {error}")))?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| CliError::Io(format!("Failed to read confirmation: {error}")))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn confirm_model_overwrite(model_id: &str, install_path: &Path) -> CliResult<bool> {
    if !io::stdin().is_terminal() && std::env::var("SONA_TEST_NON_INTERACTIVE_OK").is_err() {
        return Err(CliError::Validation(
            "Cannot prompt for confirmation in non-interactive shell. Use --yes to override."
                .to_string(),
        ));
    }

    eprint!(
        "Model {model_id} already exists at {} but is invalid (checksum mismatch). Overwrite? [y/N] ",
        install_path.display()
    );
    io::stderr()
        .flush()
        .map_err(|error| CliError::Io(format!("Failed to flush confirmation prompt: {error}")))?;

    let mut answer = String::new();
    io::stdin()
        .read_line(&mut answer)
        .map_err(|error| CliError::Io(format!("Failed to read confirmation: {error}")))?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

fn remove_model_install_path(install_path: &Path) -> CliResult<()> {
    let metadata = match std::fs::symlink_metadata(install_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(CliError::Io(format!(
                "Failed to inspect model path {}: {error}",
                install_path.display()
            )));
        }
    };

    if metadata.file_type().is_dir() {
        std::fs::remove_dir_all(install_path).map_err(|error| {
            CliError::Io(format!(
                "Failed to delete model directory {}: {error}",
                install_path.display()
            ))
        })
    } else {
        std::fs::remove_file(install_path).map_err(|error| {
            CliError::Io(format!(
                "Failed to delete model file {}: {error}",
                install_path.display()
            ))
        })
    }
}

async fn download_one_model(
    display_model_id: &str,
    resolved: &ResolvedModelDownload,
    yes: bool,
    quiet: bool,
) -> CliResult<()> {
    let stderr_is_terminal = io::stderr().is_terminal();
    let has_printed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let has_printed_clone = has_printed.clone();
    let display_id = display_model_id.to_string();
    let mut last_percentage: Option<i32> = None;

    let install_path = download_model(resolved, yes, move |downloaded, total| {
        if quiet || total == 0 {
            return;
        }
        let percentage = ((downloaded as f64 / total as f64) * 100.0).round() as i32;
        if stderr_is_terminal {
            eprint!("\rDownloading {display_id}: {percentage}%");
            let _ = io::stderr().flush();
            has_printed_clone.store(true, std::sync::atomic::Ordering::Relaxed);
        } else if (percentage == 100 || percentage % 10 == 0) && last_percentage != Some(percentage)
        {
            eprintln!("Downloading {display_id}: {percentage}%");
            last_percentage = Some(percentage);
        }
    })
    .await?;

    if has_printed.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!();
    }
    eprintln!(
        "Installed {} at {}",
        resolved.model.id,
        install_path.display()
    );
    Ok(())
}

/// Returns preset models known to the CLI with installation status.
pub fn list_models(models_dir: Option<PathBuf>) -> CliResult<Vec<CliModelSummary>> {
    let models_dir = resolve_models_dir(models_dir)?;
    Ok(preset_models()
        .iter()
        .map(|model| {
            let install_path = model.resolve_install_path(&models_dir);
            CliModelSummary {
                id: model.id.clone(),
                name: model.name.clone(),
                model_type: model.model_type.clone(),
                language: model.language.clone(),
                size: model.size.clone(),
                modes: model.modes.clone().unwrap_or_default(),
                installed: is_preset_model_installed_at(model, &models_dir),
                install_path,
            }
        })
        .collect())
}

/// Resolves model download settings from CLI arguments and defaults.
pub fn resolve_model_download(
    model_id: &str,
    models_dir: Option<PathBuf>,
) -> CliResult<ResolvedModelDownload> {
    let models_dir = resolve_models_dir(models_dir)?;
    let model = find_preset_model(model_id)
        .ok_or_else(|| CliError::Validation(format!("Unknown model id: {model_id}")))?
        .clone();
    let download_path = model.resolve_download_path(&models_dir);
    let install_path = model.resolve_install_path(&models_dir);

    Ok(ResolvedModelDownload {
        model,
        models_dir,
        download_path,
        install_path,
    })
}

/// Downloads a preset model into the local models directory.
pub async fn download_model<F>(
    resolved: &ResolvedModelDownload,
    yes: bool,
    on_progress: F,
) -> Result<PathBuf, CliError>
where
    F: FnMut(u64, u64) + Send + 'static,
{
    if is_preset_model_installed_at(&resolved.model, &resolved.models_dir) {
        let is_valid = if resolved.model.is_archive() {
            true
        } else {
            match &resolved.model.sha256 {
                Some(expected_sha) => {
                    if let Ok(actual_sha) = sha256_file(&resolved.install_path).await {
                        actual_sha.eq_ignore_ascii_case(expected_sha)
                    } else {
                        false
                    }
                }
                None => true,
            }
        };

        if is_valid {
            return Ok(resolved.install_path.clone());
        } else if !yes && !confirm_model_overwrite(&resolved.model.id, &resolved.install_path)? {
            return Err(CliError::Model(
                "Download cancelled: model files are invalid and user declined to overwrite."
                    .to_string(),
            ));
        }
    }

    tokio::fs::create_dir_all(&resolved.models_dir)
        .await
        .map_err(|error| {
            CliError::Io(format!(
                "Failed to create models directory {}: {error}",
                resolved.models_dir.display()
            ))
        })?;

    let client = reqwest::Client::builder()
        .user_agent("Sona/1.0")
        .build()
        .map_err(|error| CliError::Network(format!("Failed to create HTTP client: {error}")))?;

    let temp_download_path =
        crate::core::downloads::temporary_download_path(&resolved.download_path);

    let notify = std::sync::Arc::new(tokio::sync::Notify::new());
    let notify_clone = notify.clone();
    let ctrl_c_task = tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            notify_clone.notify_one();
        }
    });

    let result = crate::core::downloads::download_file(
        &client,
        &resolved.model.url,
        &temp_download_path,
        notify,
        Some(Box::new(on_progress)),
    )
    .await;

    ctrl_c_task.abort();

    result.map_err(|error| match error {
        crate::core::downloads::DownloadError::Cancelled => {
            CliError::Cancelled("Download cancelled by user".to_string())
        }
        crate::core::downloads::DownloadError::HttpStatus(status) => {
            CliError::Network(format!("Download failed with status: {status}"))
        }
        error => CliError::Network(format!("Failed to download model: {error}")),
    })?;

    // Validate SHA256 of the newly downloaded file before publishing
    if let Some(expected_sha) = &resolved.model.sha256 {
        let actual_sha = match sha256_file(&temp_download_path).await {
            Ok(sha) => sha,
            Err(err) => {
                let _ = tokio::fs::remove_file(&temp_download_path).await;
                return Err(CliError::Io(format!(
                    "Failed to calculate hash of downloaded file: {err}"
                )));
            }
        };
        if !actual_sha.eq_ignore_ascii_case(expected_sha) {
            let _ = tokio::fs::remove_file(&temp_download_path).await;
            return Err(CliError::Model(format!(
                "Downloaded file hash mismatch for {}. Expected: {}, got: {}",
                temp_download_path.display(),
                expected_sha,
                actual_sha
            )));
        }
    }

    crate::core::downloads::publish_download_file(&temp_download_path, &resolved.download_path)
        .await
        .map_err(|error| {
            CliError::Io(format!(
                "Failed to publish download {} to {}: {error}",
                temp_download_path.display(),
                resolved.download_path.display()
            ))
        })?;

    if resolved.model.is_archive() {
        extract_tar_bz2_archive(&resolved.download_path, &resolved.models_dir).await?;
        tokio::fs::remove_file(&resolved.download_path)
            .await
            .map_err(|error| {
                CliError::Io(format!(
                    "Failed to remove archive {}: {error}",
                    resolved.download_path.display()
                ))
            })?;
    }

    Ok(resolved.install_path.clone())
}

/// Returns suggested companion model ids for the given preset model.
pub fn required_companion_models(model: &PresetModel) -> RequiredCompanionModels {
    let rules = model.resolved_rules();
    RequiredCompanionModels {
        vad_model_id: rules.requires_vad.then(|| "silero-vad".to_string()),
        punctuation_model_id: rules.requires_punctuation.then(|| {
            "sherpa-onnx-punct-ct-transformer-zh-en-vocab272727-2024-04-12-int8".to_string()
        }),
    }
}

async fn extract_tar_bz2_archive(archive_path: &Path, target_dir: &Path) -> CliResult<()> {
    let archive_path = archive_path.to_path_buf();
    let target_dir = target_dir.to_path_buf();

    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&archive_path).map_err(|error| {
            CliError::Io(format!(
                "Failed to open archive {}: {error}",
                archive_path.display()
            ))
        })?;
        let buffered = std::io::BufReader::new(file);
        let tar = bzip2::read::BzDecoder::new(buffered);
        let mut archive = tar::Archive::new(tar);
        archive.set_preserve_permissions(false);
        archive.set_unpack_xattrs(false);
        archive.unpack(&target_dir).map_err(|error| {
            CliError::Io(format!(
                "Failed to extract archive into {}: {error}",
                target_dir.display()
            ))
        })
    })
    .await
    .map_err(|error| CliError::Other(format!("Failed to join extraction task: {error}")))?
}

pub fn resolve_models_dir(configured: Option<PathBuf>) -> Result<PathBuf, CliError> {
    let path = if let Some(path) = configured {
        path
    } else {
        default_models_dir().ok_or_else(|| {
            CliError::Validation(
                "Unable to infer the desktop models directory. Pass --models-dir explicitly."
                    .to_string(),
            )
        })?
    };

    if std::fs::metadata(&path)
        .map(|meta| !meta.is_dir())
        .unwrap_or(false)
    {
        return Err(CliError::Validation(format!(
            "Models directory '{}' exists but is not a directory.",
            path.display()
        )));
    }

    Ok(path)
}

pub(crate) fn default_models_dir() -> Option<PathBuf> {
    default_models_dir_candidates()
        .into_iter()
        .map(|path| path.join("models"))
        .find(|path| path.exists())
        .or_else(|| {
            default_models_dir_candidates()
                .into_iter()
                .next()
                .map(|path| path.join("models"))
        })
}

fn default_models_dir_candidates() -> Vec<PathBuf> {
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
        return vec![base.join("com.asoda.sona"), base.join("Sona")];
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
        return vec![base.join("com.asoda.sona"), base.join("Sona")];
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model_summary(
        id: &str,
        model_type: &str,
        language: &str,
        installed: bool,
    ) -> CliModelSummary {
        CliModelSummary {
            id: id.to_string(),
            name: format!("{id} name"),
            model_type: model_type.to_string(),
            language: language.to_string(),
            size: "1 MB".to_string(),
            modes: vec!["offline".to_string()],
            installed,
            install_path: PathBuf::from(format!("C:/models/{id}")),
        }
    }

    #[test]
    fn renders_model_list_as_table_with_headers() {
        let table = render_model_table(&[
            model_summary("short", "vad", "all", true),
            model_summary("longer-model-id", "whisper", "zh,en", false),
        ]);

        assert!(table.contains("ID"));
        assert!(table.contains("Type"));
        assert!(table.contains("Language"));
        assert!(table.contains("Size"));
        assert!(table.contains("Installed"));
        assert!(table.contains("Modes"));
        assert!(table.contains("longer-model-id"));
        assert!(table.contains("yes"));
        assert!(table.contains("no"));
        assert!(!table.contains("install_path"));
    }

    #[tokio::test]
    async fn download_single_file_model_does_not_accept_sha256_mismatch_existing_file() {
        let dir = tempfile::tempdir().unwrap();
        let models_dir = dir.path().join("models");
        std::fs::create_dir_all(&models_dir).unwrap();

        let install_path = models_dir.join("silero_vad.onnx");
        std::fs::write(&install_path, b"wrong model bytes").unwrap();

        let mut model = find_preset_model("silero-vad").unwrap().clone();
        model.url = "http://127.0.0.1:9/silero_vad.onnx".to_string();
        let resolved = ResolvedModelDownload {
            model,
            models_dir,
            download_path: install_path.clone(),
            install_path,
        };

        // Case 1: yes = false, non-interactive shell should return "Cannot prompt for confirmation" error
        let result = download_model(&resolved, false, |_, _| {}).await;
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Cannot prompt for confirmation")
        );

        // Case 2: yes = true, it should bypass the prompt and proceed to download, failing on connection or gateway status
        let result = download_model(&resolved, true, |_, _| {}).await;
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("Failed to download") || err_msg.contains("Download failed"),
            "Expected error message to contain 'Failed to download' or 'Download failed', but was: '{}'",
            err_msg
        );
    }

    #[test]
    fn resolve_models_dir_fails_when_path_is_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("not_a_dir.txt");
        std::fs::write(&file_path, b"dummy data").unwrap();

        let result = resolve_models_dir(Some(file_path));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CliError::Validation(_)));
        assert!(err.to_string().contains("exists but is not a directory"));
    }

    #[test]
    fn resolve_models_dir_succeeds_when_path_is_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dir_path = dir.path().join("valid_dir");
        std::fs::create_dir_all(&dir_path).unwrap();

        let result = resolve_models_dir(Some(dir_path.clone()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), dir_path);
    }

    #[test]
    fn resolve_models_dir_succeeds_when_path_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let non_existent_path = dir.path().join("does_not_exist");

        let result = resolve_models_dir(Some(non_existent_path.clone()));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), non_existent_path);
    }
}
