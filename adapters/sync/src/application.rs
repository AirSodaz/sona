use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sona_core::sync::{
    SyncConflictDetail, SyncConflictResolution, SyncConflictSummary, SyncError, SyncErrorSnapshot,
    SyncLifecycleState, SyncLocalRepository, SyncObjectStore, SyncPresetV1, SyncRepositoryFactory,
    SyncRunResult, SyncSecretStore, SyncStatusSnapshot,
};
use tokio::sync::Mutex;

use crate::runtime::load_remote_state_for_join;
use crate::vault::{
    OpenedRemoteVault, change_remote_master_password, create_remote_vault,
    open_remote_vault_with_password, open_remote_vault_with_recovery_key,
    open_remote_vault_with_vault_key, regenerate_remote_recovery_key, update_remote_vault_preset,
};
use crate::{SyncBackoffPolicy, SyncRuntime};

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum SyncApplicationError {
    #[error("Sync configuration error: {0}")]
    Config(String),
    #[error("Unknown sync provider: {0}")]
    UnknownProvider(String),
    #[error("{0}")]
    InvalidState(String),
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error(transparent)]
    Preset(#[from] SyncPresetChangeError),
}

pub trait SyncApplicationEnvironment: Send + Sync {
    fn now_ms(&self) -> u64;
    fn next_id(&self) -> String;
    fn jitter(&self) -> u32;
}

#[derive(Default)]
pub struct SystemSyncApplicationEnvironment;

