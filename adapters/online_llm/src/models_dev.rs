use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use serde::Deserialize;
use sona_core::domain::LlmProvider;
use sona_core::llm::model_catalog::merge_model_metadata;
use sona_core::llm::provider_protocol::{LlmModality, LlmModelMetadataSource, LlmModelSummary};
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};

use crate::transport::{http_status_port_error, is_local_or_lan_host, reqwest_port_error};

const MODELS_DEV_ENDPOINT: &str = "https://models.dev/api.json";
const MODELS_DEV_CACHE_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const MODELS_DEV_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct ModelsDevCatalog {
    endpoint: String,
    client: reqwest::Client,
    cache: Arc<RwLock<Option<CachedCatalog>>>,
}

#[derive(Clone)]
struct CachedCatalog {
    fetched_at: Instant,
    body: String,
}

impl Default for ModelsDevCatalog {
    fn default() -> Self {
        Self::with_endpoint(MODELS_DEV_ENDPOINT, MODELS_DEV_TIMEOUT)
    }
}

impl ModelsDevCatalog {
    pub fn with_endpoint(endpoint: impl Into<String>, timeout: Duration) -> Self {
        Self {
            endpoint: endpoint.into(),
            client: reqwest::Client::builder()
                .timeout(timeout)
                .build()
                .expect("models.dev HTTP client should build"),
            cache: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn enrich(
        &self,
        provider_id: &str,
        discovered: Vec<LlmModelSummary>,
    ) -> Vec<LlmModelSummary> {
        if discovered.is_empty() {
            return discovered;
        }
        let model_ids = discovered
            .iter()
            .map(|model| model.model.as_str())
            .collect::<Vec<_>>();
        let metadata = match self
            .fetch()
            .await
            .and_then(|body| parse_models_dev_models(&body, provider_id, &model_ids))
        {
            Ok(metadata) => metadata,
            Err(error) => {
                log::debug!("models.dev metadata enrichment unavailable: {error}");
                return discovered;
            }
        };
        merge_model_metadata(discovered, metadata)
    }

    pub async fn describe(&self, provider_id: &str, model_id: &str) -> Option<LlmModelSummary> {
        let body = match self.fetch().await {
            Ok(body) => body,
            Err(error) => {
                log::debug!("models.dev metadata lookup unavailable: {error}");
                return None;
            }
        };
        parse_models_dev_models(&body, provider_id, &[model_id])
            .ok()?
            .into_iter()
            .next()
    }

    async fn fetch(&self) -> Result<String, LlmPortError> {
        if let Some(cached) = self
            .cache
            .read()
            .map_err(|_| {
                LlmPortError::new(
                    LlmPortErrorKind::Unavailable,
                    "models.dev cache lock is poisoned",
                )
            })?
            .as_ref()
            .filter(|cached| cached.fetched_at.elapsed() < MODELS_DEV_CACHE_TTL)
        {
            return Ok(cached.body.clone());
        }

        let response = self
            .client
            .get(&self.endpoint)
            .send()
            .await
            .map_err(reqwest_port_error)?;
        let status = response.status();
        let headers = response.headers().clone();
        let body = response.text().await.map_err(reqwest_port_error)?;
        if !status.is_success() {
            return Err(http_status_port_error(status, &headers, body));
        }
        *self.cache.write().map_err(|_| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                "models.dev cache lock is poisoned",
            )
        })? = Some(CachedCatalog {
            fetched_at: Instant::now(),
            body: body.clone(),
        });
        Ok(body)
    }
}

pub(crate) fn default_models_dev_catalog() -> &'static ModelsDevCatalog {
    static CATALOG: OnceLock<ModelsDevCatalog> = OnceLock::new();
    CATALOG.get_or_init(ModelsDevCatalog::default)
}

#[derive(Deserialize)]
struct ModelsDevProvider {
    models: HashMap<String, ModelsDevModel>,
}

