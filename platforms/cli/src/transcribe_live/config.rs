use clap::{Args, ValueEnum};
use sona_core::ports::asr::{AsrMode, AsrTranscriptionRequest};
use sona_core::runtime::config::TranscribeLiveConfigSection;
use sona_core::transcription::runtime::{LiveTranscribeOptions, LiveTranscribePlan};
use std::path::PathBuf;
use std::time::Duration;

use crate::live_output::LiveOutputFormat;
use crate::{CliError, CliResult};

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LiveInputSource {
    Microphone,
    Stdin,
}

impl LiveInputSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Microphone => "microphone",
            Self::Stdin => "stdin",
        }
    }

    pub fn parse_config(value: &str) -> CliResult<Self> {
        match value {
            "microphone" => Ok(Self::Microphone),
            "stdin" => Ok(Self::Stdin),
            _ => Err(CliError::Validation(format!(
                "Invalid transcribe_live input '{value}'. Expected microphone or stdin."
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LiveOutputFormatArg {
    Text,
    Ndjson,
}

impl LiveOutputFormatArg {
    pub fn parse_config(value: &str) -> CliResult<Self> {
        match value {
            "text" => Ok(Self::Text),
            "ndjson" => Ok(Self::Ndjson),
            _ => Err(CliError::Validation(format!(
                "Invalid transcribe_live output_format '{value}'. Expected text or ndjson."
            ))),
        }
    }
}

impl From<LiveOutputFormatArg> for LiveOutputFormat {
    fn from(value: LiveOutputFormatArg) -> Self {
        match value {
            LiveOutputFormatArg::Text => Self::Text,
            LiveOutputFormatArg::Ndjson => Self::Ndjson,
        }
    }
}

#[derive(Debug, Args)]
#[command(
    about = "Transcribe live audio with local or online ASR",
    after_help = "Examples:\n  sona-cli transcribe-live --model-id sherpa-onnx-sense-voice-zh-en-ja-ko-yue-int8-2024-07-17\n  sona-cli transcribe-live --online-provider volcengine-doubao\n  ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | sona-cli transcribe-live --input stdin --online-provider volcengine-doubao --output-format ndjson"
)]
pub struct TranscribeLiveArgs {
    /// Live input source.
    #[arg(long, value_enum)]
    pub(crate) input: Option<LiveInputSource>,
    /// Exact microphone device name.
    #[arg(long, value_name = "NAME")]
    pub(crate) device: Option<String>,
    /// List microphone input devices and exit.
    #[arg(long, default_value_t = false)]
    pub(crate) list_input_devices: bool,
    /// Stop after this many seconds.
    #[arg(long, value_name = "SECONDS")]
    pub(crate) duration: Option<f64>,
    /// Live stdout stream format: text or ndjson.
    #[arg(
        long,
        value_enum,
        value_name = "FORMAT",
        help = "Live stdout stream format: text or ndjson"
    )]
    pub(crate) output_format: Option<LiveOutputFormatArg>,
    /// Optional final transcript file.
    #[arg(short, long, value_name = "PATH")]
    pub(crate) output: Option<PathBuf>,
    /// Final transcript export format (json, txt, srt, vtt, md). Requires --output.
    #[arg(
        short,
        long,
        help = "Final transcript export format (json, txt, srt, vtt, md). Requires --output"
    )]
    pub(crate) format: Option<String>,
    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,
    /// Override the language setting.
    #[arg(short = 'l', long)]
    pub(crate) language: Option<String>,
    /// Streaming preset model id to use.
    #[arg(short = 'm', long = "model-id")]
    pub(crate) model_id: Option<String>,
    #[command(flatten)]
    pub(crate) online: crate::online_asr::OnlineAsrArgs,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir")]
    pub(crate) models_dir: Option<PathBuf>,
    /// VAD model id override.
    #[arg(long = "vad-model-id")]
    pub(crate) vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id")]
    pub(crate) punctuation_model_id: Option<String>,
    /// Number of recognition threads.
    #[arg(long)]
    pub(crate) threads: Option<i32>,
    /// Enable inverse text normalization.
    #[arg(long, default_value_t = false)]
    pub(crate) enable_itn: bool,
    /// Optional hotwords string.
    #[arg(long)]
    pub(crate) hotwords: Option<String>,
    /// GPU acceleration mode.
    #[arg(long = "gpu-acceleration")]
    pub(crate) gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds.
    #[arg(long = "vad-buffer")]
    pub(crate) vad_buffer: Option<f32>,
    /// Overwrite an existing final transcript file.
    #[arg(long, default_value_t = false)]
    pub(crate) force: bool,
}

