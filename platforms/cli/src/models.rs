use clap::{Args, Subcommand};
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::{CliError, CliOutput, CliResult};
use sona_core::model_catalog::{ModelListEntry, ModelListFilter, ModelSummary, select_models};
use sona_core::model_downloads::{
    ResolvedModelDownload, required_companion_models, resolve_model_download,
};
use sona_model_downloads::{download_model, installed_model_is_valid, remove_model_install_path};
use sona_runtime_fs::{is_preset_model_installed_at, list_models as list_model_catalog};

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[command(subcommand)]
    command: ModelCommands,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    /// Lists preset models known to the CLI.
    #[command(
        after_help = "Examples:\n  sona-cli models list\n  sona-cli models list --mode batch --type whisper\n  sona-cli models list --language zh --installed"
    )]
    List(ModelListArgs),
    /// Downloads a preset model into the models directory.
    #[command(
        after_help = "Examples:\n  sona-cli models download sherpa-onnx-whisper-turbo\n  sona-cli models download silero-vad --models-dir ./models"
    )]
    Download(ModelDownloadArgs),
    /// Deletes an installed preset model from the models directory.
    #[command(
        after_help = "Examples:\n  sona-cli models delete sherpa-onnx-whisper-turbo --models-dir ./models --yes\n  sona-cli models delete silero-vad --models-dir ./models --yes"
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
    #[arg(long, value_name = "MODE", help = "Filter by mode: streaming or batch")]
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
    /// Overwrites invalid installed files without prompting.
    #[arg(
        long,
        help = "Overwrite invalid files without prompting for confirmation"
    )]
    yes: bool,
}

#[derive(Debug, Args)]
#[command(
    about = "Delete an installed preset model",
    after_help = "Companion models are not deleted automatically. Pass --yes to confirm deletion."
)]
pub struct ModelDeleteArgs {
    /// Preset model id to delete.
    #[arg(help = "Preset model id, for example sherpa-onnx-whisper-turbo or silero-vad")]
    model_id: String,
    /// Models directory containing installed presets.
    #[arg(long, help = "Override the models directory")]
    models_dir: Option<PathBuf>,
    /// Confirms deletion without an interactive prompt.
    #[arg(long, help = "Delete without prompting for confirmation")]
    yes: bool,
}

pub fn run_models(args: ModelsArgs) -> CliResult<CliOutput> {
    match args.command {
        ModelCommands::List(args) => run_model_list(args),
        ModelCommands::Download(args) => run_model_download(args),
        ModelCommands::Delete(args) => run_model_delete(args),
    }
}

fn run_model_list(args: ModelListArgs) -> CliResult<CliOutput> {
    let models = select_models(
        list_models(args.models_dir.clone())?,
        &ModelListFilter {
            mode: args.mode.clone(),
            model_type: args.model_type.clone(),
            language: args.language.clone(),
            installed_only: args.installed,
        },
    );
    let output = if args.json {
        serde_json::to_string_pretty(
            &models
                .into_iter()
                .map(ModelListEntry::from)
                .collect::<Vec<_>>(),
        )
        .map_err(|error| CliError::Serialize(format!("Failed to serialize model list: {error}")))?
    } else {
        render_model_table(&models)
    };

    Ok(CliOutput::stdout(output))
}

fn run_model_download(args: ModelDownloadArgs) -> CliResult<CliOutput> {
    run_async(move || async move {
        let quiet = args.quiet;
        let yes = args.yes;
        let models_dir = resolve_models_dir(args.models_dir)?;
        let mut stderr_lines = Vec::new();

        let resolved =
            resolve_model_download(&args.model_id, &models_dir).map_err(CliError::Validation)?;
        download_one_model(&resolved, yes, quiet, &mut stderr_lines).await?;

        let companions = required_companion_models(&resolved.model);
        if let Some(vad_model_id) = companions.vad_model_id {
            let vad =
                resolve_model_download(&vad_model_id, &models_dir).map_err(CliError::Validation)?;
            download_one_model(&vad, yes, quiet, &mut stderr_lines).await?;
        }
        if let Some(punctuation_model_id) = companions.punctuation_model_id {
            let punctuation = resolve_model_download(&punctuation_model_id, &models_dir)
                .map_err(CliError::Validation)?;
            download_one_model(&punctuation, yes, quiet, &mut stderr_lines).await?;
        }

        Ok(CliOutput::stderr(stderr_lines.join("\n")))
    })
}

