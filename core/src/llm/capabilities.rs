use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

use crate::llm::provider_protocol::LlmModelSummary;
use crate::llm::runtime::{ReasoningMode, ThinkingLevel};
use crate::llm::tasks::LlmProviderStrategy;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum TokenLimitKey {
    MaxTokens,
    MaxCompletionTokens,
}

impl TokenLimitKey {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::MaxTokens => "max_tokens",
            Self::MaxCompletionTokens => "max_completion_tokens",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct LlmModelCapabilities {
    pub reasoning: bool,
    pub reasoning_mode: ReasoningMode,
    pub supported_thinking_levels: Vec<ThinkingLevel>,
    pub supports_temperature: bool,
    pub token_limit_key: TokenLimitKey,
}

impl Default for LlmModelCapabilities {
    fn default() -> Self {
        Self {
            reasoning: false,
            reasoning_mode: ReasoningMode::None,
            supported_thinking_levels: Vec::new(),
            supports_temperature: true,
            token_limit_key: TokenLimitKey::MaxTokens,
        }
    }
}

impl LlmModelCapabilities {
    pub fn resolve(
        strategy: LlmProviderStrategy,
        model: &str,
        base_url: &str,
        summary: Option<&LlmModelSummary>,
    ) -> Self {
        let base = Self::infer(strategy, model, base_url);
        if let Some(s) = summary {
            Self {
                reasoning: s.supports_reasoning.unwrap_or(base.reasoning),
                reasoning_mode: s.reasoning_mode.clone().unwrap_or(base.reasoning_mode),
                supported_thinking_levels: if s.supported_thinking_levels.is_empty() {
                    base.supported_thinking_levels
                } else {
                    s.supported_thinking_levels.clone()
                },
                supports_temperature: s.supports_temperature.unwrap_or(base.supports_temperature),
                token_limit_key: match s.token_limit_key.as_deref() {
                    Some("max_completion_tokens") => TokenLimitKey::MaxCompletionTokens,
                    Some("max_tokens") => TokenLimitKey::MaxTokens,
                    _ => base.token_limit_key,
                },
            }
        } else {
            base
        }
    }