pub(crate) struct ResolvedLiveCommand {
    pub(crate) input: LiveInputSource,
    pub(crate) device: Option<String>,
    pub(crate) duration: Option<Duration>,
    pub(crate) output_format: LiveOutputFormat,
    pub(crate) asr: ResolvedLiveAsr,
}

pub(crate) enum ResolvedLiveAsr {
    Local(Box<LiveTranscribePlan>),
    Online(Box<ResolvedOnlineLiveAsr>),
}

pub(crate) struct ResolvedOnlineLiveAsr {
    pub(crate) request: AsrTranscriptionRequest,
    pub(crate) provider_id: String,
    pub(crate) export_format: Option<sona_core::export::ExportFormat>,
    pub(crate) output_path: Option<PathBuf>,
}

pub(crate) fn validate_direct_input_options(
    input: Option<LiveInputSource>,
    device: Option<&str>,
    duration: Option<f64>,
) -> CliResult<()> {
    if duration.is_some_and(|seconds| !seconds.is_finite() || seconds <= 0.0) {
        return Err(CliError::Validation(
            "--duration must be greater than 0.".to_string(),
        ));
    }
    if input == Some(LiveInputSource::Stdin) && device.is_some() {
        return Err(CliError::Validation(
            "--device can only be used with microphone input.".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn load_config(
    path: Option<&PathBuf>,
) -> CliResult<Option<TranscribeLiveConfigSection>> {
    let resolved = crate::init_config::resolve_config_path(path);
    let Some(path) = resolved.as_ref() else {
        return Ok(None);
    };
    sona_runtime_fs::load_transcribe_live_config_file(path)
        .map(Some)
        .map_err(|error| CliError::Validation(error.to_string()))
}

pub(crate) fn resolve_live_command(
    args: TranscribeLiveArgs,
    config: Option<TranscribeLiveConfigSection>,
) -> CliResult<ResolvedLiveCommand> {
    let config = config.unwrap_or_default();
    let input = match args.input {
        Some(input) => input,
        None => config
            .input
            .as_deref()
            .map(LiveInputSource::parse_config)
            .transpose()?
            .unwrap_or(LiveInputSource::Microphone),
    };
    let device = args.device.clone().or(config.device.clone());
    let duration_seconds = args.duration.or(config.duration_seconds);
    validate_direct_input_options(Some(input), device.as_deref(), duration_seconds)?;
    let duration = duration_seconds.map(Duration::from_secs_f64);
    let output_format = match args.output_format {
        Some(format) => format,
        None => config
            .output_format
            .as_deref()
            .map(LiveOutputFormatArg::parse_config)
            .transpose()?
            .unwrap_or(LiveOutputFormatArg::Text),
    };
    let resolved_online = if args.model_id.is_some() && args.online.online_provider.is_none() {
        // Explicit --model-id on CLI overrides config-file online provider
        args.online.clone()
    } else {
        args.online.resolve_with_config(
            config.online_provider.clone(),
            config.api_key_env.clone(),
            config.online_config.clone(),
        )
    };
    let asr = if resolved_online.is_online() {
        reject_online_local_options(&args)?;
        validate_online_output(args.output.as_ref(), args.format.as_deref(), args.force)?;
        let language = args
            .language
            .clone()
            .or_else(|| config.language.clone())
            .unwrap_or_else(|| sona_core::transcription::runtime::DEFAULT_LANGUAGE.to_string());
        let enable_itn = args.enable_itn || config.enable_itn.unwrap_or(false);
        let hotwords = args.hotwords.clone().or_else(|| config.hotwords.clone());
        let request =
            resolved_online.build_request(AsrMode::Streaming, language, enable_itn, hotwords)?;
        let export_format = args
            .output
            .as_deref()
            .map(|path| {
                sona_core::transcription::runtime::resolve_export_format(
                    args.format.as_deref(),
                    Some(path),
                )
            })
            .transpose()
            .map_err(|error| CliError::Validation(error.to_string()))?;
        ResolvedLiveAsr::Online(Box::new(ResolvedOnlineLiveAsr {
            provider_id: request.provider_id().to_string(),
            request,
            export_format,
            output_path: args.output,
        }))
    } else {
        let plan = sona_runtime_fs::resolve_live_transcribe_plan_with_runtime_paths(
            LiveTranscribeOptions {
                output: args.output,
                format: args.format,
                model_id: args.model_id,
                models_dir: args.models_dir,
                default_models_dir: crate::desktop_paths::default_models_dir(),
                vad_model_id: args.vad_model_id,
                punctuation_model_id: args.punctuation_model_id,
                threads: args.threads,
                enable_itn: args.enable_itn.then_some(true),
                language: args.language,
                hotwords: args.hotwords,
                gpu_acceleration: args.gpu_acceleration,
                vad_buffer: args.vad_buffer,
                force: args.force,
            },
            Some(config),
        )
        .map_err(crate::map_runtime_fs_error)?;
        ResolvedLiveAsr::Local(Box::new(plan))
    };
    Ok(ResolvedLiveCommand {
        input,
        device,
        duration,
        output_format: output_format.into(),
        asr,
    })
}

fn reject_online_local_options(args: &TranscribeLiveArgs) -> CliResult<()> {
    let local_option = [
        (args.model_id.is_some(), "--model-id"),
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
    .find_map(|(present, option)| present.then_some(option));
    if let Some(option) = local_option {
        return Err(CliError::Validation(format!(
            "{option} can only be used with local ASR."
        )));
    }
    Ok(())
}

fn validate_online_output(
    output: Option<&PathBuf>,
    format: Option<&str>,
    force: bool,
) -> CliResult<()> {
    if output.is_none() && format.is_some() {
        return Err(CliError::Validation(
            "--format requires --output for live transcription.".to_string(),
        ));
    }
    if let Some(output) = output
        && output.exists()
        && !force
    {
        return Err(CliError::Io(format!(
            "Output file already exists: {}. Use --force to overwrite.",
            output.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_config_accepts_valid_inputs() {
        assert_eq!(
            LiveInputSource::parse_config("microphone").unwrap(),
            LiveInputSource::Microphone
        );
        assert_eq!(
            LiveInputSource::parse_config("stdin").unwrap(),
            LiveInputSource::Stdin
        );
        assert_eq!(
            LiveOutputFormatArg::parse_config("text").unwrap(),
            LiveOutputFormatArg::Text
        );
        assert_eq!(
            LiveOutputFormatArg::parse_config("ndjson").unwrap(),
            LiveOutputFormatArg::Ndjson
        );
    }

    #[test]
    fn parse_config_rejects_invalid_inputs() {
        assert!(LiveInputSource::parse_config("invalid").is_err());
        assert!(LiveOutputFormatArg::parse_config("invalid").is_err());
    }

    #[test]
    fn validate_direct_input_options_checks_duration_and_device() {
        assert!(validate_direct_input_options(None, None, Some(0.0)).is_err());
        assert!(validate_direct_input_options(None, None, Some(-1.0)).is_err());
        assert!(
            validate_direct_input_options(Some(LiveInputSource::Stdin), Some("mic"), None).is_err()
        );
        assert!(
            validate_direct_input_options(
                Some(LiveInputSource::Microphone),
                Some("mic"),
                Some(5.0)
            )
            .is_ok()
        );
    }

    #[test]
    fn validate_online_output_checks_format_and_overwrite() {
        assert!(validate_online_output(None, Some("srt"), false).is_err());
        assert!(validate_online_output(None, None, false).is_ok());
    }
}
