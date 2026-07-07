use crate::punctuation::Punctuation;
use crate::recognizer::Recognizer;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OnceCell};

type RecognizerCell = Arc<OnceCell<Arc<Recognizer>>>;
type PunctuationCell = Arc<OnceCell<Arc<Punctuation>>>;

#[derive(Clone)]
pub struct RecognizerPool {
    pub recognizers: Arc<Mutex<HashMap<ModelConfigKey, RecognizerCell>>>,
    pub punctuations: Arc<Mutex<HashMap<String, PunctuationCell>>>,
}

impl Default for RecognizerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl RecognizerPool {
    pub fn new() -> Self {
        Self {
            recognizers: Arc::new(Mutex::new(HashMap::new())),
            punctuations: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ModelConfigKey {
    pub model_path: String,
    pub model_type: String,
    pub num_threads: i32,
    pub enable_itn: bool,
    pub language: String,
    pub hotwords: Option<String>,
    pub gpu_provider: Option<String>,
}

impl ModelConfigKey {
    pub fn new(
        model_path: String,
        model_type: String,
        num_threads: i32,
        enable_itn: bool,
        language: String,
        hotwords: Option<String>,
        gpu_provider: Option<String>,
    ) -> Self {
        Self {
            model_path,
            model_type,
            num_threads,
            enable_itn,
            language,
            hotwords,
            gpu_provider,
        }
    }

    pub fn with_gpu_provider(&self, gpu_provider: Option<String>) -> Self {
        Self {
            gpu_provider,
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(provider: Option<&str>) -> ModelConfigKey {
        ModelConfigKey {
            model_path: "C:/models/demo".to_string(),
            model_type: "sensevoice".to_string(),
            num_threads: 4,
            enable_itn: true,
            language: "auto".to_string(),
            hotwords: None,
            gpu_provider: provider.map(str::to_string),
        }
    }

    #[test]
    fn model_config_key_separates_gpu_provider() {
        assert_ne!(key(Some("cpu")), key(Some("cuda")));
        assert_ne!(key(Some("cpu")), key(None));
        assert_eq!(key(Some("cpu")), key(Some("cpu")));
    }
}
