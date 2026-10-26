use parking_lot::{Condvar, Mutex, RwLock, RwLockUpgradableReadGuard};
use sona_core::ports::translation::TranslationPortError;
use sona_translation_onnx::OnnxNmtAdapter;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

pub fn normalize_language_code(code: &str) -> &str {
    let trimmed = code.trim();
    if trimmed.starts_with("中文") {
        return "zh";
    }
    if let Some(prefix) = trimmed.get(..2) {
        if prefix.eq_ignore_ascii_case("zh")
            || trimmed
                .get(..3)
                .is_some_and(|p| p.eq_ignore_ascii_case("cmn"))
        {
            return "zh";
        }
        if prefix.eq_ignore_ascii_case("en") {
            return "en";
        }
        if prefix.eq_ignore_ascii_case("ja")
            || trimmed
                .get(..3)
                .is_some_and(|p| p.eq_ignore_ascii_case("jpn"))
        {
            return "ja";
        }
    }
    trimmed
}

struct TranslationModelCacheInner {
    models: HashMap<(String, String), Arc<OnnxNmtAdapter>>,
    order: Vec<(String, String)>,
}

type TranslationModelCache = Arc<RwLock<TranslationModelCacheInner>>;
type LoadingNotifier = (Mutex<HashSet<(String, String)>>, Condvar);
struct InFlightGuard<'a> {
    loading: &'a LoadingNotifier,
    key: (String, String),
}

impl<'a> Drop for InFlightGuard<'a> {
    fn drop(&mut self) {
        let (lock, cvar) = self.loading;
        let mut in_flight = lock.lock();
        in_flight.remove(&self.key);
        cvar.notify_all();
    }
}

pub struct TranslationModelPool {
    cache: TranslationModelCache,
    loading: Arc<LoadingNotifier>,
}

impl Default for TranslationModelPool {
    fn default() -> Self {
        Self::new()
    }
}

impl TranslationModelPool {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(RwLock::new(TranslationModelCacheInner {
                models: HashMap::new(),
                order: Vec::new(),
            })),
            loading: Arc::new((Mutex::new(HashSet::new()), Condvar::new())),
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

        // Fast path: upgradable read lock
        {
            let cache = self.cache.upgradable_read();
            if let Some(adapter) = cache.models.get(&key) {
                let adapter = Arc::clone(adapter);
                if cache.order.first() != Some(&key) {
                    let mut cache = RwLockUpgradableReadGuard::upgrade(cache);
                    if let Some(pos) = cache.order.iter().position(|k| k == &key) {
                        let k = cache.order.remove(pos);
                        cache.order.insert(0, k);
                    }
                }
                return Ok(adapter);
            }
        }

        // Single-flight synchronization for model loading
        let (lock, cvar) = &*self.loading;
        let mut in_flight = lock.lock();
        loop {
            {
                let cache = self.cache.upgradable_read();
                if let Some(adapter) = cache.models.get(&key) {
                    let adapter = Arc::clone(adapter);
                    if cache.order.first() != Some(&key) {
                        let mut cache = RwLockUpgradableReadGuard::upgrade(cache);
                        if let Some(pos) = cache.order.iter().position(|k| k == &key) {
                            let k = cache.order.remove(pos);
                            cache.order.insert(0, k);
                        }
                    }
                    return Ok(adapter);
                }
            }

            if in_flight.insert(key.clone()) {
                break;
            }
            cvar.wait(&mut in_flight);
        }
        drop(in_flight);

