use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use parking_lot::{Condvar, Mutex, RwLock};
use sona_core::ports::llm::{LlmPortError, LlmPortErrorKind};
use sona_core::ports::translation::{
    TranslationEngineKind, TranslationEnginePort, TranslationPortError,
};
use sona_llama_cpp::llama_cpp_2::llama_backend::LlamaBackend;
use sona_llama_cpp::llama_cpp_2::model::LlamaModel;
use sona_llama_cpp::llama_cpp_2::model::params::LlamaModelParams;
use sona_llama_cpp::{LlamaModelHandle, LlamaModelProvider};
use sona_translation_onnx::OnnxNmtAdapter;
use tokio::sync::{Mutex as TokioMutex, Notify};

pub const DEFAULT_IDLE_TTL_SECS: u64 = 300;
pub const DEFAULT_MAX_IDLE_LLM: usize = 1;
pub const DEFAULT_MAX_IDLE_NMT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelReleaseMode {
    Standard,
    GameMode,
}

impl From<&str> for ModelReleaseMode {
    fn from(s: &str) -> Self {
        if s.eq_ignore_ascii_case("game") {
            ModelReleaseMode::GameMode
        } else {
            ModelReleaseMode::Standard
        }
    }
}

pub struct ModelEntry<T> {
    pub instance: Arc<T>,
    pub active_leases: Arc<AtomicUsize>,
    pub last_used_at: Arc<Mutex<Instant>>,
}

pub struct ModelLease<T: Send + Sync + 'static> {
    model: Arc<T>,
    active_leases: Arc<AtomicUsize>,
    last_used_at: Arc<Mutex<Instant>>,
    reaper_notify: Arc<Notify>,
    is_game_mode: Arc<AtomicBool>,
}

impl<T: Send + Sync + 'static> ModelLease<T> {
    pub fn model_arc(&self) -> Arc<T> {
        Arc::clone(&self.model)
    }
}

impl<T: Send + Sync + 'static> Deref for ModelLease<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.model
    }
}

#[async_trait]
impl TranslationEnginePort for ModelLease<OnnxNmtAdapter> {
    async fn translate(
        &self,
        text: &str,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<String, TranslationPortError> {
        self.model.translate(text, source_lang, target_lang).await
    }

    fn engine_kind(&self) -> TranslationEngineKind {
        TranslationEngineKind::OnnxNmt
    }
}

impl<T: Send + Sync + 'static> Drop for ModelLease<T> {
    fn drop(&mut self) {
        let prev = self.active_leases.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            *self.last_used_at.lock() = Instant::now();
            if self.is_game_mode.load(Ordering::Relaxed) {
                self.reaper_notify.notify_one();
            }
        }
    }
}

type LlamaCacheKey = (PathBuf, u32);
type OnnxCacheKey = (String, String);
type OnnxLoadingNotifier = (Mutex<HashSet<OnnxCacheKey>>, Condvar);

pub struct LocalModelPool {
    llama_models: Mutex<HashMap<LlamaCacheKey, Arc<ModelEntry<LlamaModel>>>>,
    onnx_models: Mutex<HashMap<OnnxCacheKey, Arc<ModelEntry<OnnxNmtAdapter>>>>,
    llama_in_flight: Arc<TokioMutex<HashSet<String>>>,
    llama_notify: Arc<Notify>,
    onnx_loading: Arc<OnnxLoadingNotifier>,
    reaper_notify: Arc<Notify>,
    release_mode: RwLock<ModelReleaseMode>,
    is_game_mode: Arc<AtomicBool>,
    idle_ttl: RwLock<Duration>,
    max_idle_llm_count: usize,
    max_idle_nmt_count: usize,
    shutdown: Arc<AtomicBool>,
}

