use parking_lot::RwLock;
use sona_core::ports::translation::TranslationPortError;
use sona_translation_onnx::OnnxNmtAdapter;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

pub fn normalize_language_code(code: &str) -> &str {
    let trimmed = code.trim();
    if trimmed.len() >= 2 {
        let prefix = &trimmed[..2];
        if prefix.eq_ignore_ascii_case("zh")
            || (trimmed.len() >= 3 && trimmed[..3].eq_ignore_ascii_case("cmn"))
        {
            return "zh";
        }
        if prefix.eq_ignore_ascii_case("en") {
            return "en";
        }
        if prefix.eq_ignore_ascii_case("ja")
            || (trimmed.len() >= 3 && trimmed[..3].eq_ignore_ascii_case("jpn"))
        {
            return "ja";
        }
    }
    trimmed
}

type TranslationModelCache = Arc<RwLock<HashMap<(String, String), Arc<OnnxNmtAdapter>>>>;

pub struct TranslationModelPool {
    cache: TranslationModelCache,
}

impl Default for TranslationModelPool {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslationModelPool {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn get_or_load(
        &self,
        source_lang: &str,
        target_lang: &str,
        models_dir: &Path,
    ) -> Result<Arc<OnnxNmtAdapter>, TranslationPortError> {
        let src = normalize_language_code(source_lang);
        let tgt = normalize_language_code(target_lang);

        let key = (src.to_string(), tgt.to_string());

        // Fast path: read lock
        {
            let cache = self.cache.read();
            if let Some(adapter) = cache.get(&key) {
                return Ok(Arc::clone(adapter));
            }
        }

        let model_id = format!("opus-mt-{src}-{tgt}");
        let Some(preset) = sona_core::models::preset_models::find_preset_model(&model_id) else {
            return Err(TranslationPortError::invalid_language(format!(
                "No preset translation model found for language pair {src}->{tgt} ({model_id})"
            )));
        };

        let model_dir = preset.resolve_install_path(models_dir);
        if !model_dir.is_dir() {
            return Err(TranslationPortError::model_not_ready(format!(
                "Model directory does not exist: {}",
                model_dir.display()
            )));
        }

        // Check artifacts presence
        for artifact in &preset.artifacts {
            let file = model_dir.join(&artifact.filename);
            if !file.is_file() {
                return Err(TranslationPortError::model_not_ready(format!(
                    "Missing model file: {}",
                    file.display()
                )));
            }
        }

        let adapter = Arc::new(OnnxNmtAdapter::new(Some(model_dir), 2));
        adapter.ensure_loaded()?;

        let mut cache = self.cache.write();
        if let Some(existing) = cache.get(&key) {
            Ok(Arc::clone(existing))
        } else {
            cache.insert(key, Arc::clone(&adapter));
            Ok(adapter)
        }
    }

    pub fn unload_all(&self) {
        let mut cache = self.cache.write();
        cache.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_language_code() {
        assert_eq!(normalize_language_code("en-US"), "en");
        assert_eq!(normalize_language_code("en"), "en");
        assert_eq!(normalize_language_code("EN"), "en");
        assert_eq!(normalize_language_code("En-GB"), "en");
        assert_eq!(normalize_language_code("zh-CN"), "zh");
        assert_eq!(normalize_language_code("ZH"), "zh");
        assert_eq!(normalize_language_code("cmn-Hans"), "zh");
        assert_eq!(normalize_language_code("CMN"), "zh");
        assert_eq!(normalize_language_code("ja-JP"), "ja");
        assert_eq!(normalize_language_code("JA"), "ja");
        assert_eq!(normalize_language_code("jpn"), "ja");
        assert_eq!(normalize_language_code("fr"), "fr");
    }

    #[test]
    fn test_pool_missing_model_directory() {
        let pool = TranslationModelPool::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let Err(err) = pool.get_or_load("en", "zh", temp_dir.path()) else {
            panic!("expected error");
        };
        assert_eq!(
            err.kind,
            sona_core::ports::translation::TranslationPortErrorKind::ModelNotReady
        );
        assert!(err.message.contains("Model directory does not exist"));
    }

    #[test]
    fn test_pool_invalid_language_pair() {
        let pool = TranslationModelPool::new();
        let temp_dir = tempfile::tempdir().unwrap();
        let Err(err) = pool.get_or_load("en", "fr", temp_dir.path()) else {
            panic!("expected error");
        };
        assert_eq!(
            err.kind,
            sona_core::ports::translation::TranslationPortErrorKind::InvalidLanguage
        );
        assert!(err.message.contains("No preset translation model found"));
    }
}
