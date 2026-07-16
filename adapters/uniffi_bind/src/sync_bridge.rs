use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use serde::{Deserialize, Serialize};
use sona_core::sync::{
    SyncConflictResolution, SyncErrorSnapshot, SyncLifecycleState, SyncLocalRepository,
    SyncObjectStore, SyncPresetV1, SyncProviderDescriptor, SyncStatusSnapshot,
};
use sona_sqlite::{Database, SqliteSyncRepository};
use sona_sync::{
    OpenedRemoteVault, SyncBackoffPolicy, SyncRuntime, change_remote_master_password,
    create_remote_vault, load_remote_state_for_join, open_remote_vault_with_password,
    open_remote_vault_with_recovery_key, regenerate_remote_recovery_key,
    update_remote_vault_preset,
};
use sona_sync_webdav::{WebDavObjectStore, WebDavObjectStoreConfig};
use tokio::sync::Mutex;

use crate::json_bridge::{parse_core_json, serialize_core_json};
use crate::{SonaCoreBindingError, SonaCoreBindingResult};

const CONFIG_FILE: &str = "sync.json";

struct Session {
    store: WebDavObjectStore,
    opened: OpenedRemoteVault,
}

type SharedSession = Arc<Mutex<Session>>;

fn sessions() -> &'static Mutex<HashMap<String, SharedSession>> {
    static SESSIONS: OnceLock<Mutex<HashMap<String, SharedSession>>> = OnceLock::new();
    SESSIONS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateRequest {
    provider: WebDavObjectStoreConfig,
    preset: SyncPresetV1,
    master_password: String,
    create_recovery_key: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct JoinRequest {
    provider: WebDavObjectStoreConfig,
    vault_id: String,
    master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct UnlockRequest {
    provider_password: String,
    master_password: Option<String>,
    recovery_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ChangePasswordRequest {
    current_master_password: String,
    next_master_password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProviderConfig {
    server_url: String,
    remote_root: String,
    username: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedConfig {
    provider_id: String,
    vault_id: String,
    device_id: String,
    preset: SyncPresetV1,
    webdav: ProviderConfig,
    last_success_at_ms: Option<u64>,
    #[serde(default)]
    consecutive_failures: u32,
    #[serde(default)]
    next_retry_at_ms: Option<u64>,
    last_error: Option<SyncErrorSnapshot>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CreateResult {
    vault_id: String,
    device_id: String,
    recovery_key: Option<String>,
    status: SyncStatusSnapshot,
}

pub(crate) async fn test_provider_json(config_json: String) -> SonaCoreBindingResult<String> {
    let config: WebDavObjectStoreConfig = parse_core_json(&config_json, "sync provider")?;
    let store = WebDavObjectStore::new(config).map_err(sync_error)?;
    let capabilities = store.probe().await.map_err(sync_error)?;
    if !capabilities.conditional_create || !capabilities.compare_and_swap || !capabilities.delete {
        return Err(sync_binding_error(
            "WebDAV server lacks required conditional object operations.",
        ));
    }
    serialize_core_json(
        &SyncProviderDescriptor {
            id: "webdav".to_string(),
            display_name: "WebDAV".to_string(),
        },
        "sync provider descriptor",
    )
}

pub(crate) async fn get_status_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let status = status(&app_data_dir).await?;
    serialize_core_json(&status, "sync status")
}

pub(crate) async fn create_vault_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    ensure_unconfigured(&app_data_dir)?;
    let request: CreateRequest = parse_core_json(&request_json, "sync create request")?;
    let store = WebDavObjectStore::new(request.provider.clone()).map_err(sync_error)?;
    store.probe().await.map_err(sync_error)?;
    let vault_id = uuid::Uuid::new_v4().to_string();
    let device_id = uuid::Uuid::new_v4().to_string();
    let created = create_remote_vault(
        &store,
        &vault_id,
        request.preset,
        &request.master_password,
        request.create_recovery_key,
    )
    .await
    .map_err(sync_error)?;
    SqliteSyncRepository::initialize(
        database(&app_data_dir)?,
        &vault_id,
        &device_id,
        request.preset,
    )
    .map_err(sync_error)?;
    save_config(
        &app_data_dir,
        &persisted_config(
            &request.provider,
            vault_id.clone(),
            device_id.clone(),
            request.preset,
        ),
    )?;
    sessions().lock().await.insert(
        app_data_dir.clone(),
        Arc::new(Mutex::new(Session {
            store,
            opened: created.opened,
        })),
    );
    let _ = run_now(&app_data_dir).await;
    serialize_core_json(
        &CreateResult {
            vault_id,
            device_id,
            recovery_key: created.recovery_key,
            status: status(&app_data_dir).await?,
        },
        "sync create result",
    )
}

pub(crate) async fn preview_join_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    ensure_unconfigured(&app_data_dir)?;
    let request: JoinRequest = parse_core_json(&request_json, "sync join preview request")?;
    let store = WebDavObjectStore::new(request.provider).map_err(sync_error)?;
    let opened =
        open_remote_vault_with_password(&store, &request.vault_id, &request.master_password)
            .await
            .map_err(sync_error)?;
    let remote_segments =
        load_remote_state_for_join(&store, &request.vault_id, opened.vault_key.as_slice())
            .await
            .map_err(sync_error)?;
    let preview = SqliteSyncRepository::preview_join(
        database(&app_data_dir)?,
        &request.vault_id,
        &format!("preview-{}", uuid::Uuid::new_v4()),
        opened.header.preset,
        &remote_segments,
    )
    .map_err(sync_error)?;
    serialize_core_json(&preview, "sync join preview")
}

pub(crate) async fn join_vault_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<String> {
    ensure_unconfigured(&app_data_dir)?;
    let request: JoinRequest = parse_core_json(&request_json, "sync join request")?;
    let store = WebDavObjectStore::new(request.provider.clone()).map_err(sync_error)?;
    let opened =
        open_remote_vault_with_password(&store, &request.vault_id, &request.master_password)
            .await
            .map_err(sync_error)?;
    let device_id = uuid::Uuid::new_v4().to_string();
    SqliteSyncRepository::initialize(
        database(&app_data_dir)?,
        &request.vault_id,
        &device_id,
        opened.header.preset,
    )
    .map_err(sync_error)?;
    save_config(
        &app_data_dir,
        &persisted_config(
            &request.provider,
            request.vault_id,
            device_id,
            opened.header.preset,
        ),
    )?;
    sessions().lock().await.insert(
        app_data_dir.clone(),
        Arc::new(Mutex::new(Session { store, opened })),
    );
    run_now_json(app_data_dir).await
}

pub(crate) async fn unlock_json(
    app_data_dir: String,
    request_json: String,
    recovery: bool,
) -> SonaCoreBindingResult<String> {
    let request: UnlockRequest = parse_core_json(&request_json, "sync unlock request")?;
    let config =
        load_config(&app_data_dir)?.ok_or_else(|| sync_binding_error("Sync is not configured."))?;
    let provider = runtime_provider(&config, request.provider_password)?;
    let store = WebDavObjectStore::new(provider).map_err(sync_error)?;
    let opened = if recovery {
        open_remote_vault_with_recovery_key(
            &store,
            &config.vault_id,
            request
                .recovery_key
                .as_deref()
                .ok_or_else(|| sync_binding_error("Recovery key is required."))?,
        )
        .await
    } else {
        open_remote_vault_with_password(
            &store,
            &config.vault_id,
            request
                .master_password
                .as_deref()
                .ok_or_else(|| sync_binding_error("Master password is required."))?,
        )
        .await
    }
    .map_err(sync_error)?;
    sessions().lock().await.insert(
        app_data_dir.clone(),
        Arc::new(Mutex::new(Session { store, opened })),
    );
    get_status_json(app_data_dir).await
}

pub(crate) async fn lock(app_data_dir: String) -> SonaCoreBindingResult<()> {
    sessions().lock().await.remove(&app_data_dir);
    Ok(())
}

pub(crate) async fn set_paused_json(
    app_data_dir: String,
    paused: bool,
) -> SonaCoreBindingResult<String> {
    repository(&app_data_dir)?
        .ok_or_else(|| sync_binding_error("Sync is not configured."))?
        .set_paused(paused)
        .map_err(sync_error)?;
    get_status_json(app_data_dir).await
}

pub(crate) async fn disconnect_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    if let Some(repository) = repository(&app_data_dir)? {
        repository.disconnect().map_err(sync_error)?;
    }
    sessions().lock().await.remove(&app_data_dir);
    let path = config_path(&app_data_dir);
    match std::fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(sync_error(error)),
    }
    get_status_json(app_data_dir).await
}

pub(crate) async fn run_now_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let result = run_now(&app_data_dir).await?;
    serialize_core_json(&result, "sync run result")
}