impl SyncApplicationEnvironment for SystemSyncApplicationEnvironment {
    fn now_ms(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |duration| {
                u64::try_from(duration.as_millis()).unwrap_or(u64::MAX)
            })
    }

    fn next_id(&self) -> String {
        uuid::Uuid::new_v4().to_string()
    }

    fn jitter(&self) -> u32 {
        (uuid::Uuid::new_v4().as_u128() % 1_000_001) as u32
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncProviderInput {
    pub provider_id: String,
    pub configuration: Value,
}

#[derive(Clone)]
pub struct SyncProvider {
    pub descriptor: sona_core::sync::SyncProviderDescriptor,
    pub store: Arc<dyn SyncObjectStore>,
    pub persisted_configuration: Value,
    pub credential: Vec<u8>,
}

#[async_trait]
pub trait SyncProviderFactory: Send + Sync {
    fn provider_id(&self) -> &str;
    fn credential_secret_key(&self, vault_id: &str) -> String;
    async fn prepare(&self, configuration: Value) -> Result<SyncProvider, SyncError>;
    async fn restore(
        &self,
        persisted_configuration: Value,
        credential: Vec<u8>,
    ) -> Result<SyncProvider, SyncError>;
}

#[derive(Clone, Default)]
pub struct SyncProviderRegistry {
    factories: BTreeMap<String, Arc<dyn SyncProviderFactory>>,
}

impl SyncProviderRegistry {
    pub fn new(factories: impl IntoIterator<Item = Arc<dyn SyncProviderFactory>>) -> Self {
        let mut registry = Self::default();
        for factory in factories {
            registry.register(factory);
        }
        registry
    }

    pub fn register(&mut self, factory: Arc<dyn SyncProviderFactory>) {
        self.factories
            .insert(factory.provider_id().to_string(), factory);
    }

    pub async fn prepare(
        &self,
        input: SyncProviderInput,
    ) -> Result<SyncProvider, SyncApplicationError> {
        self.factory(&input.provider_id)?
            .prepare(input.configuration)
            .await
            .map_err(Into::into)
    }

    pub async fn test_provider(
        &self,
        input: SyncProviderInput,
    ) -> Result<sona_core::sync::SyncProviderDescriptor, SyncApplicationError> {
        let provider = self.prepare(input).await?;
        let capabilities = provider.store.probe().await?;
        if !capabilities.conditional_create
            || !capabilities.compare_and_swap
            || !capabilities.delete
        {
            return Err(SyncApplicationError::InvalidState(
                "Sync provider does not support required conditional object operations."
                    .to_string(),
            ));
        }
        Ok(provider.descriptor)
    }

    pub async fn restore(
        &self,
        provider_id: &str,
        persisted_configuration: Value,
        credential: Vec<u8>,
    ) -> Result<SyncProvider, SyncApplicationError> {
        self.factory(provider_id)?
            .restore(persisted_configuration, credential)
            .await
            .map_err(Into::into)
    }

    pub fn credential_secret_key(
        &self,
        provider_id: &str,
        vault_id: &str,
    ) -> Result<String, SyncApplicationError> {
        Ok(self.factory(provider_id)?.credential_secret_key(vault_id))
    }

    fn factory(
        &self,
        provider_id: &str,
    ) -> Result<&Arc<dyn SyncProviderFactory>, SyncApplicationError> {
        self.factories
            .get(provider_id)
            .ok_or_else(|| SyncApplicationError::UnknownProvider(provider_id.to_string()))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncCreateResult {
    pub vault_id: String,
    pub device_id: String,
    pub recovery_key: Option<String>,
    pub status: SyncStatusSnapshot,
}

struct UnlockedSession {
    provider: SyncProvider,
    opened: OpenedRemoteVault,
}

struct SyncRunGuard<'a>(&'a AtomicBool);

impl Drop for SyncRunGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

pub struct SyncApplication {
    config_store: Arc<dyn SyncConfigStore>,
    repository_factory: Arc<dyn SyncRepositoryFactory>,
    providers: SyncProviderRegistry,
    secret_store: Arc<dyn SyncSecretStore>,
    environment: Arc<dyn SyncApplicationEnvironment>,
    lifecycle: Mutex<()>,
    session: Mutex<Option<UnlockedSession>>,
    syncing: AtomicBool,
    manually_locked: AtomicBool,
}

impl SyncApplication {
    pub fn new(
        config_store: Arc<dyn SyncConfigStore>,
        repository_factory: Arc<dyn SyncRepositoryFactory>,
        providers: SyncProviderRegistry,
        secret_store: Arc<dyn SyncSecretStore>,
        environment: Arc<dyn SyncApplicationEnvironment>,
    ) -> Self {
        Self {
            config_store,
            repository_factory,
            providers,
            secret_store,
            environment,
            lifecycle: Mutex::new(()),
            session: Mutex::new(None),
            syncing: AtomicBool::new(false),
            manually_locked: AtomicBool::new(false),
        }
    }

    pub async fn status(&self) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let Some(config) = self.config_store.load()? else {
            return Ok(disabled_sync_status());
        };
        if !self.manually_locked.load(Ordering::SeqCst) {
            let _ = self.restore_session(&config).await;
        }
        let repository = self.repository()?.ok_or_else(|| {
            SyncApplicationError::InvalidState(
                "Sync connection metadata exists but local sync state is missing.".to_string(),
            )
        })?;
        let runtime = repository.runtime_repository().load_runtime_state()?;
        let unlocked = self.session.lock().await.is_some();
        Ok(build_sync_status(
            SyncStatusContext {
                provider_id: config.provider_id,
                vault_id: config.vault_id,
                preset: runtime.preset,
                paused: repository.is_paused()?,
                unlocked,
                syncing: self.syncing.load(Ordering::SeqCst),
                pending_operation_count: repository.pending_operation_count()?,
                conflict_count: repository.unresolved_conflict_count()?,
            },
            &config.retry,
        ))
    }

    pub async fn test_provider(
        &self,
        provider_input: SyncProviderInput,
    ) -> Result<sona_core::sync::SyncProviderDescriptor, SyncApplicationError> {
        self.providers.test_provider(provider_input).await
    }

    pub async fn create(
        &self,
        provider_input: SyncProviderInput,
        preset: SyncPresetV1,
        master_password: &str,
        create_recovery_key: bool,
    ) -> Result<SyncCreateResult, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        self.ensure_unconfigured()?;
        let provider = self.providers.prepare(provider_input).await?;
        provider.store.probe().await?;
        let vault_id = self.environment.next_id();
        let device_id = self.environment.next_id();
        let created = create_remote_vault(
            provider.store.as_ref(),
            &vault_id,
            preset,
            master_password,
            create_recovery_key,
        )
        .await?;
        self.repository_factory
            .initialize(&vault_id, &device_id, preset)?;
        self.config_store.save(&SyncApplicationConfig {
            provider_id: provider.descriptor.id.clone(),
            vault_id: vault_id.clone(),
            device_id: device_id.clone(),
            preset,
            provider_configuration: provider.persisted_configuration.clone(),
            retry: SyncRetryState::default(),
        })?;
        let _ = self
            .persist_session_secrets(
                provider.descriptor.id.as_str(),
                &vault_id,
                &provider.credential,
                created.opened.vault_key.as_slice(),
            )
            .await;
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession {
            provider,
            opened: created.opened,
        });
        let _ = self.run_with_lifecycle_held().await;
        Ok(SyncCreateResult {
            vault_id,
            device_id,
            recovery_key: created.recovery_key,
            status: self.status().await?,
        })
    }

    pub async fn preview_join(
        &self,
        provider_input: SyncProviderInput,
        vault_id: &str,
        master_password: &str,
    ) -> Result<sona_core::sync::SyncJoinPreview, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        self.ensure_unconfigured()?;
        let provider = self.providers.prepare(provider_input).await?;
        let opened =
            open_remote_vault_with_password(provider.store.as_ref(), vault_id, master_password)
                .await?;
        let remote_segments = load_remote_state_for_join(
            provider.store.as_ref(),
            vault_id,
            opened.vault_key.as_slice(),
        )
        .await?;
        self.repository_factory
            .preview(
                vault_id,
                &self.environment.next_id(),
                opened.header.preset,
                &remote_segments,
            )
            .map_err(Into::into)
    }

    pub async fn join(
        &self,
        provider_input: SyncProviderInput,
        vault_id: &str,
        master_password: &str,
    ) -> Result<SyncRunResult, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        self.ensure_unconfigured()?;
        let provider = self.providers.prepare(provider_input).await?;
        let opened =
            open_remote_vault_with_password(provider.store.as_ref(), vault_id, master_password)
                .await?;
        let device_id = self.environment.next_id();
        self.repository_factory
            .initialize(vault_id, &device_id, opened.header.preset)?;
        self.config_store.save(&SyncApplicationConfig {
            provider_id: provider.descriptor.id.clone(),
            vault_id: vault_id.to_string(),
            device_id,
            preset: opened.header.preset,
            provider_configuration: provider.persisted_configuration.clone(),
            retry: SyncRetryState::default(),
        })?;
        let _ = self
            .persist_session_secrets(
                provider.descriptor.id.as_str(),
                vault_id,
                &provider.credential,
                opened.vault_key.as_slice(),
            )
            .await;
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession { provider, opened });
        self.run_with_lifecycle_held().await
    }

    pub async fn unlock_with_password(
        &self,
        provider_credential: Vec<u8>,
        master_password: &str,
    ) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let config = self.config_store.load()?.ok_or_else(not_configured)?;
        let provider = self
            .providers
            .restore(
                &config.provider_id,
                config.provider_configuration.clone(),
                provider_credential,
            )
            .await?;
        let opened = open_remote_vault_with_password(
            provider.store.as_ref(),
            &config.vault_id,
            master_password,
        )
        .await?;
        self.finish_unlock(&config, provider, opened).await;
        self.status().await
    }

    pub async fn unlock_with_recovery_key(
        &self,
        provider_credential: Vec<u8>,
        recovery_key: &str,
    ) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let config = self.config_store.load()?.ok_or_else(not_configured)?;
        let provider = self
            .providers
            .restore(
                &config.provider_id,
                config.provider_configuration.clone(),
                provider_credential,
            )
            .await?;
        let opened = open_remote_vault_with_recovery_key(
            provider.store.as_ref(),
            &config.vault_id,
            recovery_key,
        )
        .await?;
        self.finish_unlock(&config, provider, opened).await;
        self.status().await
    }

    pub async fn lock(&self) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        self.manually_locked.store(true, Ordering::SeqCst);
        *self.session.lock().await = None;
        self.status().await
    }

    pub async fn set_paused(
        &self,
        paused: bool,
    ) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        if self.config_store.load()?.is_none() {
            return Err(not_configured());
        }
        self.repository()?
            .ok_or_else(local_state_missing)?
            .set_paused(paused)?;
        self.status().await
    }

    pub async fn disconnect(&self) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        if let Some(config) = self.config_store.load()? {
            self.delete_session_secrets(&config).await?;
        }
        if let Some(repository) = self.repository()? {
            repository.disconnect()?;
        }
        let mut session = self.session.lock().await;
        *session = None;
        self.manually_locked.store(false, Ordering::SeqCst);
        self.config_store.delete()?;
        Ok(disabled_sync_status())
    }

    pub async fn run(&self) -> Result<SyncRunResult, SyncApplicationError> {
        let _lifecycle = self.lifecycle.try_lock().map_err(|_| {
            if self.syncing.load(Ordering::SeqCst) {
                SyncApplicationError::InvalidState("A sync run is already in progress.".to_string())
            } else {
                SyncApplicationError::InvalidState(
                    "Another sync lifecycle operation is in progress.".to_string(),
                )
            }
        })?;
        self.run_with_lifecycle_held().await
    }

    async fn run_with_lifecycle_held(&self) -> Result<SyncRunResult, SyncApplicationError> {
        let mut config = self.config_store.load()?.ok_or_else(not_configured)?;
        let repository = self.repository()?.ok_or_else(local_state_missing)?;
        if repository.is_paused()? {
            return Err(SyncApplicationError::InvalidState(
                "Sync is paused.".to_string(),
            ));
        }
        if !self.manually_locked.load(Ordering::SeqCst) {
            let _ = self.restore_session(&config).await;
        }
        self.syncing
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| {
                SyncApplicationError::InvalidState("A sync run is already in progress.".to_string())
            })?;
        let _guard = SyncRunGuard(&self.syncing);
        let (store, vault_key) = {
            let session = self.session.lock().await;
            let session = session.as_ref().ok_or_else(locked)?;
            (
                Arc::clone(&session.provider.store),
                session.opened.vault_key.as_slice().to_vec(),
            )
        };
        let result = run_sync_cycle(
            repository.runtime_repository(),
            store.as_ref(),
            &vault_key,
            &mut config.retry,
            self.environment.now_ms(),
            self.environment.jitter(),
        )
        .await;
        self.config_store.save(&config)?;
        result.map_err(Into::into)
    }

    pub async fn change_master_password(
        &self,
        current_master_password: &str,
        next_master_password: &str,
    ) -> Result<(), SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let mut session = self.session.lock().await;
        let session = session.as_mut().ok_or_else(locked)?;
        let UnlockedSession { provider, opened } = session;
        change_remote_master_password(
            provider.store.as_ref(),
            opened,
            current_master_password,
            next_master_password,
        )
        .await
        .map_err(Into::into)
    }

    pub async fn generate_recovery_key(&self) -> Result<String, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let mut session = self.session.lock().await;
        let session = session.as_mut().ok_or_else(locked)?;
        let UnlockedSession { provider, opened } = session;
        regenerate_remote_recovery_key(provider.store.as_ref(), opened)
            .await
            .map_err(Into::into)
    }

    pub async fn change_preset(
        &self,
        preset: SyncPresetV1,
        confirm_shrink: bool,
    ) -> Result<SyncStatusSnapshot, SyncApplicationError> {
        let _lifecycle = self.lifecycle.lock().await;
        let repository = self.repository()?.ok_or_else(not_configured)?;
        {
            let mut session = self.session.lock().await;
            let session = session.as_mut().ok_or_else(locked)?;
            let UnlockedSession { provider, opened } = session;
            change_sync_preset(
                repository.runtime_repository(),
                provider.store.as_ref(),
                opened,
                preset,
                confirm_shrink,
            )
            .await?;
        }
        let mut config = self.config_store.load()?.ok_or_else(not_configured)?;
        config.preset = preset;
        self.config_store.save(&config)?;
        self.status().await
    }

    pub fn list_conflicts(&self) -> Result<Vec<SyncConflictSummary>, SyncApplicationError> {
        self.repository()?
            .ok_or_else(not_configured)?
            .list_conflict_summaries()
            .map_err(Into::into)
    }

    pub fn get_conflict(
        &self,
        conflict_id: &str,
    ) -> Result<Option<SyncConflictDetail>, SyncApplicationError> {
        self.repository()?
            .ok_or_else(not_configured)?
            .get_conflict_detail(conflict_id)
            .map_err(Into::into)
    }

    pub fn resolve_conflict(
        &self,
        conflict_id: &str,
        resolution: SyncConflictResolution,
    ) -> Result<(), SyncApplicationError> {
        self.repository()?
            .ok_or_else(not_configured)?
            .resolve_conflict(conflict_id, resolution, self.environment.now_ms())
            .map_err(Into::into)
    }

    fn ensure_unconfigured(&self) -> Result<(), SyncApplicationError> {
        if self.config_store.load()?.is_some() || self.repository()?.is_some() {
            Err(SyncApplicationError::InvalidState(
                "This Sona data directory is already connected to a sync vault.".to_string(),
            ))
        } else {
            Ok(())
        }
    }

    fn repository(
        &self,
    ) -> Result<Option<Arc<dyn sona_core::sync::SyncApplicationRepository>>, SyncApplicationError>
    {
        self.repository_factory.open().map_err(Into::into)
    }

    async fn restore_session(
        &self,
        config: &SyncApplicationConfig,
    ) -> Result<(), SyncApplicationError> {
        if self.manually_locked.load(Ordering::SeqCst) || self.session.lock().await.is_some() {
            return Ok(());
        }
        let provider_key = self
            .providers
            .credential_secret_key(&config.provider_id, &config.vault_id)?;
        let Some(credential) = self.secret_store.read_secret(&provider_key).await? else {
            return Ok(());
        };
        let Some(vault_key) = self
            .secret_store
            .read_secret(&vault_key_secret_key(&config.vault_id))
            .await?
        else {
            return Ok(());
        };
        let provider = self
            .providers
            .restore(
                &config.provider_id,
                config.provider_configuration.clone(),
                credential,
            )
            .await?;
        let opened =
            open_remote_vault_with_vault_key(provider.store.as_ref(), &config.vault_id, &vault_key)
                .await?;
        if !self.manually_locked.load(Ordering::SeqCst) {
            let mut session = self.session.lock().await;
            let current_config = self.config_store.load()?;
            if session.is_none() && current_config.as_ref() == Some(config) {
                *session = Some(UnlockedSession { provider, opened });
            }
        }
        Ok(())
    }

    async fn persist_session_secrets(
        &self,
        provider_id: &str,
        vault_id: &str,
        credential: &[u8],
        vault_key: &[u8],
    ) -> Result<(), SyncApplicationError> {
        let provider_key = self
            .providers
            .credential_secret_key(provider_id, vault_id)?;
        self.secret_store
            .write_secret(&provider_key, credential)
            .await?;
        if let Err(error) = self
            .secret_store
            .write_secret(&vault_key_secret_key(vault_id), vault_key)
            .await
        {
            let _ = self.secret_store.delete_secret(&provider_key).await;
            return Err(error.into());
        }
        Ok(())
    }

    async fn finish_unlock(
        &self,
        config: &SyncApplicationConfig,
        provider: SyncProvider,
        opened: OpenedRemoteVault,
    ) {
        let _ = self
            .persist_session_secrets(
                &config.provider_id,
                &config.vault_id,
                &provider.credential,
                opened.vault_key.as_slice(),
            )
            .await;
        self.manually_locked.store(false, Ordering::SeqCst);
        *self.session.lock().await = Some(UnlockedSession { provider, opened });
    }

    async fn delete_session_secrets(
        &self,
        config: &SyncApplicationConfig,
    ) -> Result<(), SyncApplicationError> {
        let provider_key = self
            .providers
            .credential_secret_key(&config.provider_id, &config.vault_id)?;
        self.secret_store.delete_secret(&provider_key).await?;
        self.secret_store
            .delete_secret(&vault_key_secret_key(&config.vault_id))
            .await?;
        Ok(())
    }
}

