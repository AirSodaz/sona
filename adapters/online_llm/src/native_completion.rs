use sona_core::llm::provider_protocol::StandardLlmResponse;
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::gemini::build_gemini_generate_content_request_parts_for_reqwest;
use crate::transport::LlmApiUrl;
pub type StrategyTarget = (LlmApiUrl, Vec<(&'static str, String)>);

pub fn resolve_strategy_url_and_headers(
    request: &LlmCompletionRequest,
    stream: bool,
) -> Result<StrategyTarget, LlmPortError> {
    let config = &request.config;
    let strategy = config.strategy;
    let base_url = config.base_url.trim();

    match strategy {
        LlmProviderStrategy::AzureOpenAi => {
            let api_version = config.api_version.as_deref().unwrap_or("2024-10-21");
            let endpoint = base_url.trim_end_matches('/');
            let path = format!("/openai/deployments/{}/chat/completions", config.model);
            let url = LlmApiUrl::parse(endpoint)?
                .join(&path)?
                .with_query(&format!("api-version={api_version}"))?;
            let headers = vec![("api-key", config.api_key.clone())];
            Ok((url, headers))
        }
        LlmProviderStrategy::Anthropic => {
            let host = if base_url.is_empty() {
                "https://api.anthropic.com"
            } else {
                base_url
            };
            let url = LlmApiUrl::parse(host)?
                .join(config.api_path.as_deref().unwrap_or("/v1/messages"))?;
            let headers = vec![
                ("x-api-key", config.api_key.clone()),
                ("anthropic-version", "2023-06-01".to_string()),
            ];
            Ok((url, headers))
        }
        LlmProviderStrategy::Gemini => {
            let host = if base_url.is_empty() {
                "https://generativelanguage.googleapis.com"
            } else {
                base_url
            };
            let parts = build_gemini_generate_content_request_parts_for_reqwest(
                host,
                &config.model,
                &config.api_key,
                stream,
            )?;
            Ok((parts.url, parts.headers))
        }
        LlmProviderStrategy::OpenAiResponses => {
            let host = if base_url.is_empty() {
                "https://api.openai.com"
            } else {
                base_url
            };
            let url = LlmApiUrl::parse(host)?
                .join(config.api_path.as_deref().unwrap_or("/v1/responses"))?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
        LlmProviderStrategy::Cohere => {
            let host = if base_url.is_empty() {
                "https://api.cohere.com"
            } else {
                base_url
            };
            let url =
                LlmApiUrl::parse(host)?.join(config.api_path.as_deref().unwrap_or("/v2/chat"))?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
        LlmProviderStrategy::DeepSeek => {
            let host = if base_url.is_empty() {
                "https://api.deepseek.com"
            } else {
                base_url
            };
            let path = config.api_path.as_deref().unwrap_or("/chat/completions");
            let url = LlmApiUrl::parse(host)?.join(path)?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
        LlmProviderStrategy::Groq => {
            let host = if base_url.is_empty() {
                "https://api.groq.com/openai/v1"
            } else {
                base_url
            };
            let path = config.api_path.as_deref().unwrap_or("/chat/completions");
            let url = LlmApiUrl::parse(host)?.join(path)?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
        LlmProviderStrategy::Perplexity => {
            let host = if base_url.is_empty() {
                "https://api.perplexity.ai"
            } else {
                base_url
            };
            let path = config.api_path.as_deref().unwrap_or("/chat/completions");
            let url = LlmApiUrl::parse(host)?.join(path)?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
        LlmProviderStrategy::Ollama => {
            let host = if base_url.is_empty() {
                "http://127.0.0.1:11434"
            } else {
                base_url
            };
            let path = config.api_path.as_deref().unwrap_or("/v1/chat/completions");
            let url = LlmApiUrl::parse(host)?.join(path)?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
        _ => {
            let default_host = match strategy {
                LlmProviderStrategy::OpenAi => "https://api.openai.com",
                LlmProviderStrategy::Chatglm => "https://open.bigmodel.cn/api/paas/v4",
                LlmProviderStrategy::Volcengine => "https://ark.cn-beijing.volces.com/api/v3",
                LlmProviderStrategy::Groq => "https://api.groq.com/openai/v1",
                LlmProviderStrategy::Together => "https://api.together.xyz",
                LlmProviderStrategy::Hyperbolic => "https://api.hyperbolic.xyz",
                LlmProviderStrategy::Llamafile => "http://127.0.0.1:8080",
                LlmProviderStrategy::MistralAi => "https://api.mistral.ai",
                LlmProviderStrategy::MoonshotAi => "https://api.moonshot.ai/v1",
                LlmProviderStrategy::MoonshotCn | LlmProviderStrategy::Kimi => {
                    "https://api.moonshot.cn/v1"
                }
                LlmProviderStrategy::OpenRouter => "https://openrouter.ai/api/v1",
                LlmProviderStrategy::XAi => "https://api.x.ai",
                LlmProviderStrategy::Venice => "https://api.venice.ai/api/v1",
                LlmProviderStrategy::SiliconFlow => "https://api.siliconflow.cn/v1",
                LlmProviderStrategy::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1",
                LlmProviderStrategy::QwenPortal => "https://portal.qwen.ai/v1",
                LlmProviderStrategy::MinimaxGlobal => "https://api.minimaxi.chat/v1",
                LlmProviderStrategy::MinimaxCn => "https://api.minimax.chat/v1",
                LlmProviderStrategy::Copilot => "https://api.githubcopilot.com",
                _ => "https://api.openai.com",
            };
            let host = if base_url.is_empty() {
                default_host
            } else {
                base_url
            };
            let path = config.api_path.as_deref().unwrap_or("/v1/chat/completions");
            let url = LlmApiUrl::parse(host)?.join(path)?;
            let mut headers = vec![];
            if !config.api_key.is_empty() {
                headers.push(("Authorization", format!("Bearer {}", config.api_key)));
            }
            Ok((url, headers))
        }
    }
}

pub async fn execute_native_completion(
    request: &LlmCompletionRequest,
) -> Result<StandardLlmResponse, LlmPortError> {
    match request.config.strategy {
        LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree => {
            let url = LlmApiUrl::parse(&request.config.base_url)?;
            let client = url.client(request.config.timeout_seconds)?;
            crate::providers::GoogleTranslateAdapter
                .generate(&client, request)
                .await
        }
        _ => crate::aimux_adapter::execute_aimux_completion(request).await,
    }
}