async fn run_now(app_data_dir: &str) -> SonaCoreBindingResult<sona_core::sync::SyncRunResult> {
    let repository =
        repository(app_data_dir)?.ok_or_else(|| sync_binding_error("Sync is not configured."))?;
    if repository.is_paused().map_err(sync_error)? {
        return Err(sync_binding_error("Sync is paused."));
    }
    let session = sessions()
        .lock()
        .await
        .get(app_data_dir)
        .cloned()
        .ok_or_else(|| sync_binding_error("Sync vault is locked."))?;
    let session = session.lock().await;
    let now_ms = now_ms();
    let result = SyncRuntime::new(
        &repository,
        &session.store,
        session.opened.vault_key.as_slice(),
    )
    .run_at(now_ms)
    .await;
    let mut config =
        load_config(app_data_dir)?.ok_or_else(|| sync_binding_error("Sync is not configured."))?;
    match result {
        Ok(result) => {
            config.last_success_at_ms = Some(now_ms);
            config.consecutive_failures = 0;
            config.next_retry_at_ms = None;
            config.last_error = None;
            save_config(app_data_dir, &config)?;
            Ok(result)
        }
        Err(error) => {
            config.consecutive_failures = config.consecutive_failures.saturating_add(1);
            let jitter = (uuid::Uuid::new_v4().as_u128() % 1_000_001) as u32;
            config.next_retry_at_ms = Some(SyncBackoffPolicy::default().next_retry_at_ms(
                now_ms,
                config.consecutive_failures,
                jitter,
            ));
            config.last_error = Some(SyncErrorSnapshot {
                code: "sync_error".to_string(),
                message: error.to_string(),
                retryable: matches!(error, sona_core::sync::SyncError::ObjectStore(_)),
            });
            save_config(app_data_dir, &config)?;
            Err(sync_error(error))
        }
    }
}

