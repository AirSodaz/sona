use sona_core::llm::provider_protocol::{
    StandardLlmResponse, extract_text_from_json_response, extract_usage_from_json_response,
};
use sona_core::llm::runtime::LlmCompletionRequest;
use sona_core::llm::tasks::LlmProviderStrategy;
use sona_core::ports::llm::LlmPortError;

use crate::gemini::{
    build_gemini_generate_content_request_parts_for_reqwest, extract_gemini_usage,
    extract_gemini_visible_text,
};
use crate::transport::{LlmApiUrl, post_json_request};

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
                LlmProviderStrategy::DeepSeek => "https://api.deepseek.com",
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
                LlmProviderStrategy::Cohere => "https://api.cohere.ai",
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
        LlmProviderStrategy::Anthropic => {
            let (url, headers) = resolve_strategy_url_and_headers(request, false)?;
            let payload = crate::anthropic::build_anthropic_payload_for_request(request, false)?;
            let response =
                post_json_request(&url, headers, payload, request.config.timeout_seconds).await?;
            let (text, thought, usage) =
                sona_core::llm::provider_protocol::extract_anthropic_text_and_thought_response(
                    &response,
                )?;
            Ok(StandardLlmResponse {
                text,
                thought,
                usage,
            })
        }
        LlmProviderStrategy::Gemini => {
            let (url, headers) = resolve_strategy_url_and_headers(request, false)?;
            let payload = crate::gemini::build_gemini_payload_for_request(request)?;
            let response =
                post_json_request(&url, headers, payload, request.config.timeout_seconds).await?;
            let text = extract_gemini_visible_text(&response)
                .or_else(|| extract_text_from_json_response(&response).ok())
                .unwrap_or_default();
            let thought = crate::gemini::extract_gemini_thought(&response);
            let usage = response.get("usageMetadata").and_then(extract_gemini_usage);
            Ok(StandardLlmResponse {
                text,
                thought,
                usage,
            })
        }
        LlmProviderStrategy::OpenAiResponses => {
            crate::responses::generate_with_openai_responses_api(request).await
        }
        _ => {
            let (url, headers) = resolve_strategy_url_and_headers(request, false)?;
            let is_azure = request.config.strategy == LlmProviderStrategy::AzureOpenAi;
            let payload = crate::openai_compatible::build_openai_chat_payload_for_request(
                request, false, is_azure,
            )?;
            let response =
                post_json_request(&url, headers, payload, request.config.timeout_seconds).await?;
            let (text, thought) =
                sona_core::llm::provider_protocol::extract_text_and_thought_from_json_response(
                    &response,
                )?;
            Ok(StandardLlmResponse {
                text,
                thought,
                usage: extract_usage_from_json_response(&response),
            })
        }
    }
}