        let _guard = InFlightGuard {
            loading: &self.loading,
            key: key.clone(),
        };

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
            if !file.is_file()
                && artifact.filename == "vocab.json"
                && let Some(expected_sha) = &artifact.sha256
                && let Err(e) = try_download_vocab(&artifact.url, &file, expected_sha)
            {
                log::warn!(
                    "[TranslationModelPool] Failed to auto-download missing vocab.json: {e}"
                );
            }
            if !file.is_file() {
                return Err(TranslationPortError::model_not_ready(format!(
                    "Missing model file: {}. Please download vocab.json or update the model in settings.",
                    file.display()
                )));
            }
        }

        let adapter = Arc::new(OnnxNmtAdapter::new(Some(model_dir), 2));
        adapter.ensure_loaded()?;

        let mut cache = self.cache.write();
        if let Some(existing) = cache.models.get(&key).cloned() {
            if let Some(pos) = cache.order.iter().position(|k| k == &key) {
                let k = cache.order.remove(pos);
                cache.order.insert(0, k);
            }
            Ok(existing)
        } else {
            while cache.models.len() >= 2 {
                if let Some(oldest) = cache.order.pop() {
                    cache.models.remove(&oldest);
                } else {
                    break;
                }
            }
            cache.order.insert(0, key.clone());
            cache.models.insert(key, Arc::clone(&adapter));
            Ok(adapter)
        }
    }

    pub fn unload_all(&self) {
        let mut cache = self.cache.write();
        cache.models.clear();
        cache.order.clear();
    }
}

fn try_download_vocab(url: &str, target_path: &Path, expected_sha256: &str) -> Result<(), String> {
    log::info!(
        "[TranslationModelPool] Attempting auto-heal download of vocab.json to {}",
        target_path.display()
    );
    let temp_path = sona_model_downloads::temporary_download_path(target_path);
    let cancel = std::sync::Arc::new(tokio::sync::Notify::new());
    let client = sona_model_downloads::DownloadClient::try_new()
        .map_err(|e| format!("Failed to create download client: {e}"))?;

    let fut = async {
        client
            .download_file(url, &temp_path, cancel, None)
            .await
            .map_err(|e| format!("Download error: {e}"))?;
        sona_model_downloads::complete_download_file(
            &temp_path,
            target_path,
            Some(expected_sha256),
        )
        .await
        .map_err(|e| format!("Verification error: {e}"))?;
        Ok::<(), String>(())
    };

    match tokio::runtime::Handle::try_current() {
        Ok(handle) => match handle.runtime_flavor() {
            tokio::runtime::RuntimeFlavor::MultiThread => {
                tokio::task::block_in_place(|| handle.block_on(fut))
            }
            _ => std::thread::scope(|s| {
                s.spawn(|| {
                    let rt = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|e| format!("Failed to create worker runtime: {e}"))?;
                    rt.block_on(fut)
                })
                .join()
                .unwrap_or_else(|_| Err("Worker thread panicked".to_string()))
            }),
        },
        Err(_) => {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("Failed to create runtime: {e}"))?;
            rt.block_on(fut)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sona_core::ports::translation::TranslationEnginePort;

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
        assert_eq!(normalize_language_code("中文"), "zh");
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

    #[test]
    fn test_pool_unload_all() {
        let pool = TranslationModelPool::new();
        {
            let mut cache = pool.cache.write();
            cache.models.insert(
                ("en".into(), "zh".into()),
                Arc::new(OnnxNmtAdapter::new(None, 1)),
            );
            cache.order.push(("en".into(), "zh".into()));
            assert_eq!(cache.models.len(), 1);
        }
        pool.unload_all();
        let cache = pool.cache.read();
        assert!(cache.models.is_empty());
        assert!(cache.order.is_empty());
    }

    #[test]
    fn test_pool_lru_promotion_on_read_hit() {
        let pool = TranslationModelPool::new();
        let key1 = ("en".to_string(), "zh".to_string());
        let key2 = ("zh".to_string(), "en".to_string());
        {
            let mut cache = pool.cache.write();
            cache
                .models
                .insert(key1.clone(), Arc::new(OnnxNmtAdapter::new(None, 1)));
            cache
                .models
                .insert(key2.clone(), Arc::new(OnnxNmtAdapter::new(None, 1)));
            cache.order.push(key1.clone());
            cache.order.push(key2.clone());
        }

        let temp_dir = tempfile::tempdir().unwrap();
        let adapter = pool.get_or_load("zh", "en", temp_dir.path()).unwrap();
        assert_eq!(
            adapter.engine_kind(),
            sona_core::ports::translation::TranslationEngineKind::OnnxNmt
        );

        let cache = pool.cache.read();
        assert_eq!(cache.order.first(), Some(&key2));
        assert_eq!(cache.order.get(1), Some(&key1));
    }
}
