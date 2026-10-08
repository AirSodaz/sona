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
                "Invalid transcribe_live stream_format '{value}'. Expected text or ndjson."
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
    after_help = "Examples:\n  sona-cli live -m sensevoice\n  sona-cli live -m sensevoice --duration 30 -o ./meeting.srt\n  sona-cli live --online-provider volcengine-doubao\n  ffmpeg -i sample.wav -f s16le -ac 1 -ar 16000 - | \\\n    sona-cli live --input stdin -m sensevoice --stream ndjson"
)]
pub struct TranscribeLiveArgs {
    /// Live input source.
    #[arg(long, value_enum, help_heading = "Input/Output")]
    pub(crate) input: Option<LiveInputSource>,
    /// Microphone device index (e.g. 0), exact name, or unique substring (e.g. "realtek").
    #[arg(
        long,
        value_name = "DEVICE",
        help = "Microphone device index (e.g. 0), exact name, or unique substring (e.g. \"realtek\")",
        help_heading = "Input/Output"
    )]
    pub(crate) device: Option<String>,
    /// Stop after this many seconds (supports fractional seconds, e.g. 10.5).
    #[arg(long, value_name = "SECONDS", help_heading = "Input/Output")]
    pub(crate) duration: Option<f64>,
    /// Live stdout stream format: text or ndjson (alias: --stream-format).
    #[arg(
        long = "stream",
        value_enum,
        value_name = "FORMAT",
        alias = "stream-format",
        help = "Live stdout stream format: text or ndjson (alias: --stream-format)",
        help_heading = "Input/Output"
    )]
    pub(crate) stream: Option<LiveOutputFormatArg>,
    /// Optional final transcript file.
    #[arg(short, long, value_name = "PATH", help_heading = "Input/Output")]
    pub(crate) output: Option<PathBuf>,
    /// Save the live audio input to a WAV file.
    #[arg(
        long = "save-audio",
        visible_alias = "save-wav",
        value_name = "PATH",
        help = "Save recorded audio input to a WAV file",
        help_heading = "Input/Output"
    )]
    pub(crate) save_audio: Option<PathBuf>,
    /// Final transcript export format (json, txt, srt, vtt, md). Requires --output.
    #[arg(
        short,
        long,
        visible_alias = "export-format",
        value_name = "FORMAT",
        value_parser = ["json", "txt", "srt", "vtt", "md"],
        help = "Final transcript export format (json, txt, srt, vtt, md). Requires --output",
        help_heading = "Input/Output"
    )]
    pub(crate) format: Option<String>,
    /// Text selection mode for final transcript: original, translation, or bilingual.
    #[arg(
        long,
        value_name = "MODE",
        value_parser = ["original", "translation", "bilingual"],
        default_value = "original",
        help = "Text selection mode for final transcript: original, translation, or bilingual",
        help_heading = "Input/Output"
    )]
    pub(crate) mode: String,
    /// Overwrite an existing final transcript file.
    #[arg(
        short = 'F',
        long,
        default_value_t = false,
        help = "Overwrite an existing final transcript file",
        help_heading = "Input/Output"
    )]
    pub(crate) force: bool,

    /// Streaming preset model id to use, or online model name when an online provider is selected.
    #[arg(
        short = 'm',
        long = "model",
        alias = "model-id",
        help_heading = "Model Options"
    )]
    pub(crate) model_id: Option<String>,
    /// Models directory containing installed presets.
    #[arg(long = "models-dir", help_heading = "Model Options")]
    pub(crate) models_dir: Option<PathBuf>,
    /// Override the language setting.
    #[arg(short = 'l', long, help_heading = "Model Options")]
    pub(crate) language: Option<String>,
    /// VAD model id override.
    #[arg(long = "vad-model-id", help_heading = "Model Options")]
    pub(crate) vad_model_id: Option<String>,
    /// Punctuation model id override.
    #[arg(long = "punctuation-model-id", help_heading = "Model Options")]
    pub(crate) punctuation_model_id: Option<String>,

    #[command(flatten)]
    pub(crate) online: crate::online_asr::OnlineAsrArgs,

    /// Number of recognition threads (default: system auto-configured).
    #[arg(long, help_heading = "Audio & Performance")]
    pub(crate) threads: Option<i32>,
    /// Enable inverse text normalization (convert spoken numbers/dates to digits, e.g. "一百二十" -> "120").
    #[arg(long, default_value_t = false, help_heading = "Audio & Performance")]
    pub(crate) enable_itn: bool,
    /// Optional hotwords string to enhance recognition, separated by newlines or commas.
    #[arg(long, help_heading = "Audio & Performance")]
    pub(crate) hotwords: Option<String>,
    /// GPU acceleration mode: auto, cpu, vulkan, metal, or cuda.
    #[arg(
        long = "gpu-acceleration",
        value_name = "MODE",
        value_parser = crate::runtime::gpu_acceleration_value_parser(),
        help_heading = "Audio & Performance"
    )]
    pub(crate) gpu_acceleration: Option<String>,
    /// VAD buffer size in seconds, for example 0.5.
    #[arg(long = "vad-buffer", help_heading = "Audio & Performance")]
    pub(crate) vad_buffer: Option<f32>,
    /// Only transcribe complete sentences upon VAD truncation (reduces compute/battery load).
    #[arg(
        long = "sentence-only",
        default_value_t = false,
        help = "Only transcribe complete sentences upon VAD truncation (reduces compute/battery load)",
        help_heading = "Audio & Performance"
    )]
    pub(crate) sentence_only: bool,

    /// Optional config file, usually sona-cli.toml.
    #[arg(short, long, value_name = "FILE")]
    pub(crate) config: Option<PathBuf>,

    /// List microphone input devices and exit.
    #[arg(
        long,
        default_value_t = false,
        hide = true,
        conflicts_with = "list_providers"
    )]
    pub(crate) list_input_devices: bool,
    /// List available online ASR providers and exit.
    #[arg(
        long,
        default_value_t = false,
        hide = true,
        conflicts_with = "list_input_devices"
    )]
    pub(crate) list_providers: bool,
}
pub(crate) struct ResolvedLiveCommand {
    pub(crate) input: LiveInputSource,
    pub(crate) device: Option<String>,
    pub(crate) duration: Option<Duration>,
    pub(crate) output_format: LiveOutputFormat,
    pub(crate) export_mode: sona_core::export::ExportMode,
    pub(crate) asr: ResolvedLiveAsr,
    pub(crate) save_audio: Option<PathBuf>,
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
    stdin_is_terminal: bool,
) -> CliResult<ResolvedLiveCommand> {
    let config = config.unwrap_or_default();
    let export_mode = sona_core::export::ExportMode::parse(&args.mode)
        .map_err(|error| CliError::Validation(error.to_string()))?;
    let input = match args.input {
        Some(input) => input,
        None => {
            if args.device.is_some() {
                LiveInputSource::Microphone
            } else if let Some(cfg_input) = config.input.as_deref() {
                LiveInputSource::parse_config(cfg_input)?
            } else if !stdin_is_terminal {
                LiveInputSource::Stdin
            } else {
                LiveInputSource::Microphone
            }
        }
    };
    let device = args.device.clone().or(config.device.clone());
    let duration_seconds = args.duration.or(config.duration_seconds);
    validate_direct_input_options(Some(input), device.as_deref(), duration_seconds)?;
    let duration = duration_seconds.map(Duration::from_secs_f64);
    let save_audio = args.save_audio.clone().or_else(|| config.save_wav.clone());
    let output_format = match args.stream {
        Some(format) => format,
        None => config
            .stream_format
            .as_deref()
            .or(config.output_format.as_deref())
            .map(LiveOutputFormatArg::parse_config)
            .transpose()?
            .unwrap_or(LiveOutputFormatArg::Text),
    };
    let mut resolved_online = args.online.resolve_with_config(
        config.online_provider.clone(),
        config.api_key_env.clone(),
        config.online_config.clone(),
    )?;

    if let (true, Some(cli_model)) = (resolved_online.is_online(), &args.model_id) {
        if let Some(existing_online_model) = &resolved_online.online_model {
            if existing_online_model != cli_model {
                return Err(CliError::Validation(format!(
                    "Conflicting model names specified: -m/--model '{cli_model}' vs --online-model '{existing_online_model}'."
                )));
            }
        } else {
            resolved_online.online_model = Some(cli_model.clone());
        }
    }
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
        let format_choice = args.format.as_deref().or(config.format.as_deref());
        let export_format = args
            .output
            .as_deref()
            .map(|path| {
                sona_core::transcription::runtime::resolve_export_format(format_choice, Some(path))
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
        let mut resolved_model_id = args.model_id.clone();
        if resolved_model_id.is_none() && config.model_id.is_none() {
            match infer_streaming_model(args.models_dir.as_ref()) {
                StreamingModelInference::AutoSelected(inferred) => {
                    eprintln!(
                        "Note: Automatically selected installed streaming model '{inferred}'"
                    );
                    resolved_model_id = Some(inferred);
                }
                StreamingModelInference::MultipleInstalled(names) => {
                    return Err(CliError::Validation(format!(
                        "Multiple streaming models installed ([{}]). Specify one with -m/--model <MODEL_ID>, or configure model_id in sona-cli.toml.",
                        names.join(", ")
                    )));
                }
                StreamingModelInference::NoneInstalled => {}
            }
        }
        let plan = sona_runtime_fs::resolve_live_transcribe_plan_with_runtime_paths(
            LiveTranscribeOptions {
                output: args.output,
                format: args.format.or(config.format.clone()),
                model_id: resolved_model_id,
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
                enable_partial_decoding: args.sentence_only.then_some(false),
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
        export_mode,
        asr,
        save_audio,
    })
}

fn reject_online_local_options(args: &TranscribeLiveArgs) -> CliResult<()> {
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

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum StreamingModelInference {
    AutoSelected(String),
    MultipleInstalled(Vec<String>),
    NoneInstalled,
}

pub(crate) fn infer_streaming_model_from_summaries(
    models: &[sona_core::models::catalog::ModelSummary],
) -> StreamingModelInference {
    let installed_streaming: Vec<_> = models
        .iter()
        .filter(|m| {
            m.installed
                && m.modes
                    .iter()
                    .any(|mode| mode == "streaming" || mode == "live")
        })
        .collect();

    if installed_streaming.is_empty() {
        StreamingModelInference::NoneInstalled
    } else if installed_streaming.len() == 1 {
        StreamingModelInference::AutoSelected(installed_streaming[0].id.clone())
    } else {
        use sona_core::models::preset_models::DEFAULT_SENSEVOICE_INT8_MODEL_ID;
        if let Some(preferred) = installed_streaming
            .iter()
            .find(|m| m.id == DEFAULT_SENSEVOICE_INT8_MODEL_ID)
        {
            StreamingModelInference::AutoSelected(preferred.id.clone())
        } else {
            let names = installed_streaming
                .into_iter()
                .map(|m| m.id.clone())
                .collect();
            StreamingModelInference::MultipleInstalled(names)
        }
    }
}

#[cfg(test)]
pub(crate) fn select_single_streaming_model(
    models: &[sona_core::models::catalog::ModelSummary],
) -> Option<String> {
    match infer_streaming_model_from_summaries(models) {
        StreamingModelInference::AutoSelected(id) => Some(id),
        StreamingModelInference::MultipleInstalled(_) | StreamingModelInference::NoneInstalled => {
            None
        }
    }
}

fn infer_streaming_model(models_dir: Option<&PathBuf>) -> StreamingModelInference {
    let Ok(resolved_dir) = sona_core::models::paths::resolve_models_dir(
        models_dir.cloned(),
        crate::desktop_paths::default_models_dir(),
        crate::desktop_paths::models_dir_status,
    ) else {
        return StreamingModelInference::NoneInstalled;
    };
    let all_models = sona_runtime_fs::list_models(&resolved_dir);
    infer_streaming_model_from_summaries(&all_models)
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

    #[test]
    fn select_single_streaming_model_behavior() {
        use sona_core::models::catalog::ModelSummary;
        use sona_core::models::preset_models::LanguageMode;

        let make_summary = |id: &str, modes: &[&str], installed: bool| ModelSummary {
            id: id.to_string(),
            name: id.to_string(),
            model_type: "asr".to_string(),
            languages: vec!["en".to_string()],
            language_mode: LanguageMode::Selectable,
            size: "100MB".to_string(),
            modes: modes.iter().map(|s| s.to_string()).collect(),
            installed,
            install_path: PathBuf::from(format!("/models/{id}")),
        };

        // 1. Empty list -> None
        assert_eq!(select_single_streaming_model(&[]), None);

        // 2. Installed batch-only model -> None
        let batch_model = make_summary("whisper-turbo", &["batch"], true);
        assert_eq!(
            select_single_streaming_model(std::slice::from_ref(&batch_model)),
            None
        );

        use sona_core::models::preset_models::DEFAULT_SENSEVOICE_INT8_MODEL_ID;

        // 3. Uninstalled streaming model -> None
        let uninstalled_streaming = make_summary(
            DEFAULT_SENSEVOICE_INT8_MODEL_ID,
            &["streaming", "batch"],
            false,
        );
        assert_eq!(
            select_single_streaming_model(std::slice::from_ref(&uninstalled_streaming)),
            None
        );

        // 4. Exactly one installed streaming model -> Some(DEFAULT_SENSEVOICE_INT8_MODEL_ID)
        let installed_streaming = make_summary(
            DEFAULT_SENSEVOICE_INT8_MODEL_ID,
            &["streaming", "batch"],
            true,
        );
        assert_eq!(
            select_single_streaming_model(&[batch_model.clone(), installed_streaming.clone()]),
            Some(DEFAULT_SENSEVOICE_INT8_MODEL_ID.to_string())
        );

        // 5. Multiple installed streaming models with default sensevoice -> AutoSelected(DEFAULT_SENSEVOICE_INT8_MODEL_ID)
        let second_streaming = make_summary("paraformer", &["live"], true);
        assert_eq!(
            infer_streaming_model_from_summaries(&[
                installed_streaming.clone(),
                second_streaming.clone()
            ]),
            StreamingModelInference::AutoSelected(DEFAULT_SENSEVOICE_INT8_MODEL_ID.to_string())
        );
        assert_eq!(
            select_single_streaming_model(&[installed_streaming, second_streaming.clone()]),
            Some(DEFAULT_SENSEVOICE_INT8_MODEL_ID.to_string())
        );

        // 6. Multiple installed streaming models without default -> None (ambiguous)
        let third_streaming = make_summary("zipformer", &["streaming"], true);
        assert_eq!(
            infer_streaming_model_from_summaries(&[
                second_streaming.clone(),
                third_streaming.clone()
            ]),
            StreamingModelInference::MultipleInstalled(vec![
                "paraformer".to_string(),
                "zipformer".to_string()
            ])
        );
        assert_eq!(
            select_single_streaming_model(&[second_streaming, third_streaming]),
            None
        );
    }

    #[test]
    fn device_flag_overrides_config_input_stdin() {
        use clap::Parser;
        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            args: TranscribeLiveArgs,
        }
        let parsed = TestCli::try_parse_from([
            "live",
            "--device",
            "Test Mic",
            "--online-provider",
            "volcengine-doubao",
            "--api-key",
            "test-key",
        ])
        .unwrap();
        let config = TranscribeLiveConfigSection {
            input: Some("stdin".to_string()),
            ..Default::default()
        };
        let resolved = resolve_live_command(parsed.args, Some(config), true).unwrap();
        assert_eq!(resolved.input, LiveInputSource::Microphone);
        assert_eq!(resolved.device.as_deref(), Some("Test Mic"));
    }
}
