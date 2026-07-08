use serde::{Deserialize, Serialize};
#[cfg(feature = "specta")]
use specta::Type;

#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct ModelFileConfig {
    pub encoder: Option<String>,
    pub decoder: Option<String>,
    pub model: Option<String>,
    pub joiner: Option<String>,
    pub tokens: Option<String>,
    pub conv_frontend: Option<String>,
    pub encoder_adaptor: Option<String>,
    pub llm: Option<String>,
    pub embedding: Option<String>,
    pub tokenizer: Option<String>,
}