pub(crate) async fn change_preset_json(
    app_data_dir: String,
    preset_json: String,
    confirm_shrink: bool,
) -> SonaCoreBindingResult<String> {
    let preset: SyncPresetV1 = parse_core_json(&preset_json, "sync preset")?;
    let repository =
        repository(&app_data_dir)?.ok_or_else(|| sync_binding_error("Sync is not configured."))?;
    repository
        .validate_preset_change(preset, confirm_shrink)
        .map_err(sync_error)?;
    let session = sessions()
        .lock()
        .await
        .get(&app_data_dir)
        .cloned()
        .ok_or_else(|| sync_binding_error("Sync vault is locked."))?;
    let mut session = session.lock().await;
    let store = session.store.clone();
    let previous = session.opened.header.preset;
    update_remote_vault_preset(&store, &mut session.opened, preset)
        .await
        .map_err(sync_error)?;
    if let Err(error) = repository.change_preset(preset, confirm_shrink) {
        let rollback = update_remote_vault_preset(&store, &mut session.opened, previous).await;
        return match rollback {
            Ok(()) => Err(sync_error(error)),
            Err(rollback_error) => Err(sync_binding_error(format!(
                "Local preset update failed ({error}); remote rollback also failed ({rollback_error})."
            ))),
        };
    }
    let mut config =
        load_config(&app_data_dir)?.ok_or_else(|| sync_binding_error("Sync is not configured."))?;
    config.preset = preset;
    save_config(&app_data_dir, &config)?;
    drop(session);
    get_status_json(app_data_dir).await
}

pub(crate) async fn change_master_password_json(
    app_data_dir: String,
    request_json: String,
) -> SonaCoreBindingResult<()> {
    let request: ChangePasswordRequest =
        parse_core_json(&request_json, "sync password change request")?;
    let session = sessions()
        .lock()
        .await
        .get(&app_data_dir)
        .cloned()
        .ok_or_else(|| sync_binding_error("Sync vault is locked."))?;
    let mut session = session.lock().await;
    let store = session.store.clone();
    change_remote_master_password(
        &store,
        &mut session.opened,
        &request.current_master_password,
        &request.next_master_password,
    )
    .await
    .map_err(sync_error)
}

pub(crate) async fn generate_recovery_key(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let session = sessions()
        .lock()
        .await
        .get(&app_data_dir)
        .cloned()
        .ok_or_else(|| sync_binding_error("Sync vault is locked."))?;
    let mut session = session.lock().await;
    let store = session.store.clone();
    regenerate_remote_recovery_key(&store, &mut session.opened)
        .await
        .map_err(sync_error)
}

pub(crate) fn list_conflicts_json(app_data_dir: String) -> SonaCoreBindingResult<String> {
    let conflicts = repository(&app_data_dir)?
        .ok_or_else(|| sync_binding_error("Sync is not configured."))?
        .list_conflict_summaries()
        .map_err(sync_error)?;
    serialize_core_json(&conflicts, "sync conflicts")
}

pub(crate) fn get_conflict_json(
    app_data_dir: String,
    conflict_id: String,
) -> SonaCoreBindingResult<String> {
    let conflict = repository(&app_data_dir)?
        .ok_or_else(|| sync_binding_error("Sync is not configured."))?
        .get_conflict_detail(&conflict_id)
        .map_err(sync_error)?;
    serialize_core_json(&conflict, "sync conflict")
}

