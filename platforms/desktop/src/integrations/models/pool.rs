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
use tokio::sync::Notify;

pub const DEFAULT_IDLE_TTL_SECS: u64 = 300;
pub const DEFAULT_MAX_IDLE_LLM: usize = 1;
pub const DEFAULT_MAX_IDLE_NMT: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelReleaseMode {
    Immediate,
    Standard,
    Never,
}

impl From<&str> for ModelReleaseMode {
    fn from(s: &str) -> Self {
        if s.eq_ignore_ascii_case("immediate") {
            ModelReleaseMode::Immediate
        } else if s.eq_ignore_ascii_case("never") {
            ModelReleaseMode::Never
        } else {
            ModelReleaseMode::Standard
        }
    }
}

pub struct ModelEntry<T> {
    pub instance: Arc<T>,
    pub active_leases: Arc<AtomicUsize>,
    pub last_used_at: Arc<Mutex<Instant>>,
    pub is_switched_out: Arc<AtomicBool>,
}

pub struct ModelLease<T: Send + Sync + 'static> {
    model: Arc<T>,
    active_leases: Arc<AtomicUsize>,
    last_used_at: Arc<Mutex<Instant>>,
    is_switched_out: Arc<AtomicBool>,
    reaper_notify: Arc<Notify>,
    release_mode: Arc<RwLock<ModelReleaseMode>>,
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
            let mode = *self.release_mode.read();
            if self.is_switched_out.load(Ordering::Relaxed) || mode == ModelReleaseMode::Immediate {
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
    llama_in_flight: Arc<Mutex<HashSet<String>>>,
    llama_notify: Arc<Notify>,
    onnx_loading: Arc<OnnxLoadingNotifier>,
    reaper_notify: Arc<Notify>,
    release_mode: Arc<RwLock<ModelReleaseMode>>,
    game_mode_enabled: Arc<AtomicBool>,
    game_mode_processes: Arc<RwLock<HashSet<String>>>,
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
            llama_in_flight: Arc::new(Mutex::new(HashSet::new())),
            llama_notify: Arc::new(Notify::new()),
            onnx_loading: Arc::new((Mutex::new(HashSet::new()), Condvar::new())),
            reaper_notify: Arc::new(Notify::new()),
            release_mode: Arc::new(RwLock::new(ModelReleaseMode::Standard)),
            game_mode_enabled: Arc::new(AtomicBool::new(false)),
            game_mode_processes: Arc::new(RwLock::new(HashSet::new())),
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

        // Spawn background reaper task if a Tokio reactor is active
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let reaper_pool = Arc::clone(&pool);
            handle.spawn(async move {
                reaper_pool.run_reaper_loop().await;
            });
        }

        pool
    }

    pub fn set_release_mode(&self, mode: ModelReleaseMode) {
        *self.release_mode.write() = mode;
        if mode == ModelReleaseMode::Immediate {
            self.reaper_notify.notify_one();
        }
    }

    pub fn release_mode(&self) -> ModelReleaseMode {
        *self.release_mode.read()
    }

    pub fn set_game_mode(&self, enabled: bool, processes: &str) {
        self.game_mode_enabled.store(enabled, Ordering::Relaxed);
        let set: HashSet<String> = processes
            .split([',', '\n', '\r'])
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
            .collect();
        *self.game_mode_processes.write() = set;
        if enabled {
            self.reaper_notify.notify_one();
        }
    }

    pub fn is_game_mode_enabled(&self) -> bool {
        self.game_mode_enabled.load(Ordering::Relaxed)
    }

    pub fn should_purge_on_task_end(&self) -> bool {
        let mode = *self.release_mode.read();
        mode == ModelReleaseMode::Immediate || self.check_game_mode_foreground()
    }

    pub fn check_game_mode_foreground(&self) -> bool {
        if !self.game_mode_enabled.load(Ordering::Relaxed) {
            return false;
        }
        let list = self.game_mode_processes.read();
        if list.is_empty() {
            return false;
        }
        if let Ok(Some(fg)) = crate::platform::system::get_foreground_window_info() {
            let app = fg.app_name.to_lowercase();
            if list.contains(&app) {
                return true;
            }
            let stripped = app.strip_suffix(".exe").unwrap_or(&app);
            if list.contains(stripped) {
                return true;
            }
        }
        false
    }

    pub fn mark_all_switched_out(&self) {
        {
            let cache = self.llama_models.lock();
            for entry in cache.values() {
                entry.is_switched_out.store(true, Ordering::Relaxed);
            }
        }
        {
            let cache = self.onnx_models.lock();
            for entry in cache.values() {
                entry.is_switched_out.store(true, Ordering::Relaxed);
            }
        }
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
                    entry.is_switched_out.store(false, Ordering::Relaxed);
                    return Ok(ModelLease {
                        model: Arc::clone(&entry.instance),
                        active_leases: Arc::clone(&entry.active_leases),
                        last_used_at: Arc::clone(&entry.last_used_at),
                        is_switched_out: Arc::clone(&entry.is_switched_out),
                        reaper_notify: Arc::clone(&self.reaper_notify),
                        release_mode: Arc::clone(&self.release_mode),
                    });
                }
            }

            // Single-flight synchronization with pre-registered notification
            let notified = self.llama_notify.notified();
            tokio::pin!(notified);
            notified.as_mut().enable();

            let must_wait = {
                let mut in_flight = self.llama_in_flight.lock();
                if !in_flight.contains(&in_flight_key) {
                    in_flight.insert(in_flight_key.clone());
                    false
                } else {
                    true
                }
            };

            if !must_wait {
                break;
            }

            let _ = tokio::time::timeout(Duration::from_secs(5), notified).await;
        }

        struct InFlightGuard {
            set: Arc<Mutex<HashSet<String>>>,
            key: String,
            notify: Arc<Notify>,
        }
        impl Drop for InFlightGuard {
            fn drop(&mut self) {
                self.set.lock().remove(&self.key);
                self.notify.notify_waiters();
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
        let is_switched_out = Arc::new(AtomicBool::new(false));

        let entry = Arc::new(ModelEntry {
            instance: Arc::clone(&model_arc),
            active_leases: Arc::clone(&active_leases),
            last_used_at: Arc::clone(&last_used_at),
            is_switched_out: Arc::clone(&is_switched_out),
        });

        {
            let mut cache = self.llama_models.lock();
            cache.insert(cache_key, entry);
        }

        Ok(ModelLease {
            model: model_arc,
            active_leases,
            last_used_at,
            is_switched_out,
            reaper_notify: Arc::clone(&self.reaper_notify),
            release_mode: Arc::clone(&self.release_mode),
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
                entry.is_switched_out.store(false, Ordering::Relaxed);
                return Ok(ModelLease {
                    model: Arc::clone(&entry.instance),
                    active_leases: Arc::clone(&entry.active_leases),
                    last_used_at: Arc::clone(&entry.last_used_at),
                    is_switched_out: Arc::clone(&entry.is_switched_out),
                    reaper_notify: Arc::clone(&self.reaper_notify),
                    release_mode: Arc::clone(&self.release_mode),
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
                    entry.is_switched_out.store(false, Ordering::Relaxed);
                    return Ok(ModelLease {
                        model: Arc::clone(&entry.instance),
                        active_leases: Arc::clone(&entry.active_leases),
                        last_used_at: Arc::clone(&entry.last_used_at),
                        is_switched_out: Arc::clone(&entry.is_switched_out),
                        reaper_notify: Arc::clone(&self.reaper_notify),
                        release_mode: Arc::clone(&self.release_mode),
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
        let is_switched_out = Arc::new(AtomicBool::new(false));

        let entry = Arc::new(ModelEntry {
            instance: Arc::clone(&adapter),
            active_leases: Arc::clone(&active_leases),
            last_used_at: Arc::clone(&last_used_at),
            is_switched_out: Arc::clone(&is_switched_out),
        });

        {
            let mut cache = self.onnx_models.lock();
            cache.insert(cache_key, entry);
        }

        Ok(ModelLease {
            model: adapter,
            active_leases,
            last_used_at,
            is_switched_out,
            reaper_notify: Arc::clone(&self.reaper_notify),
            release_mode: Arc::clone(&self.release_mode),
        })
    }

    fn evict_excess_idle_onnx_sync(&self) {
        let dropped = {
            let mut cache = self.onnx_models.lock();
            let mut idle_keys: Vec<(OnnxCacheKey, Instant)> = cache
                .iter()
                .filter(|(_, entry)| entry.active_leases.load(Ordering::Relaxed) == 0)
                .map(|(k, entry)| (k.clone(), *entry.last_used_at.lock()))
                .collect();

            if idle_keys.len() >= self.max_idle_nmt_count {
                idle_keys.sort_by_key(|(_, t)| *t);
                let excess = idle_keys.len() - self.max_idle_nmt_count + 1;
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
        drop(dropped);
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
            // Wake every 3 seconds for game mode check or TTL, or immediately when reaper_notify fires
            let _ =
                tokio::time::timeout(Duration::from_secs(3), self.reaper_notify.notified()).await;

            if self.shutdown.load(Ordering::Relaxed) {
                break;
            }

            let mode = *self.release_mode.read();
            let ttl = *self.idle_ttl.read();
            let is_game_fg = self.check_game_mode_foreground();

            let (llm_dropped, onnx_dropped) = {
                let mut llm_cache = self.llama_models.lock();
                let mut onnx_cache = self.onnx_models.lock();

                let mut llm_dropped = Vec::new();
                let mut onnx_dropped = Vec::new();

                llm_cache.retain(|_, entry| {
                    if entry.active_leases.load(Ordering::Relaxed) == 0 {
                        let is_switched = entry.is_switched_out.load(Ordering::Relaxed);
                        let elapsed = entry.last_used_at.lock().elapsed();

                        if is_switched
                            || mode == ModelReleaseMode::Immediate
                            || is_game_fg
                            || (mode == ModelReleaseMode::Standard && elapsed >= ttl)
                        {
                            llm_dropped.push(Arc::clone(entry));
                            return false;
                        }
                    }
                    true
                });

                onnx_cache.retain(|_, entry| {
                    if entry.active_leases.load(Ordering::Relaxed) == 0 {
                        let is_switched = entry.is_switched_out.load(Ordering::Relaxed);
                        let elapsed = entry.last_used_at.lock().elapsed();

                        if is_switched
                            || mode == ModelReleaseMode::Immediate
                            || is_game_fg
                            || (mode == ModelReleaseMode::Standard && elapsed >= ttl)
                        {
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
        assert_eq!(
            ModelReleaseMode::from("immediate"),
            ModelReleaseMode::Immediate
        );
        assert_eq!(
            ModelReleaseMode::from("IMMEDIATE"),
            ModelReleaseMode::Immediate
        );
        assert_eq!(ModelReleaseMode::from("never"), ModelReleaseMode::Never);
        assert_eq!(ModelReleaseMode::from("NEVER"), ModelReleaseMode::Never);
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

        pool.set_release_mode(ModelReleaseMode::Immediate);
        assert_eq!(pool.release_mode(), ModelReleaseMode::Immediate);

        pool.set_game_mode(true, "eldenring.exe, cs2.exe\ncyberpunk2077.exe");
        assert!(pool.is_game_mode_enabled());
        {
            let procs = pool.game_mode_processes.read();
            assert!(procs.contains("eldenring.exe"));
            assert!(procs.contains("cs2.exe"));
            assert!(procs.contains("cyberpunk2077.exe"));
        }

        pool.set_idle_ttl(Duration::from_secs(60));
        assert_eq!(pool.idle_ttl(), Duration::from_secs(60));

        pool.shutdown();
    }

    #[test]
    fn test_pool_new_outside_tokio_runtime() {
        let pool = LocalModelPool::new();
        assert_eq!(pool.release_mode(), ModelReleaseMode::Standard);
        pool.shutdown();
    }

    #[tokio::test]
    async fn test_purge_all_idle() {
        let pool = LocalModelPool::new();
        pool.purge_all_idle().await;
        pool.shutdown();
    }
}