fn vault_key_secret_key(vault_id: &str) -> String {
    format!("vault-key:{vault_id}")
}

fn not_configured() -> SyncApplicationError {
    SyncApplicationError::InvalidState("Sync is not configured.".to_string())
}

fn local_state_missing() -> SyncApplicationError {
    SyncApplicationError::InvalidState("Local sync state is missing.".to_string())
}

fn locked() -> SyncApplicationError {
    SyncApplicationError::InvalidState("Sync vault is locked.".to_string())
}

#[derive(Clone, Debug, thiserror::Error, PartialEq, Eq)]
pub enum SyncPresetChangeError {
    #[error(transparent)]
    Sync(#[from] SyncError),
    #[error("Local preset update failed ({local}); remote rollback also failed ({rollback}).")]
    LocalUpdateAndRollback {
        local: SyncError,
        rollback: SyncError,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(default, rename_all = "camelCase")]
pub struct SyncRetryState {
    pub last_success_at_ms: Option<u64>,
    pub consecutive_failures: u32,
    pub next_retry_at_ms: Option<u64>,
    pub last_error: Option<SyncErrorSnapshot>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SyncApplicationConfig {
    pub provider_id: String,
    pub vault_id: String,
    pub device_id: String,
    pub preset: SyncPresetV1,
    pub provider_configuration: Value,
    pub retry: SyncRetryState,
}

pub trait SyncConfigStore: Send + Sync {
    fn load(&self) -> Result<Option<SyncApplicationConfig>, SyncApplicationError>;
    fn save(&self, config: &SyncApplicationConfig) -> Result<(), SyncApplicationError>;
    fn delete(&self) -> Result<(), SyncApplicationError>;
}

#[derive(Clone, Debug)]
pub struct JsonFileSyncConfigStore {
    path: PathBuf,
}

impl JsonFileSyncConfigStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl SyncConfigStore for JsonFileSyncConfigStore {
    fn load(&self) -> Result<Option<SyncApplicationConfig>, SyncApplicationError> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(config_error(error)),
        };
        let value: Value = serde_json::from_slice(&bytes).map_err(config_error)?;
        let object = value.as_object().ok_or_else(|| {
            SyncApplicationError::Config("Sync configuration must be a JSON object.".to_string())
        })?;
        let provider_id = required_config_string(object, "providerId")?;
        let provider_configuration = object.get(&provider_id).cloned().ok_or_else(|| {
            SyncApplicationError::Config(format!(
                "Sync configuration is missing provider settings for {provider_id}."
            ))
        })?;
        Ok(Some(SyncApplicationConfig {
            provider_id,
            vault_id: required_config_string(object, "vaultId")?,
            device_id: required_config_string(object, "deviceId")?,
            preset: serde_json::from_value(object.get("preset").cloned().ok_or_else(|| {
                SyncApplicationError::Config("Sync configuration is missing preset.".to_string())
            })?)
            .map_err(config_error)?,
            provider_configuration,
            retry: serde_json::from_value(value).map_err(config_error)?,
        }))
    }

    fn save(&self, config: &SyncApplicationConfig) -> Result<(), SyncApplicationError> {
        let mut object = Map::new();
        object.insert(
            "providerId".to_string(),
            Value::String(config.provider_id.clone()),
        );
        object.insert(
            "vaultId".to_string(),
            Value::String(config.vault_id.clone()),
        );
        object.insert(
            "deviceId".to_string(),
            Value::String(config.device_id.clone()),
        );
        object.insert(
            "preset".to_string(),
            serde_json::to_value(config.preset).map_err(config_error)?,
        );
        object.insert(
            config.provider_id.clone(),
            config.provider_configuration.clone(),
        );
        let retry = serde_json::to_value(&config.retry).map_err(config_error)?;
        object.extend(retry.as_object().cloned().ok_or_else(|| {
            SyncApplicationError::Config("Sync retry state must be a JSON object.".to_string())
        })?);
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(config_error)?;
        }
        let bytes = serde_json::to_vec_pretty(&Value::Object(object)).map_err(config_error)?;
        std::fs::write(&self.path, bytes).map_err(config_error)
    }

