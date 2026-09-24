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
        let base_lower = base_url.to_lowercase();

        match strategy {
            LlmProviderStrategy::Anthropic => {
                let is_adaptive = core_model.contains("claude-opus-5")
                    || core_model.contains("claude-5")
                    || core_model.contains("claude-sonnet-4")
                    || core_model.contains("claude-4");
                let is_budget = core_model.contains("claude-3-7");

                if is_adaptive {
                    Self {
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
                    }
                } else if is_budget {
                    Self {
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
                    }
                } else {
                    Self {
                        reasoning: false,
                        reasoning_mode: ReasoningMode::None,
                        supported_thinking_levels: Vec::new(),
                        supports_temperature: true,
                        token_limit_key: TokenLimitKey::MaxTokens,
                    }
                }
            }
            LlmProviderStrategy::Gemini => {
                let uses_level = core_model.contains("gemini-3")
                    || core_model == "gemini-flash-latest"
                    || core_model == "gemini-flash-lite-latest"
                    || core_model.contains("gemma-4");
                let is_budget =
                    core_model.contains("gemini-2.5") || core_model.contains("gemini-2.0");

                if uses_level {
                    Self {
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
                    }
                } else if is_budget {
                    Self {
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
                    }
                } else {
                    Self {
                        reasoning: false,
                        reasoning_mode: ReasoningMode::None,
                        supported_thinking_levels: Vec::new(),
                        supports_temperature: true,
                        token_limit_key: TokenLimitKey::MaxTokens,
                    }
                }
            }
            LlmProviderStrategy::OpenAi
            | LlmProviderStrategy::AzureOpenAi
            | LlmProviderStrategy::OpenAiResponses
            | LlmProviderStrategy::Copilot => {
                let is_temp_prohibited = core_model.starts_with("o1")
                    || core_model.starts_with("o3")
                    || core_model.starts_with("o4")
                    || core_model.starts_with("gpt-5")
                    || core_model.starts_with("gpt-6")
                    || core_model.contains("reasoner")
                    || core_model.contains("r1")
                    || core_model.contains("qwq");
                let use_max_completion_tokens = core_model.starts_with("o1")
                    || core_model.starts_with("o3")
                    || core_model.starts_with("o4")
                    || core_model.starts_with("gpt-5")
                    || core_model.starts_with("gpt-6");

                Self {
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
                    supports_temperature: !is_temp_prohibited,
                    token_limit_key: if use_max_completion_tokens {
                        TokenLimitKey::MaxCompletionTokens
                    } else {
                        TokenLimitKey::MaxTokens
                    },
                }
            }
            LlmProviderStrategy::DeepSeek => {
                let is_r1 = core_model.contains("r1") || core_model.contains("reasoner");
                Self {
                    reasoning: is_r1,
                    reasoning_mode: ReasoningMode::None,
                    supported_thinking_levels: Vec::new(),
                    supports_temperature: !is_r1,
                    token_limit_key: TokenLimitKey::MaxTokens,
                }
            }
            LlmProviderStrategy::Chatglm => {
                let is_reasoning = core_model.contains("glm-zero") || core_model.contains("r1");
                Self {
                    reasoning: is_reasoning,
                    reasoning_mode: ReasoningMode::None,
                    supported_thinking_levels: Vec::new(),
                    supports_temperature: !is_reasoning,
                    token_limit_key: TokenLimitKey::MaxTokens,
                }
            }
            LlmProviderStrategy::Together => {
                let is_reasoning = core_model.contains("r1") || core_model.contains("qwq");
                Self {
                    reasoning: is_reasoning,
                    reasoning_mode: ReasoningMode::None,
                    supported_thinking_levels: Vec::new(),
                    supports_temperature: !is_reasoning,
                    token_limit_key: TokenLimitKey::MaxTokens,
                }
            }
            LlmProviderStrategy::OpenRouter => {
                let is_openai_reasoning = (core_model.contains("o1")
                    || core_model.contains("o3")
                    || core_model.contains("gpt-5")
                    || core_model.contains("gpt-6"))
                    && (lower.starts_with("openai/") || base_lower.contains("openai"));
                let is_fixed_reasoning = core_model.contains("r1") || core_model.contains("qwq");

                if is_openai_reasoning {
                    Self {
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
                    }
                } else if is_fixed_reasoning {
                    Self {
                        reasoning: true,
                        reasoning_mode: ReasoningMode::None,
                        supported_thinking_levels: Vec::new(),
                        supports_temperature: false,
                        token_limit_key: TokenLimitKey::MaxTokens,
                    }
                } else {
                    Self {
                        reasoning: false,
                        reasoning_mode: ReasoningMode::None,
                        supported_thinking_levels: Vec::new(),
                        supports_temperature: true,
                        token_limit_key: TokenLimitKey::MaxTokens,
                    }
                }
            }
            _ => {
                let is_non_standard = base_lower.contains("deepseek")
                    || base_lower.contains("moonshot")
                    || base_lower.contains("bigmodel.cn")
                    || base_lower.contains("together.xyz")
                    || base_lower.contains("x.ai")
                    || core_model.contains("deepseek-chat")
                    || core_model.contains("deepseek-v3");

                let is_fixed_reasoning = core_model.contains("r1")
                    || core_model.contains("reasoner")
                    || core_model.contains("qwq");

                let is_temp_prohibited = is_fixed_reasoning
                    || core_model.starts_with("o1")
                    || core_model.starts_with("o3")
                    || core_model.starts_with("o4")
                    || core_model.starts_with("gpt-5")
                    || core_model.starts_with("gpt-6");

                Self {
                    reasoning: is_fixed_reasoning || !is_non_standard,
                    reasoning_mode: if is_fixed_reasoning {
                        ReasoningMode::None
                    } else if !is_non_standard {
                        ReasoningMode::Effort {
                            supported_levels: vec![
                                ThinkingLevel::Low,
                                ThinkingLevel::Medium,
                                ThinkingLevel::High,
                                ThinkingLevel::Xhigh,
                                ThinkingLevel::Max,
                            ],
                        }
                    } else {
                        ReasoningMode::None
                    },
                    supported_thinking_levels: if is_fixed_reasoning || is_non_standard {
                        Vec::new()
                    } else {
                        vec![
                            ThinkingLevel::Low,
                            ThinkingLevel::Medium,
                            ThinkingLevel::High,
                            ThinkingLevel::Xhigh,
                            ThinkingLevel::Max,
                        ]
                    },
                    supports_temperature: !is_temp_prohibited,
                    token_limit_key: if core_model.starts_with("o1")
                        || core_model.starts_with("o3")
                        || core_model.starts_with("o4")
                        || core_model.starts_with("gpt-5")
                        || core_model.starts_with("gpt-6")
                    {
                        TokenLimitKey::MaxCompletionTokens
                    } else {
                        TokenLimitKey::MaxTokens
                    },
                }
            }
        }
    }
}