#[derive(Deserialize)]
struct ModelsDevReasoningOption {
    #[serde(rename = "type")]
    option_type: String,
    min: Option<u64>,
    max: Option<u64>,
    default: Option<serde_json::Value>,
    options: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct ModelsDevModel {
    id: String,
    name: Option<String>,
    structured_output: Option<bool>,
    reasoning: Option<bool>,
    reasoning_options: Option<Vec<ModelsDevReasoningOption>>,
    tool_call: Option<bool>,
    knowledge: Option<String>,
    release_date: Option<String>,
    last_updated: Option<String>,
    modalities: Option<ModelsDevModalities>,
    limit: Option<ModelsDevLimit>,
    cost: Option<ModelsDevCost>,
}

#[derive(Deserialize)]
struct ModelsDevModalities {
    #[serde(default)]
    input: Vec<String>,
    #[serde(default)]
    output: Vec<String>,
}

#[derive(Deserialize)]
struct ModelsDevLimit {
    context: Option<u64>,
    output: Option<u64>,
}

#[derive(Deserialize)]
struct ModelsDevCost {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: Option<f64>,
}

pub fn parse_models_dev_models(
    body: &str,
    provider_id: &str,
    model_ids: &[&str],
) -> Result<Vec<LlmModelSummary>, LlmPortError> {
    let catalog =
        serde_json::from_str::<HashMap<String, ModelsDevProvider>>(body).map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Protocol,
                format!("expected a valid models.dev catalog response: {error}"),
            )
        })?;
    let provider = catalog.get(provider_id);
    let mut results = Vec::new();

    for model_id in model_ids {
        // 1. Exact match in the requested provider
        if let Some(m) = provider.and_then(|p| p.models.get(*model_id)) {
            let mut summary = model_summary(m, provider_id);
            summary.model = (*model_id).to_string();
            results.push(summary);
            continue;
        }

        // 2. Cross-provider fallback (e.g. for custom/reverse-proxy gateways)
        let core_id = model_id.split('/').next_back().unwrap_or(model_id);
        let mut found = None;

        for (p_id, p) in &catalog {
            if let Some(m) = p.models.get(*model_id).or_else(|| p.models.get(core_id)) {
                found = Some((m, p_id.as_str()));
                break;
            }
        }

        if found.is_none() {
            let lower_core = core_id.to_lowercase();
            for (p_id, p) in &catalog {
                for (id, m) in &p.models {
                    if id.to_lowercase() == lower_core {
                        found = Some((m, p_id.as_str()));
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
            }
        }

        if let Some((m, found_p_id)) = found {
            let mut summary = model_summary(m, found_p_id);
            summary.model = (*model_id).to_string();
            results.push(summary);
        }
    }

    Ok(results)
}

fn model_summary(model: &ModelsDevModel, provider_id: &str) -> LlmModelSummary {
    let modalities = model.modalities.as_ref();
    let input_modalities = modalities
        .map(|modalities| parse_modalities(&modalities.input))
        .unwrap_or_default();
    let output_modalities = modalities
        .map(|modalities| parse_modalities(&modalities.output))
        .unwrap_or_default();
    let supports_multimodal =
        (!input_modalities.is_empty() || !output_modalities.is_empty()).then(|| {
            input_modalities
                .iter()
                .chain(&output_modalities)
                .any(|modality| *modality != LlmModality::Text)
        });
    let cost = model.cost.as_ref();
    let cache_read_price = cost.and_then(|cost| cost.cache_read);
    let cache_write_price = cost.and_then(|cost| cost.cache_write);

    let strategy = strategy_from_models_dev_provider(provider_id);
    let inferred =
        sona_core::llm::capabilities::LlmModelCapabilities::infer(strategy, &model.id, "");

    let (reasoning_mode, supported_thinking_levels) = if model.reasoning == Some(true) {
        if let Some(options) = &model.reasoning_options {
            if let Some(effort_opt) = options.iter().find(|o| o.option_type == "effort") {
                let levels: Vec<sona_core::llm::runtime::ThinkingLevel> = effort_opt
                    .options
                    .as_ref()
                    .map(|opts| {
                        opts.iter()
                            .filter_map(|s| match s.as_str() {
                                "minimal" => Some(sona_core::llm::runtime::ThinkingLevel::Minimal),
                                "low" => Some(sona_core::llm::runtime::ThinkingLevel::Low),
                                "medium" => Some(sona_core::llm::runtime::ThinkingLevel::Medium),
                                "high" => Some(sona_core::llm::runtime::ThinkingLevel::High),
                                "xhigh" => Some(sona_core::llm::runtime::ThinkingLevel::Xhigh),
                                "max" => Some(sona_core::llm::runtime::ThinkingLevel::Max),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();
                let supported = if levels.is_empty() {
                    inferred.supported_thinking_levels.clone()
                } else {
                    levels
                };
                (
                    Some(sona_core::llm::runtime::ReasoningMode::Effort {
                        supported_levels: supported.clone(),
                    }),
                    supported,
                )
            } else if let Some(budget_opt) = options.iter().find(|o| o.option_type == "budget") {
                let min_budget = budget_opt.min.unwrap_or(1024) as u32;
                let max_budget = budget_opt
                    .max
                    .unwrap_or_else(|| model.limit.as_ref().and_then(|l| l.output).unwrap_or(64000))
                    as u32;
                let default_budget = budget_opt
                    .default
                    .as_ref()
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(4096) as u32;
                (
                    Some(sona_core::llm::runtime::ReasoningMode::Budget {
                        min_budget,
                        max_budget,
                        default_budget,
                    }),
                    inferred.supported_thinking_levels.clone(),
                )
            } else {
                (
                    Some(inferred.reasoning_mode.clone()),
                    inferred.supported_thinking_levels.clone(),
                )
            }
        } else {
            (
                Some(inferred.reasoning_mode.clone()),
                inferred.supported_thinking_levels.clone(),
            )
        }
    } else {
        (
            Some(sona_core::llm::runtime::ReasoningMode::None),
            Vec::new(),
        )
    };

    let token_limit_key = Some(inferred.token_limit_key.as_str().to_string());
    let supports_temperature = Some(inferred.supports_temperature);

    LlmModelSummary {
        model: model.id.clone(),
        display_name: model.name.clone(),
        input_price: cost.and_then(|cost| cost.input),
        output_price: cost.and_then(|cost| cost.output),
        cache_read_price,
        cache_write_price,
        context_window: model.limit.as_ref().and_then(|limit| limit.context),
        max_output_tokens: model.limit.as_ref().and_then(|limit| limit.output),
        knowledge_cutoff: model.knowledge.clone(),
        release_date: model.release_date.clone(),
        last_updated: model.last_updated.clone(),
        input_modalities,
        output_modalities,
        supports_multimodal,
        supports_tools: model.tool_call,
        supports_reasoning: model.reasoning,
        supports_temperature,
        supports_structured_output: model.structured_output,
        supports_prompt_caching: (cache_read_price.is_some() || cache_write_price.is_some())
            .then_some(true),
        metadata_sources: vec![LlmModelMetadataSource::ModelsDev],
        reasoning_mode,
        token_limit_key,
        supported_thinking_levels,
    }
}

pub fn strategy_from_models_dev_provider(provider_id: &str) -> LlmProviderStrategy {
    match provider_id {
        "openai" => LlmProviderStrategy::OpenAi,
        "azure" => LlmProviderStrategy::AzureOpenAi,
        "anthropic" => LlmProviderStrategy::Anthropic,
        "google" => LlmProviderStrategy::Gemini,
        "deepseek" => LlmProviderStrategy::DeepSeek,
        "moonshotai" | "moonshotai-cn" => LlmProviderStrategy::MoonshotAi,
        "xiaomi" => LlmProviderStrategy::Xiaomi,
        "siliconflow" => LlmProviderStrategy::SiliconFlow,
        "alibaba" => LlmProviderStrategy::Qwen,
        "minimax" | "minimax-cn" => LlmProviderStrategy::MinimaxGlobal,
        "openrouter" => LlmProviderStrategy::OpenRouter,
        "groq" => LlmProviderStrategy::Groq,
        "xai" => LlmProviderStrategy::XAi,
        "mistral" => LlmProviderStrategy::MistralAi,
        "perplexity" => LlmProviderStrategy::Perplexity,
        "zhipuai" => LlmProviderStrategy::Chatglm,
        "github-copilot" => LlmProviderStrategy::Copilot,
        "cohere" => LlmProviderStrategy::Cohere,
        "together" => LlmProviderStrategy::Together,
        "venice" => LlmProviderStrategy::Venice,
        "hyperbolic" => LlmProviderStrategy::Hyperbolic,
        _ => LlmProviderStrategy::OpenAiCompatible,
    }
}

fn parse_modalities(values: &[String]) -> Vec<LlmModality> {
    values
        .iter()
        .filter_map(|value| match value.as_str() {
            "text" => Some(LlmModality::Text),
            "image" => Some(LlmModality::Image),
            "audio" => Some(LlmModality::Audio),
            "video" => Some(LlmModality::Video),
            "pdf" => Some(LlmModality::Pdf),
            _ => None,
        })
        .collect()
}

pub fn models_dev_provider_id(strategy: LlmProviderStrategy) -> Option<&'static str> {
    Some(match strategy {
        LlmProviderStrategy::OpenAi | LlmProviderStrategy::OpenAiResponses => "openai",
        LlmProviderStrategy::AzureOpenAi => "azure",
        LlmProviderStrategy::Anthropic => "anthropic",
        LlmProviderStrategy::Gemini => "google",
        LlmProviderStrategy::Ollama => return None,
        LlmProviderStrategy::DeepSeek => "deepseek",
        LlmProviderStrategy::MoonshotAi => "moonshotai",
        LlmProviderStrategy::MoonshotCn => "moonshotai-cn",
        LlmProviderStrategy::Xiaomi => "xiaomi",
        LlmProviderStrategy::Kimi => "moonshotai",
        LlmProviderStrategy::SiliconFlow => "siliconflow",
        LlmProviderStrategy::Qwen | LlmProviderStrategy::QwenPortal => "alibaba",
        LlmProviderStrategy::MinimaxGlobal => "minimax",
        LlmProviderStrategy::MinimaxCn => "minimax-cn",
        LlmProviderStrategy::OpenRouter => "openrouter",
        LlmProviderStrategy::LmStudio => "lmstudio",
        LlmProviderStrategy::Groq => "groq",
        LlmProviderStrategy::XAi => "xai",
        LlmProviderStrategy::MistralAi => "mistral",
        LlmProviderStrategy::Perplexity => "perplexity",
        LlmProviderStrategy::Chatglm => "zhipuai",
        LlmProviderStrategy::Copilot => "github-copilot",
        LlmProviderStrategy::Cohere => "cohere",
        LlmProviderStrategy::Together => "together",
        LlmProviderStrategy::Venice => "venice",
        LlmProviderStrategy::Hyperbolic => "hyperbolic",
        LlmProviderStrategy::Llamafile
        | LlmProviderStrategy::Local
        | LlmProviderStrategy::Volcengine
        | LlmProviderStrategy::GoogleTranslate
        | LlmProviderStrategy::GoogleTranslateFree
        | LlmProviderStrategy::OpenAiCompatible
        | LlmProviderStrategy::OpenAiCompatibleCustomPath => return None,
    })
}

pub fn should_enrich_model_metadata(provider: &LlmProvider, base_url: &str) -> bool {
    let trimmed = base_url.trim();
    if trimmed.is_empty() {
        return match provider {
            LlmProvider::Builtin(b) => !matches!(
                b,
                sona_core::domain::BuiltinLlmProvider::Local
                    | sona_core::domain::BuiltinLlmProvider::Ollama
                    | sona_core::domain::BuiltinLlmProvider::Llamafile
                    | sona_core::domain::BuiltinLlmProvider::LmStudio
                    | sona_core::domain::BuiltinLlmProvider::GoogleTranslate
                    | sona_core::domain::BuiltinLlmProvider::GoogleTranslateFree
            ),
            LlmProvider::Custom(_) => false,
        };
    }
    let Ok(url) = reqwest::Url::parse(trimmed) else {
        return false;
    };
    !is_local_or_lan_host(&url)
}