impl Default for LocalModelPool {
    fn default() -> Self {
        Self {
            llama_models: Mutex::new(HashMap::new()),
            onnx_models: Mutex::new(HashMap::new()),
            llama_in_flight: Arc::new(TokioMutex::new(HashSet::new())),
            llama_notify: Arc::new(Notify::new()),
            onnx_loading: Arc::new((Mutex::new(HashSet::new()), Condvar::new())),
            reaper_notify: Arc::new(Notify::new()),
            release_mode: RwLock::new(ModelReleaseMode::Standard),
            is_game_mode: Arc::new(AtomicBool::new(false)),
            idle_ttl: RwLock::new(Duration::from_secs(DEFAULT_IDLE_TTL_SECS)),
            max_idle_llm_count: DEFAULT_MAX_IDLE_LLM,
            max_idle_nmt_count: DEFAULT_MAX_IDLE_NMT,
            shutdown: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl LocalModelPool {
    pub fn new() -> Arc<Self> {
        let pool = Arc::new(Self::default());

        // Spawn background reaper task
        let reaper_pool = Arc::clone(&pool);
        tokio::spawn(async move {
            reaper_pool.run_reaper_loop().await;
        });

        pool
    }

    pub fn set_release_mode(&self, mode: ModelReleaseMode) {
        *self.release_mode.write() = mode;
        let is_game = mode == ModelReleaseMode::GameMode;
        self.is_game_mode.store(is_game, Ordering::Relaxed);
        if is_game {
            self.reaper_notify.notify_one();
        }
    }

    pub fn release_mode(&self) -> ModelReleaseMode {
        *self.release_mode.read()
    }

    pub fn set_idle_ttl(&self, ttl: Duration) {
        *self.idle_ttl.write() = ttl;
    }

    pub fn idle_ttl(&self) -> Duration {
        *self.idle_ttl.read()
    }

    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        self.reaper_notify.notify_one();
    }

    // ------------------------------------------------------------------------
    // LLM (LlamaModel) Management
    // ------------------------------------------------------------------------

    pub async fn acquire_llama_model(
        &self,
        backend: &'static LlamaBackend,
        model_path: &Path,
        n_gpu_layers: u32,
    ) -> Result<ModelLease<LlamaModel>, LlmPortError> {
        let canonical = model_path.canonicalize().map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!(
                    "Failed to canonicalize local model path '{}': {error}",
                    model_path.display()
                ),
            )
        })?;

        let cache_key = (canonical, n_gpu_layers);
        let in_flight_key = format!("llama:{}:{}", cache_key.0.display(), cache_key.1);

        loop {
            // Fast path: check if already loaded
            {
                let cache = self.llama_models.lock();
                if let Some(entry) = cache.get(&cache_key) {
                    entry.active_leases.fetch_add(1, Ordering::SeqCst);
                    return Ok(ModelLease {
                        model: Arc::clone(&entry.instance),
                        active_leases: Arc::clone(&entry.active_leases),
                        last_used_at: Arc::clone(&entry.last_used_at),
                        reaper_notify: Arc::clone(&self.reaper_notify),
                        is_game_mode: Arc::clone(&self.is_game_mode),
                    });
                }
            }

            // Single-flight synchronization
            let mut in_flight = self.llama_in_flight.lock().await;
            if in_flight.contains(&in_flight_key) {
                drop(in_flight);
                self.llama_notify.notified().await;
                continue;
            }
            in_flight.insert(in_flight_key.clone());
            break;
        }

        struct InFlightGuard {
            set: Arc<TokioMutex<HashSet<String>>>,
            key: String,
            notify: Arc<Notify>,
        }
        impl Drop for InFlightGuard {
            fn drop(&mut self) {
                let key = self.key.clone();
                let set = Arc::clone(&self.set);
                let notify = Arc::clone(&self.notify);
                tokio::spawn(async move {
                    let mut lock = set.lock().await;
                    lock.remove(&key);
                    notify.notify_waiters();
                });
            }
        }
        let _guard = InFlightGuard {
            set: Arc::clone(&self.llama_in_flight),
            key: in_flight_key,
            notify: Arc::clone(&self.llama_notify),
        };