    fn delete(&self) -> Result<(), SyncApplicationError> {
        match std::fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(config_error(error)),
        }
    }
}

fn required_config_string(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, SyncApplicationError> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            SyncApplicationError::Config(format!(
                "Sync configuration field {field} must be a non-empty string."
            ))
        })
}

fn config_error(error: impl ToString) -> SyncApplicationError {
    SyncApplicationError::Config(error.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SyncStatusContext {
    pub provider_id: String,
    pub vault_id: String,
    pub preset: SyncPresetV1,
    pub paused: bool,
    pub unlocked: bool,
    pub syncing: bool,
    pub pending_operation_count: u64,
    pub conflict_count: u64,
}

pub fn disabled_sync_status() -> SyncStatusSnapshot {
    SyncStatusSnapshot {
        state: SyncLifecycleState::Disabled,
        provider_id: None,
        vault_id: None,
        preset: None,
        last_success_at_ms: None,
        pending_operation_count: 0,
        conflict_count: 0,
        next_retry_at_ms: None,
        last_error: None,
    }
}

pub fn build_sync_status(context: SyncStatusContext, retry: &SyncRetryState) -> SyncStatusSnapshot {
    let state = if context.syncing {
        SyncLifecycleState::Syncing
    } else if context.paused {
        SyncLifecycleState::Paused
    } else if !context.unlocked {
        SyncLifecycleState::Locked
    } else if retry.last_error.is_some() {
        SyncLifecycleState::Error
    } else {
        SyncLifecycleState::Idle
    };

    SyncStatusSnapshot {
        state,
        provider_id: Some(context.provider_id),
        vault_id: Some(context.vault_id),
        preset: Some(context.preset),
        last_success_at_ms: retry.last_success_at_ms,
        pending_operation_count: context.pending_operation_count,
        conflict_count: context.conflict_count,
        next_retry_at_ms: retry.next_retry_at_ms,
        last_error: retry.last_error.clone(),
    }
}

pub fn sync_error_code(error: &SyncError) -> &'static str {
    match error {
        SyncError::InvalidOperation(_) => "invalid_operation",
        SyncError::InvalidObjectKey(_) => "invalid_object_key",
        SyncError::ObjectStore(_) => "provider_error",
        SyncError::LocalRepository(_) => "local_repository_error",
        SyncError::SecretStore(_) => "secret_store_error",
        SyncError::Protocol(_) => "protocol_error",
        SyncError::Crypto(_) => "crypto_error",
    }
}

pub fn is_retryable_sync_error(error: &SyncError) -> bool {
    matches!(error, SyncError::ObjectStore(_))
}

pub fn apply_sync_run_result(
    retry: &mut SyncRetryState,
    now_ms: u64,
    jitter: u32,
    result: &Result<SyncRunResult, SyncError>,
) {
    match result {
        Ok(_) => {
            retry.last_success_at_ms = Some(now_ms);
            retry.consecutive_failures = 0;
            retry.next_retry_at_ms = None;
            retry.last_error = None;
        }
        Err(error) => {
            retry.consecutive_failures = retry.consecutive_failures.saturating_add(1);
            retry.next_retry_at_ms = Some(SyncBackoffPolicy::default().next_retry_at_ms(
                now_ms,
                retry.consecutive_failures,
                jitter,
            ));
            retry.last_error = Some(SyncErrorSnapshot {
                code: sync_error_code(error).to_string(),
                message: error.to_string(),
                retryable: is_retryable_sync_error(error),
            });
        }
    }
}

pub async fn run_sync_cycle(
    local: &dyn SyncLocalRepository,
    remote: &dyn SyncObjectStore,
    vault_key: &[u8],
    retry: &mut SyncRetryState,
    now_ms: u64,
    jitter: u32,
) -> Result<SyncRunResult, SyncError> {
    let result = SyncRuntime::new(local, remote, vault_key)
        .run_at(now_ms)
        .await;
    apply_sync_run_result(retry, now_ms, jitter, &result);
    result
}

pub async fn change_sync_preset(
    local: &dyn SyncLocalRepository,
    remote: &dyn SyncObjectStore,
    opened: &mut OpenedRemoteVault,
    preset: SyncPresetV1,
    confirm_shrink: bool,
) -> Result<(), SyncPresetChangeError> {
    local.validate_preset_change(preset, confirm_shrink)?;
    let previous = opened.header.preset;
    update_remote_vault_preset(remote, opened, preset).await?;

    if let Err(local_error) = local.change_preset(preset, confirm_shrink) {
        return match update_remote_vault_preset(remote, opened, previous).await {
            Ok(()) => Err(local_error.into()),
            Err(rollback_error) => Err(SyncPresetChangeError::LocalUpdateAndRollback {
                local: local_error,
                rollback: rollback_error,
            }),
        };
    }

    Ok(())
}
