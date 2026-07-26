use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use sona_runtime_fs::{SystemClock, UuidGenerator};
use sona_sqlite::{DatabaseError, SqliteApplicationContext, SqliteHistoryStore};
use sona_sync::SyncApplication;

use crate::sync_secret_store_bridge::{FfiSecretStore, HostSyncSecretStore};

const DEFAULT_CONTEXT_CACHE_CAPACITY: usize = 8;

pub(crate) struct HostApplicationContext {
    sqlite: Arc<SqliteApplicationContext>,
    sync: OnceLock<Arc<SyncApplication>>,
    sync_secret_store: Arc<HostSyncSecretStore>,
}

impl HostApplicationContext {
    fn open(
        app_data_dir: &Path,
        sync_secret_store: Option<Arc<dyn FfiSecretStore>>,
    ) -> Result<Self, DatabaseError> {
        Ok(Self {
            sqlite: Arc::new(SqliteApplicationContext::open(app_data_dir)?),
            sync: OnceLock::new(),
            sync_secret_store: Arc::new(HostSyncSecretStore::new(sync_secret_store)),
        })
    }

    pub(crate) fn sqlite(&self) -> &Arc<SqliteApplicationContext> {
        &self.sqlite
    }

    pub(crate) fn history_store(&self) -> SqliteHistoryStore {
        self.sqlite
            .history_store(Arc::new(SystemClock), Arc::new(UuidGenerator))
    }

    pub(crate) fn sync_application(
        &self,
        initialize: impl FnOnce(&SqliteApplicationContext) -> Arc<SyncApplication>,
    ) -> Arc<SyncApplication> {
        Arc::clone(self.sync.get_or_init(|| initialize(self.sqlite.as_ref())))
    }

    pub(crate) fn sync_secret_store(&self) -> Arc<HostSyncSecretStore> {
        Arc::clone(&self.sync_secret_store)
    }

    fn register_sync_secret_store(&self, store: Arc<dyn FfiSecretStore>) {
        self.sync_secret_store.register(store);
    }

    fn has_active_sync_handle(&self) -> bool {
        self.sync
            .get()
            .is_some_and(|application| Arc::strong_count(application) > 1)
    }
}

struct CachedContext {
    context: Arc<HostApplicationContext>,
    last_used: u64,
}

pub(crate) struct ApplicationContextRegistry {
    capacity: usize,
    access_sequence: u64,
    entries: HashMap<PathBuf, CachedContext>,
    default_sync_secret_store: Option<Arc<dyn FfiSecretStore>>,
    sync_secret_store_overrides: HashMap<PathBuf, Arc<dyn FfiSecretStore>>,
}

impl ApplicationContextRegistry {
    fn new() -> Self {
        Self::with_capacity(DEFAULT_CONTEXT_CACHE_CAPACITY)
    }

    pub(crate) fn with_capacity(capacity: usize) -> Self {
        assert!(
            capacity > 0,
            "application context cache capacity must be positive"
        );
        Self {
            capacity,
            access_sequence: 0,
            entries: HashMap::new(),
            default_sync_secret_store: None,
            sync_secret_store_overrides: HashMap::new(),
        }
    }

    pub(crate) fn get_or_open(
        &mut self,
        app_data_dir: &Path,
    ) -> Result<Arc<HostApplicationContext>, DatabaseError> {
        let key = SqliteApplicationContext::normalize_writable_app_data_dir(app_data_dir)?;
        self.access_sequence = self.access_sequence.wrapping_add(1);
        if let Some(cached) = self.entries.get_mut(&key) {
            cached.last_used = self.access_sequence;
            let context = Arc::clone(&cached.context);
            self.trim_to_capacity(self.capacity, Some(&key));
            return Ok(context);
        }

        let sync_secret_store = self
            .sync_secret_store_overrides
            .get(&key)
            .cloned()
            .or_else(|| self.default_sync_secret_store.clone());
        let context = Arc::new(HostApplicationContext::open(&key, sync_secret_store)?);
        self.trim_to_capacity(self.capacity.saturating_sub(1), None);
        self.entries.insert(
            key,
            CachedContext {
                context: Arc::clone(&context),
                last_used: self.access_sequence,
            },
        );
        Ok(context)
    }