fn run_model_delete(args: ModelDeleteArgs) -> CliResult<CliOutput> {
    if !args.yes {
        return Err(CliError::Validation(
            "Refusing to delete without --yes in standalone CLI mode.".to_string(),
        ));
    }

    let models_dir = resolve_models_dir(args.models_dir)?;
    let model = sona_core::preset_models::find_preset_model(&args.model_id)
        .ok_or_else(|| CliError::Validation(format!("Unknown model id: {}", args.model_id)))?;
    let install_path = model.resolve_install_path(&models_dir);

    if !is_preset_model_installed_at(model, &models_dir) && !install_path.exists() {
        return Ok(CliOutput::stderr(format!(
            "Model {} is not installed at {}",
            model.id,
            install_path.display()
        )));
    }

    remove_model_install_path(&install_path).map_err(CliError::Io)?;

    Ok(CliOutput::stderr(format!(
        "Deleted {} from {}",
        model.id,
        install_path.display()
    )))
}

async fn download_one_model(
    resolved: &ResolvedModelDownload,
    yes: bool,
    quiet: bool,
    stderr_lines: &mut Vec<String>,
) -> CliResult<()> {
    if installed_model_is_valid(resolved)
        .await
        .map_err(CliError::Io)?
    {
        stderr_lines.push(format!(
            "Installed {} at {}",
            resolved.model.id,
            resolved.install_path.display()
        ));
        return Ok(());
    }

    if resolved.install_path.exists()
        && !yes
        && !confirm_model_overwrite(&resolved.model.id, &resolved.install_path)?
    {
        return Err(CliError::Model(
            "Download cancelled: model files are invalid and user declined to overwrite."
                .to_string(),
        ));
    }

    let stderr_is_terminal = io::stderr().is_terminal();
    let has_printed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let has_printed_clone = has_printed.clone();
    let display_id = resolved.model.id.clone();
    let mut last_percentage: Option<i32> = None;

    let install_path = download_model(resolved, move |downloaded, total| {
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
    .await
    .map_err(map_download_error)?;

    if has_printed.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!();
    }
    stderr_lines.push(format!(
        "Installed {} at {}",
        resolved.model.id,
        install_path.display()
    ));
    Ok(())
}

fn confirm_model_overwrite(model_id: &str, install_path: &Path) -> CliResult<bool> {
    if !io::stdin().is_terminal() {
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

fn list_models(models_dir: Option<PathBuf>) -> CliResult<Vec<ModelSummary>> {
    let models_dir = resolve_models_dir(models_dir)?;
    Ok(list_model_catalog(&models_dir))
}

fn resolve_models_dir(configured: Option<PathBuf>) -> CliResult<PathBuf> {
    sona_core::model_paths::resolve_models_dir(
        configured,
        crate::desktop_paths::default_models_dir(),
        crate::desktop_paths::models_dir_status,
    )
    .map_err(CliError::Validation)
}

fn map_download_error(error: String) -> CliError {
    if error.contains("cancelled by user") {
        CliError::Cancelled(error)
    } else if error.contains("Unknown model id") {
        CliError::Validation(error)
    } else if error.contains("hash mismatch") {
        CliError::Model(error)
    } else if error.contains("Failed to create HTTP client")
        || error.contains("Failed to download model")
        || error.contains("Download failed with status")
    {
        CliError::Network(error)
    } else if error.contains("Failed to create models directory")
        || error.contains("Failed to calculate hash")
        || error.contains("Failed to publish download")
        || error.contains("Failed to remove archive")
        || error.contains("Failed to open archive")
        || error.contains("Failed to extract archive")
    {
        CliError::Io(error)
    } else {
        CliError::Other(error)
    }
}

fn run_async<F, Fut>(factory: F) -> CliResult<CliOutput>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = CliResult<CliOutput>>,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?;
    runtime.block_on(factory())
}

fn render_model_table(models: &[ModelSummary]) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn model_summary(id: &str, model_type: &str, language: &str, installed: bool) -> ModelSummary {
        ModelSummary {
            id: id.to_string(),
            name: format!("{id} name"),
            model_type: model_type.to_string(),
            language: language.to_string(),
            size: "1 MB".to_string(),
            modes: vec!["batch".to_string()],
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
}