        // Evict idle models if capacity exceeded before loading new model
        self.evict_excess_idle_llm().await;

        let path_to_load = cache_key.0.clone();
        let loaded_model = tokio::task::spawn_blocking(move || {
            let params = LlamaModelParams::default().with_n_gpu_layers(n_gpu_layers);
            LlamaModel::load_from_file(backend, &path_to_load, &params)
        })
        .await
        .map_err(|e| LlmPortError::new(LlmPortErrorKind::Unavailable, e.to_string()))?
        .map_err(|error| {
            LlmPortError::new(
                LlmPortErrorKind::Unavailable,
                format!(
                    "Failed to load llama.cpp model from '{}': {error}",
                    cache_key.0.display()
                ),
            )
        })?;

        let model_arc = Arc::new(loaded_model);
        let active_leases = Arc::new(AtomicUsize::new(1));
        let last_used_at = Arc::new(Mutex::new(Instant::now()));

        let entry = Arc::new(ModelEntry {
            instance: Arc::clone(&model_arc),
            active_leases: Arc::clone(&active_leases),
            last_used_at: Arc::clone(&last_used_at),
        });

        {
            let mut cache = self.llama_models.lock();
            cache.insert(cache_key, entry);
        }

        Ok(ModelLease {
            model: model_arc,
            active_leases,
            last_used_at,
            reaper_notify: Arc::clone(&self.reaper_notify),
            is_game_mode: Arc::clone(&self.is_game_mode),
        })
    }

    async fn evict_excess_idle_llm(&self) {
        let to_drop = {
            let mut cache = self.llama_models.lock();
            let mut idle_keys: Vec<(LlamaCacheKey, Instant)> = cache
                .iter()
                .filter(|(_, entry)| entry.active_leases.load(Ordering::Relaxed) == 0)
                .map(|(k, entry)| (k.clone(), *entry.last_used_at.lock()))
                .collect();

            if idle_keys.len() >= self.max_idle_llm_count {
                idle_keys.sort_by_key(|(_, t)| *t);
                let excess = idle_keys.len() - self.max_idle_llm_count + 1;
                let mut dropped = Vec::new();
                for (k, _) in idle_keys.into_iter().take(excess) {
                    if let Some(entry) = cache.remove(&k) {
                        dropped.push(entry);
                    }
                }
                dropped
            } else {
                Vec::new()
            }
        };

        if !to_drop.is_empty() {
            tokio::task::spawn_blocking(move || {
                drop(to_drop);
            })
            .await
            .ok();
        }
    }

    // ------------------------------------------------------------------------
    // ONNX NMT Management
    // ------------------------------------------------------------------------

    pub fn acquire_onnx_model(
        &self,
        source_lang: &str,
        target_lang: &str,
        models_dir: &Path,
    ) -> Result<ModelLease<OnnxNmtAdapter>, TranslationPortError> {
        let src = crate::integrations::translation::normalize_language_code(source_lang);
        let tgt = crate::integrations::translation::normalize_language_code(target_lang);
        let cache_key = (src.to_string(), tgt.to_string());

        // Fast path: check if already loaded
        {
            let cache = self.onnx_models.lock();
            if let Some(entry) = cache.get(&cache_key) {
                entry.active_leases.fetch_add(1, Ordering::SeqCst);
                return Ok(ModelLease {
                    model: Arc::clone(&entry.instance),
                    active_leases: Arc::clone(&entry.active_leases),
                    last_used_at: Arc::clone(&entry.last_used_at),
                    reaper_notify: Arc::clone(&self.reaper_notify),
                    is_game_mode: Arc::clone(&self.is_game_mode),
                });
            }
        }

        // Single-flight synchronization with Condvar
        let (lock, cvar) = &*self.onnx_loading;
        let mut in_flight = lock.lock();
        loop {
            {
                let cache = self.onnx_models.lock();
                if let Some(entry) = cache.get(&cache_key) {
                    entry.active_leases.fetch_add(1, Ordering::SeqCst);
                    return Ok(ModelLease {
                        model: Arc::clone(&entry.instance),
                        active_leases: Arc::clone(&entry.active_leases),
                        last_used_at: Arc::clone(&entry.last_used_at),
                        reaper_notify: Arc::clone(&self.reaper_notify),
                        is_game_mode: Arc::clone(&self.is_game_mode),
                    });
                }
            }

            if in_flight.insert(cache_key.clone()) {
                break;
            }
            cvar.wait(&mut in_flight);
        }
        drop(in_flight);

        struct OnnxInFlightGuard<'a> {
            loading: &'a OnnxLoadingNotifier,
            key: OnnxCacheKey,
        }
        impl<'a> Drop for OnnxInFlightGuard<'a> {
            fn drop(&mut self) {
                let (lock, cvar) = self.loading;
                let mut in_flight = lock.lock();
                in_flight.remove(&self.key);
                cvar.notify_all();
            }
        }
        let _guard = OnnxInFlightGuard {
            loading: &self.onnx_loading,
            key: cache_key.clone(),
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

        for artifact in &preset.artifacts {
            let file = model_dir.join(&artifact.filename);
            if !file.is_file() {
                return Err(TranslationPortError::model_not_ready(format!(
                    "Missing model file: {}. Please download the model in settings.",
                    file.display()
                )));
            }
        }

        self.evict_excess_idle_onnx_sync();

        let adapter = Arc::new(OnnxNmtAdapter::new(Some(model_dir), 2));
        adapter.ensure_loaded()?;

        let active_leases = Arc::new(AtomicUsize::new(1));
        let last_used_at = Arc::new(Mutex::new(Instant::now()));

        let entry = Arc::new(ModelEntry {
            instance: Arc::clone(&adapter),
            active_leases: Arc::clone(&active_leases),
            last_used_at: Arc::clone(&last_used_at),
        });

        {
            let mut cache = self.onnx_models.lock();
            cache.insert(cache_key, entry);
        }

        Ok(ModelLease {
            model: adapter,
            active_leases,
            last_used_at,
            reaper_notify: Arc::clone(&self.reaper_notify),
            is_game_mode: Arc::clone(&self.is_game_mode),
        })
    }

    fn evict_excess_idle_onnx_sync(&self) {
        let mut cache = self.onnx_models.lock();
        let mut idle_keys: Vec<(OnnxCacheKey, Instant)> = cache
            .iter()
            .filter(|(_, entry)| entry.active_leases.load(Ordering::Relaxed) == 0)
            .map(|(k, entry)| (k.clone(), *entry.last_used_at.lock()))
            .collect();

        if idle_keys.len() >= self.max_idle_nmt_count {
            idle_keys.sort_by_key(|(_, t)| *t);
            let excess = idle_keys.len() - self.max_idle_nmt_count + 1;
            for (k, _) in idle_keys.into_iter().take(excess) {
                cache.remove(&k);
            }
        }
    }

    // ------------------------------------------------------------------------
    // Purge & Reaper
    // ------------------------------------------------------------------------

    pub async fn purge_all_idle(&self) {
        let (llm_dropped, onnx_dropped) = {
            let mut llm_cache = self.llama_models.lock();
            let mut onnx_cache = self.onnx_models.lock();

            let mut llm_dropped = Vec::new();
            let mut onnx_dropped = Vec::new();

            llm_cache.retain(|_, entry| {
                if entry.active_leases.load(Ordering::Relaxed) == 0 {
                    llm_dropped.push(Arc::clone(entry));
                    false
                } else {
                    true
                }
            });

            onnx_cache.retain(|_, entry| {
                if entry.active_leases.load(Ordering::Relaxed) == 0 {
                    onnx_dropped.push(Arc::clone(entry));
                    false
                } else {
                    true
                }
            });

            (llm_dropped, onnx_dropped)
        };

        if !llm_dropped.is_empty() || !onnx_dropped.is_empty() {
            tokio::task::spawn_blocking(move || {
                drop(llm_dropped);
                drop(onnx_dropped);
            })
            .await
            .ok();
        }
    }

    async fn run_reaper_loop(&self) {
        while !self.shutdown.load(Ordering::Relaxed) {
            // Wake every 20 seconds, or immediately when reaper_notify fires (e.g. lease drop in GameMode)
            let _ =
                tokio::time::timeout(Duration::from_secs(20), self.reaper_notify.notified()).await;

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }

            let is_game = self.is_game_mode.load(Ordering::Relaxed);
            let ttl = *self.idle_ttl.read();

            let (llm_dropped, onnx_dropped) = {
                let mut llm_cache = self.llama_models.lock();
                let mut onnx_cache = self.onnx_models.lock();

                let mut llm_dropped = Vec::new();
                let mut onnx_dropped = Vec::new();

                llm_cache.retain(|_, entry| {
                    if entry.active_leases.load(Ordering::Relaxed) == 0 {
                        let elapsed = entry.last_used_at.lock().elapsed();
                        if is_game || elapsed >= ttl {
                            llm_dropped.push(Arc::clone(entry));
                            return false;
                        }
                    }
                    true
                });

                onnx_cache.retain(|_, entry| {
                    if entry.active_leases.load(Ordering::Relaxed) == 0 {
                        let elapsed = entry.last_used_at.lock().elapsed();
                        if is_game || elapsed >= ttl {
                            onnx_dropped.push(Arc::clone(entry));
                            return false;
                        }
                    }
                    true
                });

                (llm_dropped, onnx_dropped)
            };

            if !llm_dropped.is_empty() || !onnx_dropped.is_empty() {
                tokio::task::spawn_blocking(move || {
                    drop(llm_dropped);
                    drop(onnx_dropped);
                })
                .await
                .ok();
            }
        }
    }
}

