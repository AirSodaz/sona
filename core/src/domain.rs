use serde::{Deserialize, Deserializer, Serialize, de};
#[cfg(feature = "specta")]
use specta::Type;

/// A macro to implement backward-compatible deserialization for an enum
/// that is internally represented as `Builtin(BuiltinEnum)` and `Custom(String)`.
/// This macro handles parsing both flat strings (e.g., `"general"`) and the nested struct `{"Builtin": "general"}`.
macro_rules! impl_fallback_deserialize {
    ($enum_name:ident, $builtin_enum:ident) => {
        impl<'de> Deserialize<'de> for $enum_name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                // We use an untagged temporary enum to try parsing either format.
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum Temp {
                    // Try parsing the flat string first
                    String(String),
                    // If not a flat string, try parsing the nested object representation
                    Tagged {
                        #[serde(rename = "Builtin")]
                        builtin: Option<$builtin_enum>,
                        #[serde(rename = "Custom")]
                        custom: Option<String>,
                    },
                }

                let temp = Temp::deserialize(deserializer)?;
                match temp {
                    Temp::String(s) => {
                        // Attempt to parse the string as the Builtin enum using Serde's string parsing.
                        // We use a small trick: deserialize the string into the Builtin enum.
                        let json_str = format!("\"{}\"", s);
                        match serde_json::from_str::<$builtin_enum>(&json_str) {
                            Ok(builtin) => Ok($enum_name::Builtin(builtin)),
                            Err(_) => Ok($enum_name::Custom(s)),
                        }
                    }
                    Temp::Tagged { builtin, custom } => {
                        if let Some(b) = builtin {
                            Ok($enum_name::Builtin(b))
                        } else if let Some(c) = custom {
                            Ok($enum_name::Custom(c))
                        } else {
                            Err(de::Error::custom("Invalid object structure"))
                        }
                    }
                }
            }
        }
    };
}

// -----------------------------------------------------------------------------
// Polish Preset
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum BuiltinPolishPresetId {
    General,
    CustomerService,
    Meeting,
    Interview,
    Lecture,
    Podcast,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum PolishPresetId {
    Builtin(BuiltinPolishPresetId),
    Custom(String),
}

impl_fallback_deserialize!(PolishPresetId, BuiltinPolishPresetId);

// -----------------------------------------------------------------------------
// Summary Template
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum BuiltinSummaryTemplateId {
    General,
    Meeting,
    Lecture,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum SummaryTemplateId {
    Builtin(BuiltinSummaryTemplateId),
    Custom(String),
}

impl_fallback_deserialize!(SummaryTemplateId, BuiltinSummaryTemplateId);

// -----------------------------------------------------------------------------
// LLM Provider
// -----------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "snake_case")]
pub enum BuiltinLlmProvider {
    GoogleTranslate,
    GoogleTranslateFree,
    OpenAi,
    OpenAiResponses,
    AzureOpenai,
    Anthropic,
    Gemini,
    Ollama,
    DeepSeek,
    MoonshotAi,
    MoonshotCn,
    Xiaomi,
    Kimi,
    SiliconFlow,
    Qwen,
    QwenPortal,
    MinimaxGlobal,
    MinimaxCn,
    Openrouter,
    LmStudio,
    Groq,
    XAi,
    MistralAi,
    Perplexity,
    Volcengine,
    Chatglm,
    #[serde(rename = "copilot", alias = "github_copilot")]
    Copilot,
    #[serde(
        rename = "custom-openai-compatible",
        alias = "openai_compatible",
        alias = "open_ai_compatible"
    )]
    CustomOpenAiCompatible,
}

impl BuiltinLlmProvider {
    pub fn default_api_host(&self) -> &str {
        match self {
            Self::GoogleTranslateFree => "https://translate.googleapis.com/translate_a/single",
            Self::GoogleTranslate => "https://translation.googleapis.com/language/translate/v2",
            Self::OpenAi | Self::OpenAiResponses => "https://api.openai.com",
            Self::AzureOpenai => "",
            Self::Anthropic => "https://api.anthropic.com",
            Self::Gemini => "https://generativelanguage.googleapis.com",
            Self::Ollama => "http://127.0.0.1:11434",
            Self::DeepSeek => "https://api.deepseek.com",
            Self::MoonshotAi => "https://api.moonshot.ai",
            Self::MoonshotCn | Self::Kimi => "https://api.moonshot.cn",
            Self::Xiaomi => "https://api.xiaomimimo.com",
            Self::SiliconFlow => "https://api.siliconflow.cn",
            Self::Qwen => "https://dashscope.aliyuncs.com/compatible-mode/v1",
            Self::QwenPortal => "https://portal.qwen.ai/v1",
            Self::MinimaxGlobal => "https://api.minimaxi.chat/v1",
            Self::MinimaxCn => "https://api.minimax.chat/v1",
            Self::Openrouter => "https://openrouter.ai/api/v1",
            Self::LmStudio => "http://localhost:1234/v1",
            Self::Groq => "https://api.groq.com/openai",
            Self::XAi => "https://api.x.ai",
            Self::MistralAi => "https://api.mistral.ai/v1",
            Self::Perplexity => "https://api.perplexity.ai",
            Self::Volcengine => "https://ark.cn-beijing.volces.com",
            Self::Chatglm => "https://open.bigmodel.cn/api/paas/v4/",
            Self::Copilot => "https://api.githubcopilot.com",
            Self::CustomOpenAiCompatible => "",
        }
    }

    pub fn requires_api_key(&self) -> bool {
        !matches!(
            self,
            Self::GoogleTranslateFree | Self::Ollama | Self::LmStudio
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "specta", derive(Type))]
pub enum LlmProvider {
    Builtin(BuiltinLlmProvider),
    Custom(String),
}

impl LlmProvider {
    pub fn as_str(&self) -> String {
        match self {
            Self::Builtin(b) => serde_json::to_string(b)
                .unwrap_or_default()
                .trim_matches('"')
                .to_string(),
            Self::Custom(c) => c.clone(),
        }
    }
}

impl_fallback_deserialize!(LlmProvider, BuiltinLlmProvider);

// Add a helper trait to map common enum patterns
pub trait IntoLlmProvider {
    fn into_provider(self) -> LlmProvider;
}

impl IntoLlmProvider for &str {
    fn into_provider(self) -> LlmProvider {
        let json_str = format!("\"{}\"", self);
        match serde_json::from_str::<BuiltinLlmProvider>(&json_str) {
            Ok(builtin) => LlmProvider::Builtin(builtin),
            Err(_) => LlmProvider::Custom(self.to_string()),
        }
    }
}

impl IntoLlmProvider for String {
    fn into_provider(self) -> LlmProvider {
        self.as_str().into_provider()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_provider_keeps_transport_string() {
        assert_eq!(
            LlmProvider::Builtin(BuiltinLlmProvider::OpenAi).as_str(),
            "open_ai"
        );
    }

    #[test]
    fn provider_deserializes_custom_flat_string() {
        let provider: LlmProvider = serde_json::from_str("\"private-gateway\"").unwrap();

        assert_eq!(provider, LlmProvider::Custom("private-gateway".to_string()));
    }
}
