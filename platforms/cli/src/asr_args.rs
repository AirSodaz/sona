use std::path::PathBuf;

use crate::online_asr::OnlineAsrArgs;
use crate::{CliError, CliResult};

#[derive(Debug, Clone, Default, clap::Args)]
pub struct CommonAsrArgs {
    /// Preset model id to use, or online model name when an online provider is selected.
    #[arg(
        short = 'm',
        long = "model",
        alias = "model-id",
        help_heading = "Model Options"
    )]
    pub model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir", help_heading = "Model Options")]
    pub models_dir: Option<PathBuf>,
    /// Override the language setting.
    #[arg(short = 'l', long, help_heading = "Model Options")]
    pub language: Option<String>,
    /// VAD model id override.
    #[arg(long = "vad-model-id", help_heading = "Model Options")]
    pub vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id", help_heading = "Model Options")]
    pub punctuation_model_id: Option<String>,

    #[command(flatten)]
    pub online: OnlineAsrArgs,

    /// Number of recognition threads (default: system auto-configured).
    #[arg(long, help_heading = "Audio & Performance")]
    pub threads: Option<i32>,
    /// Enable inverse text normalization (convert spoken numbers/dates to digits, e.g. "一百二十" -> "120").
    #[arg(long, default_value_t = false, help_heading = "Audio & Performance")]
    pub enable_itn: bool,
    /// Optional hotwords string to enhance recognition, separated by newlines or commas.
    #[arg(long, help_heading = "Audio & Performance")]
    pub hotwords: Option<String>,
    /// GPU acceleration mode: auto, cpu, vulkan, metal, or cuda.
    #[arg(
        long = "gpu-acceleration",
        value_name = "MODE",
        value_parser = crate::runtime::gpu_acceleration_value_parser(),
        help_heading = "Audio & Performance"
    )]
    pub gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds, for example 0.5.
    #[arg(long = "vad-buffer", help_heading = "Audio & Performance")]
    pub vad_buffer: Option<f32>,
}

pub fn reject_online_local_options(
    args: &CommonAsrArgs,
    extra_local_options: &[(bool, &'static str)],
) -> CliResult<()> {
    let local_option = [
        (args.models_dir.is_some(), "--models-dir"),
        (args.vad_model_id.is_some(), "--vad-model-id"),
        (
            args.punctuation_model_id.is_some(),
            "--punctuation-model-id",
        ),
        (args.threads.is_some(), "--threads"),
        (args.gpu_acceleration.is_some(), "--gpu-acceleration"),
        (args.vad_buffer.is_some(), "--vad-buffer"),
    ]
    .into_iter()
    .chain(extra_local_options.iter().copied())
    .find_map(|(present, option)| present.then_some(option));
    if let Some(option) = local_option {
        return Err(CliError::Validation(format!(
            "{option} can only be used with local ASR."
        )));
    }
    Ok(())
}

pub fn reconcile_online_model(
    resolved_online: &mut OnlineAsrArgs,
    cli_model: Option<&str>,
) -> CliResult<()> {
    if let (true, Some(cli_model)) = (resolved_online.is_online(), cli_model) {
        if let Some(existing_online_model) = &resolved_online.online_model {
            if existing_online_model != cli_model {
                return Err(CliError::Validation(format!(
                    "Conflicting model names specified: -m/--model '{cli_model}' vs --online-model '{existing_online_model}'."
                )));
            }
        } else {
            resolved_online.online_model = Some(cli_model.to_string());
        }
    }
    Ok(())
}