#[async_trait]
impl LlamaModelProvider for LocalModelPool {
    async fn acquire_model(
        &self,
        backend: &'static LlamaBackend,
        model_path: &Path,
        n_gpu_layers: u32,
    ) -> Result<LlamaModelHandle, LlmPortError> {
        let lease = self
            .acquire_llama_model(backend, model_path, n_gpu_layers)
            .await?;
        let model = lease.model_arc();
        Ok(LlamaModelHandle::new(model, Some(Arc::new(lease))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_release_mode_from_str() {
        assert_eq!(ModelReleaseMode::from("game"), ModelReleaseMode::GameMode);
        assert_eq!(ModelReleaseMode::from("GAME"), ModelReleaseMode::GameMode);
        assert_eq!(
            ModelReleaseMode::from("standard"),
            ModelReleaseMode::Standard
        );
        assert_eq!(
            ModelReleaseMode::from("anything_else"),
            ModelReleaseMode::Standard
        );
    }

    #[tokio::test]
    async fn test_pool_release_mode_and_ttl_config() {
        let pool = LocalModelPool::new();
        assert_eq!(pool.release_mode(), ModelReleaseMode::Standard);
        assert_eq!(pool.idle_ttl(), Duration::from_secs(DEFAULT_IDLE_TTL_SECS));

        pool.set_release_mode(ModelReleaseMode::GameMode);
        assert_eq!(pool.release_mode(), ModelReleaseMode::GameMode);
        assert!(pool.is_game_mode.load(Ordering::Relaxed));

        pool.set_idle_ttl(Duration::from_secs(60));
        assert_eq!(pool.idle_ttl(), Duration::from_secs(60));

        pool.shutdown();
    }

    #[tokio::test]
    async fn test_purge_all_idle() {
        let pool = LocalModelPool::new();
        pool.purge_all_idle().await;
        pool.shutdown();
    }
}
