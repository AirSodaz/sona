use clap::{Args, Subcommand};
use std::path::{Path, PathBuf};

use crate::{CliError, CliOutput, CliResult};
use sona_core::cli_models::{
    CliModelSummary, ModelListEntry, ModelListFilter, list_cli_models, remove_model_install_path,
    render_cli_model_table, select_cli_models,
};

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[command(subcommand)]
    command: ModelCommands,
}

#[derive(Debug, Subcommand)]
pub enum ModelCommands {
    /// Lists preset models known to the CLI.
    #[command(
        after_help = "Examples:\n  sona-cli models list\n  sona-cli models list --mode offline --type whisper\n  sona-cli models list --language zh --installed"
    )]
    List(ModelListArgs),
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
        ModelCommands::Delete(args) => run_model_delete(args),
    }
}

fn run_model_list(args: ModelListArgs) -> CliResult<CliOutput> {
    let models = select_cli_models(
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
        render_cli_model_table(&models)
    };

    Ok(CliOutput::stdout(output))
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

    if !sona_core::preset_models::is_preset_model_installed_at(model, &models_dir)
        && !install_path.exists()
    {
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

fn list_models(models_dir: Option<PathBuf>) -> CliResult<Vec<CliModelSummary>> {
    let models_dir = resolve_models_dir(models_dir)?;
    Ok(list_cli_models(&models_dir))
}

fn resolve_models_dir(configured: Option<PathBuf>) -> CliResult<PathBuf> {
    let path = if let Some(path) = configured {
        path
    } else {
        sona_core::paths::default_desktop_models_dir().ok_or_else(|| {
            CliError::Validation(
                "Unable to infer the desktop models directory. Pass --models-dir explicitly."
                    .to_string(),
            )
        })?
    };

    if std::fs::metadata(&path)
        .map(|metadata| !metadata.is_dir())
        .unwrap_or(false)
    {
        return Err(CliError::Validation(format!(
            "Models directory '{}' exists but is not a directory.",
            path.display()
        )));
    }

    Ok(path)
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
        let table = render_cli_model_table(&[
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