    pub fn infer(strategy: LlmProviderStrategy, model: &str, base_url: &str) -> Self {
        let lower = model.to_lowercase();
        let core_model = lower.trim().rsplit('/').next().unwrap_or(&lower);
        let normalized = core_model.replace('.', "-");
        let base_lower = base_url.to_lowercase();

        // 1. Translation-only providers
        if matches!(
            strategy,
            LlmProviderStrategy::GoogleTranslate | LlmProviderStrategy::GoogleTranslateFree
        ) {
            return Self {
                reasoning: false,
                reasoning_mode: ReasoningMode::None,
                supported_thinking_levels: Vec::new(),
                supports_temperature: false,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 2. Claude (Anthropic) Models — detected across ALL strategies
        let is_claude = strategy == LlmProviderStrategy::Anthropic
            || normalized.contains("claude")
            || base_lower.contains("anthropic");

        if is_claude {
            let is_claude_4_5_budget = normalized.contains("claude-sonnet-4-5")
                || normalized.contains("claude-opus-4-5")
                || normalized.contains("claude-haiku-4-5")
                || normalized.contains("claude-4-5");

            let is_claude_3_7_budget = normalized.contains("claude-3-7");

            let is_claude_adaptive = !is_claude_4_5_budget
                && !is_claude_3_7_budget
                && (normalized.contains("claude-opus-5")
                    || normalized.contains("claude-5")
                    || normalized.contains("claude-sonnet-5")
                    || normalized.contains("claude-fable-5")
                    || normalized.contains("claude-sonnet-4-6")
                    || normalized.contains("claude-opus-4-6")
                    || normalized.contains("claude-opus-4-7")
                    || normalized.contains("claude-opus-4-8")
                    || normalized.contains("claude-sonnet-4")
                    || normalized.contains("claude-opus-4")
                    || normalized.contains("claude-4"));

            if is_claude_adaptive {
                return Self {
                    reasoning: true,
                    reasoning_mode: ReasoningMode::Effort {
                        supported_levels: vec![
                            ThinkingLevel::Low,
                            ThinkingLevel::Medium,
                            ThinkingLevel::High,
                            ThinkingLevel::Xhigh,
                            ThinkingLevel::Max,
                        ],
                    },
                    supported_thinking_levels: vec![
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                    supports_temperature: false,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            }

            if is_claude_3_7_budget || is_claude_4_5_budget {
                return Self {
                    reasoning: true,
                    reasoning_mode: ReasoningMode::Budget {
                        min_budget: 1024,
                        max_budget: 64000,
                        default_budget: 4096,
                    },
                    supported_thinking_levels: vec![
                        ThinkingLevel::Minimal,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                    supports_temperature: false,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            }

            let is_older_claude = normalized.contains("claude-3-5")
                || normalized.contains("claude-3-opus")
                || normalized.contains("claude-3-sonnet")
                || normalized.contains("claude-3-haiku")
                || normalized.contains("claude-2")
                || normalized.contains("claude-1")
                || normalized.contains("claude-instant");

            if is_older_claude {
                return Self {
                    reasoning: false,
                    reasoning_mode: ReasoningMode::None,
                    supported_thinking_levels: Vec::new(),
                    supports_temperature: true,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            }

            if normalized.contains("thinking") || normalized.contains("reasoning") {
                return Self {
                    reasoning: true,
                    reasoning_mode: ReasoningMode::Effort {
                        supported_levels: vec![
                            ThinkingLevel::Low,
                            ThinkingLevel::Medium,
                            ThinkingLevel::High,
                            ThinkingLevel::Xhigh,
                            ThinkingLevel::Max,
                        ],
                    },
                    supported_thinking_levels: vec![
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                    supports_temperature: false,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            }

            return Self {
                reasoning: false,
                reasoning_mode: ReasoningMode::None,
                supported_thinking_levels: Vec::new(),
                supports_temperature: true,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 3. Google Gemini & Gemma Models
        let is_gemini = strategy == LlmProviderStrategy::Gemini
            || normalized.contains("gemini")
            || normalized.contains("gemma");

        if is_gemini {
            let uses_level = normalized.contains("gemini-3")
                || normalized == "gemini-flash-latest"
                || normalized == "gemini-flash-lite-latest"
                || normalized.contains("gemma-4");
            let is_budget = normalized.contains("gemini-2-5")
                || normalized.contains("gemini-2-0-flash-thinking")
                || normalized.contains("gemini-2-0-thinking");

            if uses_level {
                return Self {
                    reasoning: true,
                    reasoning_mode: ReasoningMode::Effort {
                        supported_levels: vec![
                            ThinkingLevel::Minimal,
                            ThinkingLevel::Low,
                            ThinkingLevel::Medium,
                            ThinkingLevel::High,
                        ],
                    },
                    supported_thinking_levels: vec![
                        ThinkingLevel::Minimal,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                    ],
                    supports_temperature: true,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            } else if is_budget {
                return Self {
                    reasoning: true,
                    reasoning_mode: ReasoningMode::Hybrid {
                        supported_levels: vec![
                            ThinkingLevel::Minimal,
                            ThinkingLevel::Low,
                            ThinkingLevel::Medium,
                            ThinkingLevel::High,
                        ],
                        default_budget: 2048,
                    },
                    supported_thinking_levels: vec![
                        ThinkingLevel::Minimal,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                    ],
                    supports_temperature: true,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            } else {
                return Self {
                    reasoning: false,
                    reasoning_mode: ReasoningMode::None,
                    supported_thinking_levels: Vec::new(),
                    supports_temperature: true,
                    token_limit_key: TokenLimitKey::MaxTokens,
                };
            }
        }

        // 4. OpenAI Reasoning Models (o1, o3, o4, gpt-5, gpt-6)
        let is_openai_family = matches!(
            strategy,
            LlmProviderStrategy::OpenAi
                | LlmProviderStrategy::AzureOpenAi
                | LlmProviderStrategy::OpenAiResponses
                | LlmProviderStrategy::Copilot
        ) || normalized.starts_with("openai/")
            || base_lower.contains("openai");

        let is_o_series = normalized.starts_with("o1")
            || normalized.starts_with("o3")
            || normalized.starts_with("o4");

        let is_gpt_reasoning = normalized.starts_with("gpt-5") || normalized.starts_with("gpt-6");

        if is_o_series {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::Effort {
                    supported_levels: vec![
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                    ],
                },
                supported_thinking_levels: vec![
                    ThinkingLevel::Low,
                    ThinkingLevel::Medium,
                    ThinkingLevel::High,
                ],
                supports_temperature: false,
                token_limit_key: TokenLimitKey::MaxCompletionTokens,
            };
        }

        if is_gpt_reasoning {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::Effort {
                    supported_levels: vec![
                        ThinkingLevel::Minimal,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                },
                supported_thinking_levels: vec![
                    ThinkingLevel::Minimal,
                    ThinkingLevel::Low,
                    ThinkingLevel::Medium,
                    ThinkingLevel::High,
                    ThinkingLevel::Xhigh,
                    ThinkingLevel::Max,
                ],
                supports_temperature: false,
                token_limit_key: TokenLimitKey::MaxCompletionTokens,
            };
        }

        // 5. DeepSeek reasoning models
        let is_deepseek =
            strategy == LlmProviderStrategy::DeepSeek || normalized.contains("deepseek");

        if is_deepseek {
            let is_r1 = normalized.contains("r1") || normalized.contains("reasoner");
            return Self {
                reasoning: is_r1,
                reasoning_mode: ReasoningMode::None,
                supported_thinking_levels: Vec::new(),
                supports_temperature: !is_r1,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 6. QwQ / Qwen reasoning models
        if normalized.contains("qwq") || normalized.contains("qvq") {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::None,
                supported_thinking_levels: Vec::new(),
                supports_temperature: false,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        if normalized.contains("qwen3")
            || normalized.contains("qwen-3")
            || normalized.contains("qwen3-5")
        {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::Effort {
                    supported_levels: vec![
                        ThinkingLevel::Minimal,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                },
                supported_thinking_levels: vec![
                    ThinkingLevel::Minimal,
                    ThinkingLevel::Low,
                    ThinkingLevel::Medium,
                    ThinkingLevel::High,
                    ThinkingLevel::Xhigh,
                    ThinkingLevel::Max,
                ],
                supports_temperature: true,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        if strategy == LlmProviderStrategy::Local
            && let Some(preset) = crate::llm::local_models::find_local_llm_model(model)
            && preset.capabilities.iter().any(|c| c == "reasoning")
        {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::Effort {
                    supported_levels: vec![
                        ThinkingLevel::Minimal,
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                },
                supported_thinking_levels: vec![
                    ThinkingLevel::Minimal,
                    ThinkingLevel::Low,
                    ThinkingLevel::Medium,
                    ThinkingLevel::High,
                    ThinkingLevel::Xhigh,
                    ThinkingLevel::Max,
                ],
                supports_temperature: true,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 7. GLM / Zhipu reasoning models
        if strategy == LlmProviderStrategy::Chatglm || normalized.contains("glm") {
            let is_reasoning = normalized.contains("glm-zero") || normalized.contains("r1");
            return Self {
                reasoning: is_reasoning,
                reasoning_mode: ReasoningMode::None,
                supported_thinking_levels: Vec::new(),
                supports_temperature: !is_reasoning,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 8. Together / generic fixed reasoning
        if normalized.contains("reasoner") || normalized.contains("r1") {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::None,
                supported_thinking_levels: Vec::new(),
                supports_temperature: false,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 9. Generic models with "thinking" in name
        if normalized.contains("thinking") {
            return Self {
                reasoning: true,
                reasoning_mode: ReasoningMode::Effort {
                    supported_levels: vec![
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                    ],
                },
                supported_thinking_levels: vec![
                    ThinkingLevel::Low,
                    ThinkingLevel::Medium,
                    ThinkingLevel::High,
                ],
                supports_temperature: false,
                token_limit_key: TokenLimitKey::MaxTokens,
            };
        }

        // 10. Default standard chat models (gpt-4o, qwen-turbo, mistral, llama, etc.)
        let use_max_completion_tokens = is_openai_family && normalized.starts_with("gpt-4-1");
        Self {
            reasoning: false,
            reasoning_mode: ReasoningMode::None,
            supported_thinking_levels: Vec::new(),
            supports_temperature: true,
            token_limit_key: if use_max_completion_tokens {
                TokenLimitKey::MaxCompletionTokens
            } else {
                TokenLimitKey::MaxTokens
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn infers_claude_opus_5_5_capabilities_under_any_strategy() {
        for strategy in [
            LlmProviderStrategy::Anthropic,
            LlmProviderStrategy::OpenRouter,
            LlmProviderStrategy::OpenAiCompatible,
        ] {
            for model in [
                "claude-opus-5-5",
                "claude-opus-5.5",
                "anthropic/claude-opus-5-5",
                "claude-5-5-opus",
            ] {
                let caps = LlmModelCapabilities::infer(strategy, model, "");
                assert!(
                    caps.reasoning,
                    "expected reasoning for {model} under {strategy:?}"
                );
                assert_eq!(
                    caps.supported_thinking_levels,
                    vec![
                        ThinkingLevel::Low,
                        ThinkingLevel::Medium,
                        ThinkingLevel::High,
                        ThinkingLevel::Xhigh,
                        ThinkingLevel::Max,
                    ],
                    "expected 5 thinking levels for {model} under {strategy:?}"
                );
                assert_eq!(
                    caps.reasoning_mode,
                    ReasoningMode::Effort {
                        supported_levels: vec![
                            ThinkingLevel::Low,
                            ThinkingLevel::Medium,
                            ThinkingLevel::High,
                            ThinkingLevel::Xhigh,
                            ThinkingLevel::Max,
                        ],
                    }
                );
                assert!(
                    !caps.supports_temperature,
                    "claude opus 5.5 must prohibit temperature"
                );
            }
        }
    }

    #[test]
    fn infers_claude_3_7_budget_mode() {
        let caps =
            LlmModelCapabilities::infer(LlmProviderStrategy::Anthropic, "claude-3-7-sonnet", "");
        assert!(caps.reasoning);
        assert_eq!(
            caps.supported_thinking_levels.len(),
            6,
            "expected 6 thinking levels"
        );
        assert!(matches!(caps.reasoning_mode, ReasoningMode::Budget { .. }));
        assert!(!caps.supports_temperature);
    }

    #[test]
    fn infers_older_claude_as_non_reasoning_with_temperature() {
        let caps = LlmModelCapabilities::infer(
            LlmProviderStrategy::Anthropic,
            "claude-3-5-sonnet-20241022",
            "",
        );
        assert!(!caps.reasoning);
        assert_eq!(caps.reasoning_mode, ReasoningMode::None);
        assert!(caps.supports_temperature);
    }

    #[test]
    fn infers_openai_o_series_and_standard_models() {
        let o3 = LlmModelCapabilities::infer(LlmProviderStrategy::OpenAi, "o3-mini", "");
        assert!(o3.reasoning);
        assert_eq!(
            o3.supported_thinking_levels,
            vec![
                ThinkingLevel::Low,
                ThinkingLevel::Medium,
                ThinkingLevel::High
            ]
        );
        assert!(!o3.supports_temperature);
        assert_eq!(o3.token_limit_key, TokenLimitKey::MaxCompletionTokens);

        let gpt4o = LlmModelCapabilities::infer(LlmProviderStrategy::OpenAi, "gpt-4o", "");
        assert!(!gpt4o.reasoning);
        assert_eq!(gpt4o.reasoning_mode, ReasoningMode::None);
        assert!(gpt4o.supports_temperature);
    }

    #[test]
    fn infers_gemini_models() {
        let gemini3 =
            LlmModelCapabilities::infer(LlmProviderStrategy::Gemini, "gemini-3-flash", "");
        assert!(gemini3.reasoning);
        assert_eq!(
            gemini3.supported_thinking_levels,
            vec![
                ThinkingLevel::Minimal,
                ThinkingLevel::Low,
                ThinkingLevel::Medium,
                ThinkingLevel::High
            ]
        );
        assert!(gemini3.supports_temperature);

        let gemini15 =
            LlmModelCapabilities::infer(LlmProviderStrategy::Gemini, "gemini-1.5-flash", "");
        assert!(!gemini15.reasoning);
        assert!(gemini15.supports_temperature);
    }

    #[test]
    fn infers_deepseek_models() {
        let r1 =
            LlmModelCapabilities::infer(LlmProviderStrategy::DeepSeek, "deepseek-reasoner", "");
        assert!(r1.reasoning);
        assert_eq!(r1.reasoning_mode, ReasoningMode::None);
        assert!(!r1.supports_temperature);

        let v3 = LlmModelCapabilities::infer(LlmProviderStrategy::DeepSeek, "deepseek-chat", "");
        assert!(!v3.reasoning);
        assert!(v3.supports_temperature);
    }

    #[test]
    fn infers_qwen3_5_and_local_reasoning_models() {
        let qwen = LlmModelCapabilities::infer(LlmProviderStrategy::Local, "Qwen/Qwen3.5-4B", "");
        assert!(qwen.reasoning);
        assert!(matches!(qwen.reasoning_mode, ReasoningMode::Effort { .. }));
        assert_eq!(qwen.supported_thinking_levels.len(), 6);
        assert!(qwen.supports_temperature);

        let qwen3 =
            LlmModelCapabilities::infer(LlmProviderStrategy::Local, "Qwen/Qwen3-1.7B-Instruct", "");
        assert!(qwen3.reasoning);
        assert!(matches!(qwen3.reasoning_mode, ReasoningMode::Effort { .. }));
        assert_eq!(qwen3.supported_thinking_levels.len(), 6);
        assert!(qwen3.supports_temperature);

        let gemma =
            LlmModelCapabilities::infer(LlmProviderStrategy::Local, "google/gemma-4-e2b", "");
        assert!(gemma.reasoning);
        assert!(matches!(gemma.reasoning_mode, ReasoningMode::Effort { .. }));
        assert!(gemma.supports_temperature);
    }
}