    /// Evicts least-recently-used entries down to `target_len`.
    ///
    /// Capacity is a soft limit. Evicting an entry a caller still holds would
    /// not free anything — the caller keeps its `Arc` — but the next lookup for
    /// that path would miss and open a *second* `SqliteApplicationContext` for
    /// it: a second connection pool, a second migration run, and a second
    /// service graph for one application-data directory. That breaks the
    /// one-context-per-directory invariant the cache exists to uphold, so an
    /// in-use entry is never evicted and the cache is allowed to exceed
    /// `capacity` until its callers release. The loop stops when nothing is
    /// evictable, so a fully in-use cache stops trimming instead of spinning.
    fn trim_to_capacity(&mut self, target_len: usize, retained: Option<&Path>) {
        while self.entries.len() > target_len {
            let Some(evicted) = self
                .entries
                .iter()
                .filter(|(path, cached)| {
                    retained != Some(path.as_path()) && Self::is_evictable(cached)
                })
                .min_by_key(|(_, cached)| cached.last_used)
                .map(|(path, _)| path.clone())
            else {
                break;
            };
            self.entries.remove(&evicted);
        }
    }

    /// An entry is evictable only when the cache is its sole owner. The sync
    /// check is still needed on top: a caller may drop the context handle while
    /// keeping the `SyncApplication` it produced.
    fn is_evictable(cached: &CachedContext) -> bool {
        Arc::strong_count(&cached.context) == 1 && !cached.context.has_active_sync_handle()
    }

    fn get_cached(&mut self, app_data_dir: &Path) -> Option<Arc<HostApplicationContext>> {
        let key = SqliteApplicationContext::normalize_existing_app_data_dir(app_data_dir).ok()?;
        self.access_sequence = self.access_sequence.wrapping_add(1);
        let cached = self.entries.get_mut(&key)?;
        cached.last_used = self.access_sequence;
        Some(Arc::clone(&cached.context))
    }

    pub(crate) fn register_default_sync_secret_store(&mut self, store: Arc<dyn FfiSecretStore>) {
        self.default_sync_secret_store = Some(Arc::clone(&store));
        for (path, cached) in &self.entries {
            if !self.sync_secret_store_overrides.contains_key(path) {
                cached
                    .context
                    .register_sync_secret_store(Arc::clone(&store));
            }
        }
    }

    pub(crate) fn register_sync_secret_store(
        &mut self,
        app_data_dir: &Path,
        store: Arc<dyn FfiSecretStore>,
    ) -> Result<(), DatabaseError> {
        let context = self.get_or_open(app_data_dir)?;
        let key = context.sqlite().app_data_dir().to_path_buf();
        self.sync_secret_store_overrides
            .insert(key, Arc::clone(&store));
        context.register_sync_secret_store(store);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    #[cfg(test)]
    pub(crate) fn contains(&self, app_data_dir: &Path) -> bool {
        SqliteApplicationContext::normalize_existing_app_data_dir(app_data_dir)
            .is_ok_and(|path| self.entries.contains_key(&path))
    }

    pub(crate) fn release(&mut self, app_data_dir: &Path) -> Result<bool, DatabaseError> {
        let key = SqliteApplicationContext::normalize_existing_app_data_dir(app_data_dir)?;
        let context_removed = self.entries.remove(&key).is_some();
        let registration_removed = self.sync_secret_store_overrides.remove(&key).is_some();
        Ok(context_removed || registration_removed)
    }
}

fn registry() -> &'static Mutex<ApplicationContextRegistry> {
    static REGISTRY: OnceLock<Mutex<ApplicationContextRegistry>> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(ApplicationContextRegistry::new()))
}

/// Acquire the registry lock, recovering from a poisoned state.
///
/// A poisoned Mutex means a panic occurred while the lock was held.  The
/// registry is a cache of open database handles; an interrupted operation
/// may leave a stale entry, but the state is not otherwise corrupted.
/// Propagating the poison to FFI callers as a hard error would produce
/// a misleading `DatabaseError::Internal` where none of the usual remedies
/// (retrying, checking the path) would help.  Recovering is the safer and
/// more consistent choice across all call sites, including
/// `register_default_sync_secret_store` which cannot return an error.
///
/// Recovery is deliberate but must not be silent: a poisoned lock means a
/// panic crossed the FFI boundary, which is a defect worth surfacing even
/// though it is not worth failing the call over.
fn lock_registry() -> std::sync::MutexGuard<'static, ApplicationContextRegistry> {
    registry().lock().unwrap_or_else(|poisoned| {
        log::warn!(
            "[UniFFI] application context registry mutex was poisoned by an earlier panic; \
             recovering with the existing cache. A panic crossed the FFI boundary — \
             check preceding logs for its source."
        );
        poisoned.into_inner()
    })
}

