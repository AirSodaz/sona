use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use clap::{ArgGroup, Args, Subcommand, ValueEnum};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use sona_core::llm::provider_protocol::LlmModelSummary;
#[cfg(test)]
use sona_core::llm::requests::LlmConfig;
use sona_core::llm::requests::{
    LlmModelsRequest, PolishSegmentsRequest, SummarizeTranscriptRequest, TranslateSegmentsRequest,
};
use sona_core::llm::runtime::{
    LlmCapabilityPolicy, LlmCompletionOptions, LlmCompletionRequest, LlmCompletionResponse,
    LlmPromptCachePolicy, LlmResponseFormat, LlmRuntimeError, LlmRuntimeService,
};
use sona_core::llm::tasks::{
    LlmTaskError, LlmTaskService, PolishedSegment, TranscriptSummaryResult, TranslatedSegment,
};
use sona_core::llm::usage::LlmGenerateSource;
use sona_core::ports::llm::{
    LlmCompletionPort, LlmModelDiscoveryPort, LlmModelMetadataPort, LlmPortErrorKind,
    LlmTaskRuntimePort,
};
use sona_online_llm::OnlineLlmAdapter;

use crate::table::{append_table_row, append_table_separator, column_widths, sanitize_table_cell};
use crate::{CliError, CliOutput, CliResult};

#[derive(Debug, Args)]
pub struct LlmArgs {
    #[command(subcommand)]
    command: LlmCommands,
}

#[derive(Debug, Subcommand)]
enum LlmCommands {
    /// Generates text or structured output.
    Generate(LlmGenerateArgs),
    /// Lists models available from the configured provider.
    Models(LlmModelsArgs),
    /// Runs the shared core polish task.
    Polish(LlmTaskArgs),
    /// Runs the shared core translation task.
    Translate(LlmTaskArgs),
    /// Runs the shared core transcript summary task.
    Summary(LlmTaskArgs),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("input")
        .required(true)
        .multiple(false)
        .args(["text", "file", "stdin"])
))]
#[command(group(
    ArgGroup::new("system_prompt")
        .multiple(false)
        .args(["system_prompt", "system_prompt_file"])
))]
struct LlmGenerateArgs {
    /// JSON file containing an LlmConfig object.
    #[arg(long, value_name = "FILE")]
    config_json: PathBuf,
    /// Environment variable whose value overrides apiKey in the config JSON.
    #[arg(long, value_name = "NAME")]
    api_key_env: Option<String>,
    /// Uses this literal text as input.
    #[arg(long)]
    text: Option<String>,
    /// Reads input text from a UTF-8 file.
    #[arg(long, value_name = "FILE")]
    file: Option<PathBuf>,
    /// Reads input text from standard input.
    #[arg(long)]
    stdin: bool,
    /// Optional stable system prompt.
    #[arg(long)]
    system_prompt: Option<String>,
    /// Reads the system prompt from a UTF-8 file.
    #[arg(long, value_name = "FILE")]
    system_prompt_file: Option<PathBuf>,
    /// Requested response format.
    #[arg(long, alias = "format", value_enum, default_value_t = ResponseFormatArg::Text)]
    response_format: ResponseFormatArg,
    /// JSON Schema file required by json-schema mode.
    #[arg(long, value_name = "FILE")]
    schema: Option<PathBuf>,
    /// Name included with a JSON Schema response format.
    #[arg(long, default_value = "response")]
    schema_name: String,
    /// Provider-managed prompt cache policy.
    #[arg(long, value_enum, default_value_t = CacheArg::Disabled)]
    cache: CacheArg,
    /// Behavior when model metadata reports an unsupported capability.
    #[arg(long, value_enum, default_value_t = CapabilityArg::Compatible)]
    capability: CapabilityArg,
    /// Request-level temperature override.
    #[arg(long)]
    temperature: Option<f32>,
    /// Request-level maximum output token count.
    #[arg(long)]
    max_output_tokens: Option<u64>,
    /// Enables or disables reasoning without changing the persisted config.
    #[arg(long, value_name = "BOOL")]
    reasoning: Option<bool>,
    /// Request-level reasoning effort or level.
    #[arg(long)]
    reasoning_level: Option<String>,
    /// Terminal output format.
    #[arg(long, value_enum, default_value_t = OutputArg::Text)]
    output: OutputArg,
}