pub(crate) fn resolve_conflict_json(
    app_data_dir: String,
    conflict_id: String,
    resolution_json: String,
) -> SonaCoreBindingResult<()> {
    let resolution: SyncConflictResolution =
        parse_core_json(&resolution_json, "sync conflict resolution")?;
    repository(&app_data_dir)?
        .ok_or_else(|| sync_binding_error("Sync is not configured."))?
        .resolve_conflict(&conflict_id, resolution, now_ms())
        .map_err(sync_error)
}

async fn status(app_data_dir: &str) -> SonaCoreBindingResult<SyncStatusSnapshot> {
    let Some(config) = load_config(app_data_dir)? else {
        return Ok(disabled_status());
    };
    let repository = repository(app_data_dir)?
        .ok_or_else(|| sync_binding_error("SQLite sync state is missing."))?;
    let runtime = repository.load_runtime_state().map_err(sync_error)?;
    let unlocked = sessions().lock().await.contains_key(app_data_dir);
    Ok(SyncStatusSnapshot {
        state: if repository.is_paused().map_err(sync_error)? {
            SyncLifecycleState::Paused
        } else if config.last_error.is_some() && unlocked {
            SyncLifecycleState::Error
        } else if unlocked {
            SyncLifecycleState::Idle
        } else {
            SyncLifecycleState::Locked
        },
        provider_id: Some(config.provider_id),
        vault_id: Some(config.vault_id),
        preset: Some(runtime.preset),
        last_success_at_ms: config.last_success_at_ms,
        pending_operation_count: repository.pending_operation_count().map_err(sync_error)?,
        conflict_count: repository.unresolved_conflict_count().map_err(sync_error)?,
        next_retry_at_ms: config.next_retry_at_ms,
        last_error: config.last_error,
    })
}

fn ensure_unconfigured(app_data_dir: &str) -> SonaCoreBindingResult<()> {
    if load_config(app_data_dir)?.is_some() || repository(app_data_dir)?.is_some() {
        Err(sync_binding_error(
            "This Sona data directory is already connected to a sync vault.",
        ))
    } else {
        Ok(())
    }
}

fn database(app_data_dir: &str) -> SonaCoreBindingResult<Arc<Database>> {
    std::fs::create_dir_all(app_data_dir).map_err(sync_error)?;
    Database::open(&PathBuf::from(app_data_dir))
        .map(Arc::new)
        .map_err(sync_error)
}

fn repository(app_data_dir: &str) -> SonaCoreBindingResult<Option<SqliteSyncRepository>> {
    SqliteSyncRepository::open_existing(database(app_data_dir)?).map_err(sync_error)
}

fn persisted_config(
    provider: &WebDavObjectStoreConfig,
    vault_id: String,
    device_id: String,
    preset: SyncPresetV1,
) -> PersistedConfig {
    PersistedConfig {
        provider_id: "webdav".to_string(),
        vault_id,
        device_id,
        preset,
        webdav: ProviderConfig {
            server_url: provider.server_url.clone(),
            remote_root: provider.remote_root.clone(),
            username: provider.username.clone(),
        },
        last_success_at_ms: None,
        consecutive_failures: 0,
        next_retry_at_ms: None,
        last_error: None,
    }
}

fn runtime_provider(
    config: &PersistedConfig,
    password: String,
) -> SonaCoreBindingResult<WebDavObjectStoreConfig> {
    WebDavObjectStoreConfig::new(
        &config.webdav.server_url,
        &config.webdav.remote_root,
        &config.webdav.username,
        password,
    )
    .map_err(sync_error)
}

fn load_config(app_data_dir: &str) -> SonaCoreBindingResult<Option<PersistedConfig>> {
    match std::fs::read(config_path(app_data_dir)) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(sync_error),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(sync_error(error)),
    }
}

fn save_config(app_data_dir: &str, config: &PersistedConfig) -> SonaCoreBindingResult<()> {
    std::fs::create_dir_all(app_data_dir).map_err(sync_error)?;
    std::fs::write(
        config_path(app_data_dir),
        serde_json::to_vec_pretty(config).map_err(sync_error)?,
    )
    .map_err(sync_error)
}

fn config_path(app_data_dir: &str) -> PathBuf {
    Path::new(app_data_dir).join(CONFIG_FILE)
}

fn disabled_status() -> SyncStatusSnapshot {
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

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_millis() as u64)
}

fn sync_error(error: impl ToString) -> SonaCoreBindingError {
    sync_binding_error(error.to_string())
}

fn sync_binding_error(reason: impl Into<String>) -> SonaCoreBindingError {
    SonaCoreBindingError::Sync {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unconfigured_status_uses_the_shared_disabled_contract() {
        let directory = tempfile::tempdir().unwrap();
        let json = get_status_json(directory.path().to_string_lossy().into_owned())
            .await
            .unwrap();
        let status: SyncStatusSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(status.state, SyncLifecycleState::Disabled);
        assert_eq!(status.pending_operation_count, 0);
    }
}