/// Where a bridge gets its application context.
///
/// The legacy free functions carry an application-data directory and resolve it
/// through the registry on every call. An explicit context handle already owns
/// one, so it hands the context over directly and never touches the registry.
/// Bridges are generic over this, which keeps one implementation serving both
/// surfaces instead of duplicating each operation.
/// Owned rather than borrowed so `impl Into<ContextSource>` works in `async fn`
/// signatures too. The extra `PathBuf` allocation on the legacy path is
/// negligible beside the database work that follows it.
#[derive(Clone)]
pub(crate) enum ContextSource {
    Path(PathBuf),
    Owned(Arc<HostApplicationContext>),
}

impl ContextSource {
    pub(crate) fn resolve(self) -> Result<Arc<HostApplicationContext>, DatabaseError> {
        match self {
            Self::Path(path) => lock_registry().get_or_open(&path),
            Self::Owned(context) => Ok(context),
        }
    }

    /// Resolves without opening: `None` when nothing is cached for a path. An
    /// owned context is always present.
    pub(crate) fn resolve_cached(
        self,
    ) -> Result<Option<Arc<HostApplicationContext>>, DatabaseError> {
        match self {
            Self::Path(path) => Ok(lock_registry().get_cached(&path)),
            Self::Owned(context) => Ok(Some(context)),
        }
    }

    /// The absolute application-data directory this source refers to.
    ///
    /// Several bridges need the directory itself — to scan storage, to measure
    /// files, to check a history directory — as well as the context. A path
    /// source is made absolute exactly as those bridges did themselves; an
    /// owned context reports the directory the registry already normalized when
    /// it opened, so both sources agree on one directory.
    pub(crate) fn app_data_dir(&self) -> std::io::Result<PathBuf> {
        match self {
            Self::Path(path) => std::path::absolute(path),
            Self::Owned(context) => Ok(context.sqlite().app_data_dir().to_path_buf()),
        }
    }
}

impl From<&str> for ContextSource {
    fn from(value: &str) -> Self {
        Self::Path(PathBuf::from(value))
    }
}

impl From<String> for ContextSource {
    fn from(value: String) -> Self {
        Self::Path(PathBuf::from(value))
    }
}

impl From<&Path> for ContextSource {
    fn from(value: &Path) -> Self {
        Self::Path(value.to_path_buf())
    }
}

impl From<PathBuf> for ContextSource {
    fn from(value: PathBuf) -> Self {
        Self::Path(value)
    }
}

impl From<&Arc<HostApplicationContext>> for ContextSource {
    fn from(value: &Arc<HostApplicationContext>) -> Self {
        Self::Owned(Arc::clone(value))
    }
}

impl From<Arc<HostApplicationContext>> for ContextSource {
    fn from(value: Arc<HostApplicationContext>) -> Self {
        Self::Owned(value)
    }
}

/// Test-only shorthand. Production code goes through `ContextSource` so the
/// registry lookup stays at one edge instead of being reachable from anywhere.
#[cfg(test)]
pub(crate) fn application_context(
    app_data_dir: impl AsRef<Path>,
) -> Result<Arc<HostApplicationContext>, DatabaseError> {
    lock_registry().get_or_open(app_data_dir.as_ref())
}

pub(crate) fn register_default_sync_secret_store(store: Arc<dyn FfiSecretStore>) {
    lock_registry().register_default_sync_secret_store(store);
}

pub(crate) fn register_sync_secret_store_for_app_data_dir(
    app_data_dir: impl AsRef<Path>,
    store: Arc<dyn FfiSecretStore>,
) -> Result<(), DatabaseError> {
    lock_registry().register_sync_secret_store(app_data_dir.as_ref(), store)
}

pub(crate) fn release_application_context(
    app_data_dir: impl AsRef<Path>,
) -> Result<bool, DatabaseError> {
    lock_registry().release(app_data_dir.as_ref())
}