#[derive(Debug, Args)]
struct LlmModelsArgs {
    /// JSON file containing provider, baseUrl, apiKey and optional strategy.
    #[arg(long, value_name = "FILE")]
    config_json: PathBuf,
    /// Environment variable whose value overrides apiKey in the config JSON.
    #[arg(long, value_name = "NAME")]
    api_key_env: Option<String>,
    /// Terminal output format.
    #[arg(long, value_enum, default_value_t = OutputArg::Text)]
    output: OutputArg,
}

#[derive(Debug, Args)]
struct LlmTaskArgs {
    /// JSON file containing the core task request object.
    #[arg(long, value_name = "FILE")]
    request_json: PathBuf,
    /// Environment variable whose value overrides config.apiKey in the request JSON.
    #[arg(long, value_name = "NAME")]
    api_key_env: Option<String>,
    /// Terminal output format.
    #[arg(long, value_enum, default_value_t = OutputArg::Json)]
    output: OutputArg,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ResponseFormatArg {
    Text,
    JsonObject,
    JsonSchema,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CacheArg {
    Disabled,
    Automatic,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum CapabilityArg {
    Compatible,
    Strict,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum OutputArg {
    Text,
    Json,
}

pub fn run_llm(args: LlmArgs) -> CliResult<CliOutput> {
    match args.command {
        LlmCommands::Generate(args) => {
            let output = args.output;
            let request = build_generate_request(args)?;
            run_async(move || async move {
                let adapter = OnlineLlmAdapter;
                let response = execute_generate(request, &adapter, adapter).await?;
                render_completion(response, output)
            })
        }
        LlmCommands::Models(args) => {
            let request = load_json_config(&args.config_json, args.api_key_env.as_deref())?;
            run_async(move || async move {
                let adapter = OnlineLlmAdapter;
                let models = execute_models(request, &adapter, adapter).await?;
                render_models(&models, args.output)
            })
        }
        LlmCommands::Polish(args) => {
            let output = args.output;
            let request = load_task_request(&args.request_json, args.api_key_env.as_deref())?;
            run_async(move || async move {
                let result = execute_polish(request, OnlineLlmAdapter).await?;
                render_polished_segments(&result, output)
            })
        }
        LlmCommands::Translate(args) => {
            let output = args.output;
            let request = load_task_request(&args.request_json, args.api_key_env.as_deref())?;
            run_async(move || async move {
                let result = execute_translate(request, OnlineLlmAdapter).await?;
                render_translated_segments(&result, output)
            })
        }
        LlmCommands::Summary(args) => {
            let output = args.output;
            let request = load_task_request(&args.request_json, args.api_key_env.as_deref())?;
            run_async(move || async move {
                let result = execute_summary(request, OnlineLlmAdapter).await?;
                render_summary_result(&result, output)
            })
        }
    }
}

fn build_generate_request(args: LlmGenerateArgs) -> CliResult<LlmCompletionRequest> {
    let config = load_json_config(&args.config_json, args.api_key_env.as_deref())?;
    let input = read_selected_text(args.text.as_deref(), args.file.as_deref(), args.stdin)?;
    let system_prompt = match (args.system_prompt, args.system_prompt_file) {
        (Some(prompt), None) => Some(prompt),
        (None, Some(path)) => Some(read_utf8_file(&path, "system prompt")?),
        (None, None) => None,
        (Some(_), Some(_)) => unreachable!("clap enforces the system prompt group"),
    };
    let response_format = match args.response_format {
        ResponseFormatArg::Text => {
            reject_schema(
                &args.schema,
                "--schema requires --response-format json-schema",
            )?;
            LlmResponseFormat::Text
        }
        ResponseFormatArg::JsonObject => {
            reject_schema(
                &args.schema,
                "--schema requires --response-format json-schema",
            )?;
            LlmResponseFormat::JsonObject
        }
        ResponseFormatArg::JsonSchema => {
            let path = args.schema.as_deref().ok_or_else(|| {
                CliError::Validation(
                    "--schema is required with --response-format json-schema".to_string(),
                )
            })?;
            let schema =
                serde_json::from_str(&read_utf8_file(path, "JSON Schema")?).map_err(|error| {
                    CliError::Validation(format!("Invalid JSON Schema JSON: {error}"))
                })?;
            LlmResponseFormat::JsonSchema {
                name: args.schema_name,
                schema,
            }
        }
    };

    Ok(LlmCompletionRequest {
        config,
        system_prompt,
        input,
        options: LlmCompletionOptions {
            temperature: args.temperature,
            max_output_tokens: args.max_output_tokens,
            reasoning_enabled: args.reasoning,
            reasoning_level: args.reasoning_level,
            response_format,
            prompt_cache: match args.cache {
                CacheArg::Disabled => LlmPromptCachePolicy::Disabled,
                CacheArg::Automatic => LlmPromptCachePolicy::Automatic,
            },
            capability_policy: match args.capability {
                CapabilityArg::Compatible => LlmCapabilityPolicy::Compatible,
                CapabilityArg::Strict => LlmCapabilityPolicy::Strict,
            },
        },
        source: Some(LlmGenerateSource::Generic),
    })
}

async fn execute_generate<C, M>(
    request: LlmCompletionRequest,
    completion: &C,
    metadata: M,
) -> CliResult<LlmCompletionResponse>
where
    C: LlmCompletionPort,
    M: LlmModelMetadataPort,
{
    LlmRuntimeService::new(completion, metadata)
        .complete(request)
        .await
        .map_err(map_runtime_error)
}

async fn execute_models<C, M>(
    request: LlmModelsRequest,
    completion: &C,
    metadata: M,
) -> CliResult<Vec<LlmModelSummary>>
where
    C: LlmCompletionPort + LlmModelDiscoveryPort,
    M: LlmModelMetadataPort,
{
    LlmRuntimeService::new(completion, metadata)
        .list_models(request)
        .await
        .map_err(map_runtime_error)
}

async fn execute_polish<R>(
    request: PolishSegmentsRequest,
    runtime: R,
) -> CliResult<Vec<PolishedSegment>>
where
    R: LlmTaskRuntimePort,
{
    LlmTaskService::new(runtime)
        .polish(request, &())
        .await
        .map_err(map_task_error)
}

async fn execute_translate<R>(
    request: TranslateSegmentsRequest,
    runtime: R,
) -> CliResult<Vec<TranslatedSegment>>
where
    R: LlmTaskRuntimePort,
{
    LlmTaskService::new(runtime)
        .translate(request, &())
        .await
        .map_err(map_task_error)
}

async fn execute_summary<R>(
    request: SummarizeTranscriptRequest,
    runtime: R,
) -> CliResult<TranscriptSummaryResult>
where
    R: LlmTaskRuntimePort,
{
    LlmTaskService::new(runtime)
        .summarize(request, &())
        .await
        .map_err(map_task_error)
}

fn load_json_config<T: DeserializeOwned>(path: &Path, api_key_env: Option<&str>) -> CliResult<T> {
    let mut value: Value = serde_json::from_str(&read_utf8_file(path, "LLM config")?)
        .map_err(|error| CliError::Validation(format!("Invalid LLM config JSON: {error}")))?;
    if let Some(name) = api_key_env {
        let api_key = std::env::var(name).map_err(|_| {
            CliError::Validation(format!("Environment variable '{name}' is not set"))
        })?;
        let object = value
            .as_object_mut()
            .ok_or_else(|| CliError::Validation("LLM config JSON must be an object".to_string()))?;
        object.insert("apiKey".to_string(), Value::String(api_key));
    }
    serde_json::from_value(value)
        .map_err(|error| CliError::Validation(format!("Invalid LLM config: {error}")))
}

fn load_task_request<T: DeserializeOwned>(path: &Path, api_key_env: Option<&str>) -> CliResult<T> {
    let mut value: Value = serde_json::from_str(&read_utf8_file(path, "LLM task request")?)
        .map_err(|error| CliError::Validation(format!("Invalid LLM task request JSON: {error}")))?;
    if let Some(name) = api_key_env {
        let api_key = std::env::var(name).map_err(|_| {
            CliError::Validation(format!("Environment variable '{name}' is not set"))
        })?;
        override_task_request_api_key(&mut value, api_key)?;
    }
    serde_json::from_value(value)
        .map_err(|error| CliError::Validation(format!("Invalid LLM task request: {error}")))
}

fn override_task_request_api_key(value: &mut Value, api_key: String) -> CliResult<()> {
    let request = value.as_object_mut().ok_or_else(|| {
        CliError::Validation("LLM task request JSON must be an object".to_string())
    })?;
    let config = request
        .get_mut("config")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| {
            CliError::Validation("LLM task request JSON must contain a config object".to_string())
        })?;
    config.insert("apiKey".to_string(), Value::String(api_key));
    Ok(())
}

fn read_selected_text(text: Option<&str>, file: Option<&Path>, stdin: bool) -> CliResult<String> {
    let value = if let Some(text) = text {
        text.to_string()
    } else if let Some(path) = file {
        read_utf8_file(path, "input")?
    } else if stdin {
        let mut value = String::new();
        io::stdin()
            .read_to_string(&mut value)
            .map_err(|error| CliError::Io(format!("Failed to read stdin: {error}")))?;
        value
    } else {
        unreachable!("clap enforces the input group")
    };
    if value.trim().is_empty() {
        return Err(CliError::Validation(
            "LLM input cannot be empty".to_string(),
        ));
    }
    Ok(value)
}

fn read_utf8_file(path: &Path, label: &str) -> CliResult<String> {
    fs::read_to_string(path).map_err(|error| {
        CliError::Io(format!(
            "Failed to read {label} '{}': {error}",
            path.display()
        ))
    })
}

fn reject_schema(schema: &Option<PathBuf>, message: &str) -> CliResult<()> {
    if schema.is_some() {
        Err(CliError::Validation(message.to_string()))
    } else {
        Ok(())
    }
}

fn render_completion(response: LlmCompletionResponse, output: OutputArg) -> CliResult<CliOutput> {
    match output {
        OutputArg::Text => Ok(CliOutput::stdout(response.text)),
        OutputArg::Json => serde_json::to_string_pretty(&response)
            .map(CliOutput::stdout)
            .map_err(|error| CliError::Serialize(error.to_string())),
    }
}

fn render_models(models: &[LlmModelSummary], output: OutputArg) -> CliResult<CliOutput> {
    if matches!(output, OutputArg::Json) {
        return serde_json::to_string_pretty(models)
            .map(CliOutput::stdout)
            .map_err(|error| CliError::Serialize(error.to_string()));
    }

    let headers = [
        "MODEL",
        "NAME",
        "CONTEXT",
        "MAX_OUTPUT",
        "INPUT",
        "OUTPUT",
        "STRUCTURED",
        "CACHE",
    ];
    let rows = models
        .iter()
        .map(|model| {
            [
                sanitize_table_cell(&model.model),
                sanitize_table_cell(model.display_name.as_deref().unwrap_or("")),
                optional_number(model.context_window),
                optional_number(model.max_output_tokens),
                modalities(&model.input_modalities),
                modalities(&model.output_modalities),
                optional_bool(model.supports_structured_output),
                optional_bool(model.supports_prompt_caching),
            ]
        })
        .collect::<Vec<_>>();
    let widths = column_widths(&headers, &rows);
    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in &rows {
        let values = std::array::from_fn(|index| row[index].as_str());
        append_table_row(&mut output, &values, &widths);
    }
    Ok(CliOutput::stdout(output))
}

fn render_polished_segments(
    segments: &[PolishedSegment],
    output: OutputArg,
) -> CliResult<CliOutput> {
    match output {
        OutputArg::Json => render_json(segments),
        OutputArg::Text => render_task_segments_table(
            ["ID", "TEXT"],
            segments
                .iter()
                .map(|segment| [segment.id.as_str(), segment.text.as_str()]),
        ),
    }
}

fn render_translated_segments(
    segments: &[TranslatedSegment],
    output: OutputArg,
) -> CliResult<CliOutput> {
    match output {
        OutputArg::Json => render_json(segments),
        OutputArg::Text => render_task_segments_table(
            ["ID", "TRANSLATION"],
            segments
                .iter()
                .map(|segment| [segment.id.as_str(), segment.translation.as_str()]),
        ),
    }
}

fn render_summary_result(
    result: &TranscriptSummaryResult,
    output: OutputArg,
) -> CliResult<CliOutput> {
    match output {
        OutputArg::Json => render_json(result),
        OutputArg::Text => Ok(CliOutput::stdout(result.content.clone())),
    }
}

fn render_task_segments_table<'a>(
    headers: [&str; 2],
    rows: impl IntoIterator<Item = [&'a str; 2]>,
) -> CliResult<CliOutput> {
    let rows = rows
        .into_iter()
        .map(|row| row.map(sanitize_table_cell))
        .collect::<Vec<_>>();
    let widths = column_widths(&headers, &rows);
    let mut output = String::new();
    append_table_row(&mut output, &headers, &widths);
    append_table_separator(&mut output, &widths);
    for row in &rows {
        append_table_row(&mut output, &[row[0].as_str(), row[1].as_str()], &widths);
    }
    Ok(CliOutput::stdout(output))
}

fn render_json(value: impl Serialize) -> CliResult<CliOutput> {
    serde_json::to_string_pretty(&value)
        .map(CliOutput::stdout)
        .map_err(|error| CliError::Serialize(error.to_string()))
}

fn optional_number(value: Option<u64>) -> String {
    value.map(|value| value.to_string()).unwrap_or_default()
}

fn optional_bool(value: Option<bool>) -> String {
    value
        .map(|value| if value { "yes" } else { "no" }.to_string())
        .unwrap_or_default()
}

fn modalities(values: &[sona_core::llm::provider_protocol::LlmModality]) -> String {
    values
        .iter()
        .map(|value| format!("{value:?}").to_ascii_lowercase())
        .collect::<Vec<_>>()
        .join(",")
}

fn map_runtime_error(error: LlmRuntimeError) -> CliError {
    let message = error.to_string();
    match error {
        LlmRuntimeError::InvalidRequest { reason } => CliError::Validation(reason),
        error => map_runtime_error_with_message(error, message),
    }
}

fn map_task_error(error: LlmTaskError) -> CliError {
    let message = error.to_string();
    match error {
        LlmTaskError::InvalidRequest { reason } => CliError::Validation(reason),
        LlmTaskError::InvalidResponse { .. } => CliError::Model(message),
        LlmTaskError::Observer { .. } => CliError::Other(message),
        LlmTaskError::Runtime { source, .. } => map_runtime_error_with_message(source, message),
    }
}

fn map_runtime_error_with_message(error: LlmRuntimeError, message: String) -> CliError {
    match error {
        LlmRuntimeError::InvalidRequest { .. } => CliError::Validation(message),
        LlmRuntimeError::UnsupportedCapability { .. } | LlmRuntimeError::InvalidResponse { .. } => {
            CliError::Model(message)
        }
        LlmRuntimeError::Adapter { kind, .. } => match kind {
            LlmPortErrorKind::InvalidRequest => CliError::Validation(message),
            LlmPortErrorKind::RateLimited
            | LlmPortErrorKind::Timeout
            | LlmPortErrorKind::Unavailable
            | LlmPortErrorKind::Network => CliError::Network(message),
            LlmPortErrorKind::Authentication
            | LlmPortErrorKind::Permission
            | LlmPortErrorKind::Unsupported
            | LlmPortErrorKind::Protocol => CliError::Model(message),
        },
    }
}

fn run_async<F, Fut>(factory: F) -> CliResult<CliOutput>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = CliResult<CliOutput>>,
{
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| CliError::Io(format!("Failed to create async runtime: {error}")))?
        .block_on(factory())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use sona_core::domain::{BuiltinLlmProvider, LlmProvider};
    use sona_core::llm::provider_protocol::StandardLlmResponse;
    use sona_core::llm::tasks::LlmProviderStrategy;
    use sona_core::ports::llm::LlmPortError;

    use super::*;

    #[derive(Clone, Default)]
    struct FakeLlm {
        completion_request: Arc<Mutex<Option<LlmCompletionRequest>>>,
        models_request: Arc<Mutex<Option<LlmModelsRequest>>>,
    }

    #[async_trait]
    impl LlmCompletionPort for FakeLlm {
        async fn complete(
            &self,
            request: LlmCompletionRequest,
        ) -> Result<StandardLlmResponse, LlmPortError> {
            *self.completion_request.lock().unwrap() = Some(request);
            Ok(StandardLlmResponse {
                text: r#"{"ok":true}"#.to_string(),
                usage: None,
            })
        }
    }

    #[async_trait]
    impl LlmModelMetadataPort for FakeLlm {
        async fn describe_model(
            &self,
            _config: &LlmConfig,
        ) -> Result<Option<LlmModelSummary>, LlmPortError> {
            Ok(Some(LlmModelSummary {
                model: "test-model".to_string(),
                supports_structured_output: Some(true),
                ..LlmModelSummary::default()
            }))
        }
    }

    #[async_trait]
    impl LlmModelDiscoveryPort for FakeLlm {
        async fn list_models(
            &self,
            request: LlmModelsRequest,
        ) -> Result<Vec<LlmModelSummary>, LlmPortError> {
            *self.models_request.lock().unwrap() = Some(request);
            Ok(vec![
                LlmModelSummary {
                    model: "first".to_string(),
                    ..LlmModelSummary::default()
                },
                LlmModelSummary {
                    model: "second".to_string(),
                    max_output_tokens: Some(4096),
                    ..LlmModelSummary::default()
                },
            ])
        }
    }

    fn config() -> LlmConfig {
        LlmConfig {
            provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
            strategy: LlmProviderStrategy::OpenAi,
            base_url: "https://api.example.com".to_string(),
            api_key: "secret".to_string(),
            model: "test-model".to_string(),
            api_path: None,
            api_version: None,
            temperature: None,
            reasoning_enabled: None,
            reasoning_level: None,
            timeout_seconds: None,
        }
    }

    #[tokio::test]
    async fn generate_delegates_structured_request_and_keeps_execution_metadata() {
        let fake = FakeLlm::default();
        let request = LlmCompletionRequest {
            config: config(),
            system_prompt: Some("stable".to_string()),
            input: "hello".to_string(),
            options: LlmCompletionOptions {
                response_format: LlmResponseFormat::JsonObject,
                prompt_cache: LlmPromptCachePolicy::Automatic,
                ..LlmCompletionOptions::default()
            },
            source: Some(LlmGenerateSource::Generic),
        };

        let response = execute_generate(request, &fake, fake.clone())
            .await
            .unwrap();
        let captured = fake.completion_request.lock().unwrap();

        assert_eq!(
            captured.as_ref().unwrap().system_prompt.as_deref(),
            Some("stable")
        );
        assert_eq!(response.json, Some(serde_json::json!({"ok": true})));
        assert_eq!(response.execution.attempts, 1);
    }

    #[tokio::test]
    async fn models_preserve_provider_order_and_metadata() {
        let fake = FakeLlm::default();
        let models = execute_models(
            LlmModelsRequest {
                provider: LlmProvider::Builtin(BuiltinLlmProvider::OpenAi),
                strategy: Some(LlmProviderStrategy::OpenAi),
                base_url: "https://api.example.com".to_string(),
                api_key: "secret".to_string(),
            },
            &fake,
            fake.clone(),
        )
        .await
        .unwrap();

        assert_eq!(
            models
                .iter()
                .map(|model| model.model.as_str())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
        assert_eq!(models[1].max_output_tokens, Some(4096));
        assert!(fake.models_request.lock().unwrap().is_some());
    }

    #[test]
    fn task_subcommands_are_recognized_by_the_cli_parser() {
        for command in ["polish", "translate", "summary"] {
            let error = crate::run_cli_from_args([
                "sona-cli",
                "llm",
                command,
                "--request-json",
                "missing-request.json",
            ])
            .expect_err("the request file should fail after parsing");

            assert!(
                !matches!(error, crate::CliError::Usage(_)),
                "{command} was rejected as an unknown CLI subcommand: {error}"
            );
        }
    }

    #[test]
    fn task_request_loading_reads_nested_config_api_key() {
        let directory = tempfile::tempdir().unwrap();
        let request_path = directory.path().join("polish-request.json");
        std::fs::write(
            &request_path,
            r#"{
                "taskId": "polish-1",
                "config": {
                    "provider": "open_ai",
                    "baseUrl": "https://api.example.com",
                    "apiKey": "nested-secret",
                    "model": "test-model"
                },
                "segments": [{"id": "segment-1", "text": "hello"}]
            }"#,
        )
        .unwrap();

        let request: PolishSegmentsRequest = load_task_request(&request_path, None).unwrap();

        assert_eq!(request.config.api_key, "nested-secret");
        assert_eq!(request.segments[0].id, "segment-1");
    }

    #[test]
    fn task_request_api_key_override_updates_nested_config() {
        let mut request = serde_json::json!({
            "config": {
                "apiKey": "request-secret"
            }
        });

        override_task_request_api_key(&mut request, "environment-secret".to_string()).unwrap();

        assert_eq!(request["config"]["apiKey"], "environment-secret");
        assert!(request.get("apiKey").is_none());
    }

    #[test]
    fn segment_task_results_render_as_json_and_text() {
        let polished = vec![PolishedSegment {
            id: "segment-1".to_string(),
            text: "hello\nworld".to_string(),
        }];
        let polished_json = render_polished_segments(&polished, OutputArg::Json).unwrap();
        let polished_text = render_polished_segments(&polished, OutputArg::Text).unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(&polished_json.stdout).unwrap(),
            serde_json::json!([{"id": "segment-1", "text": "hello\nworld"}])
        );
        assert!(polished_text.stdout.contains("ID"));
        assert!(polished_text.stdout.contains("TEXT"));
        assert!(polished_text.stdout.contains("segment-1"));
        assert!(polished_text.stdout.contains(r"hello\nworld"));

        let translated = vec![TranslatedSegment {
            id: "segment-2".to_string(),
            translation: "bonjour".to_string(),
        }];
        let translated_json = render_translated_segments(&translated, OutputArg::Json).unwrap();
        let translated_text = render_translated_segments(&translated, OutputArg::Text).unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(&translated_json.stdout).unwrap(),
            serde_json::json!([{"id": "segment-2", "translation": "bonjour"}])
        );
        assert!(translated_text.stdout.contains("TRANSLATION"));
        assert!(translated_text.stdout.contains("bonjour"));
    }

    #[test]
    fn summary_task_result_renders_as_json_and_text() {
        let result = TranscriptSummaryResult {
            template_id: "general".to_string(),
            content: "Summary content".to_string(),
        };
        let json = render_summary_result(&result, OutputArg::Json).unwrap();
        let text = render_summary_result(&result, OutputArg::Text).unwrap();

        assert_eq!(
            serde_json::from_str::<Value>(&json.stdout).unwrap(),
            serde_json::json!({
                "templateId": "general",
                "content": "Summary content"
            })
        );
        assert_eq!(text.stdout, "Summary content");
    }
}
