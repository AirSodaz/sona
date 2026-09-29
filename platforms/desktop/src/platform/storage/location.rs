use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_opener::OpenerExt;

pub use sona_runtime_fs::{
    STORAGE_BOOTSTRAP_FILE_NAME, StorageBootstrapConfig, load_bootstrap_config,
    resolve_active_data_dir, resolve_active_models_dir, save_bootstrap_config,
};

pub const DATA_MIGRATION_FILE_NAMES: [&str; 8] = [
    "sona.db",
    "sona.db-wal",
    "sona.db-shm",
    "sona-analytics.db",
    "sona-analytics.db-wal",
    "sona-analytics.db-shm",
    "sync.json",
    ".history.lock",
];

pub const DATA_MIGRATION_COPY_FILE_NAMES: [&str; 5] = [
    "sona.db",
    "sona.db-wal",
    "sona-analytics.db",
    "sona-analytics.db-wal",
    "sync.json",
];

pub const DATA_MIGRATION_SUBDIRECTORIES: [&str; 4] =
    ["history", "speaker-profiles", "recovery", "api_temp"];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct StorageDirectoriesInfo {
    pub data_dir: String,
    pub default_data_dir: String,
    pub is_custom_data_dir: bool,
    pub models_dir: String,
    pub default_models_dir: String,
    pub is_custom_models_dir: bool,
}

pub fn default_app_local_data_dir_for_app<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    app.path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve default AppLocalData: {e}"))
}

pub fn resolve_active_data_dir_for_app<R: Runtime>(app: &AppHandle<R>) -> Result<PathBuf, String> {
    let default_dir = default_app_local_data_dir_for_app(app)?;
    Ok(resolve_active_data_dir(&default_dir))
}

pub fn resolve_active_models_dir_for_app<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<PathBuf, String> {
    let default_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_dir);
    Ok(resolve_active_models_dir(&default_dir, &active_data_dir))
}

pub fn get_storage_directories_info<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageDirectoriesInfo, String> {
    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);
    let active_models_dir = resolve_active_models_dir(&default_data_dir, &active_data_dir);
    let default_models_dir = default_data_dir.join("models");

    let config = load_bootstrap_config(&default_data_dir);
    let is_custom_data_dir = config.custom_data_dir.is_some()
        && config.custom_data_dir.as_ref() != Some(&default_data_dir);
    let is_custom_models_dir = config.custom_models_dir.is_some()
        && config.custom_models_dir.as_ref() != Some(&default_models_dir);

    Ok(StorageDirectoriesInfo {
        data_dir: active_data_dir.to_string_lossy().into_owned(),
        default_data_dir: default_data_dir.to_string_lossy().into_owned(),
        is_custom_data_dir,
        models_dir: active_models_dir.to_string_lossy().into_owned(),
        default_models_dir: default_models_dir.to_string_lossy().into_owned(),
        is_custom_models_dir,
    })
}

pub fn validate_target_directory(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty() {
        return Err("Directory path cannot be empty".to_string());
    }
    if !path.is_absolute() {
        return Err(format!(
            "Directory path '{}' must be an absolute path",
            path.display()
        ));
    }
    if path
        .components()
        .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(format!(
            "Directory path '{}' cannot contain parent directory traversal (..)",
            path.display()
        ));
    }
    if path.is_file() {
        return Err(format!("'{}' is a file, not a directory", path.display()));
    }
    std::fs::create_dir_all(path)
        .map_err(|e| format!("Failed to create directory '{}': {}", path.display(), e))?;

    let test_file = path.join(".sona_write_test");
    std::fs::write(&test_file, b"test")
        .map_err(|e| format!("Directory '{}' is not writable: {}", path.display(), e))?;
    let _ = std::fs::remove_file(&test_file);
    Ok(())
}

pub fn copy_directory_contents(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_directory_contents(&entry_path, &target_path)?;
        } else if entry_path.is_file() {
            std::fs::copy(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

static MIGRATION_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub struct MigrationGuard;

impl MigrationGuard {
    pub fn acquire() -> Result<Self, String> {
        if MIGRATION_IN_PROGRESS
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err("Another storage migration is already in progress.".to_string());
        }
        Ok(Self)
    }
}

impl Drop for MigrationGuard {
    fn drop(&mut self) {
        MIGRATION_IN_PROGRESS.store(false, Ordering::Release);
    }
}

pub fn is_migration_in_progress() -> bool {
    MIGRATION_IN_PROGRESS.load(Ordering::Acquire)
}

pub async fn check_active_tasks_idle<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if let Some(audio_state) = app.try_state::<crate::integrations::audio::AudioState>()
        && audio_state.has_active_captures()
    {
        return Err(
            "Cannot migrate storage while audio recording or live capture is active. Please stop recording first."
                .to_string(),
        );
    }
    if let Some(asr_state) = app.try_state::<crate::integrations::asr::AsrState>()
        && asr_state.is_busy().await
    {
        return Err(
            "Cannot migrate storage while speech recognition or batch transcription is in progress. Please wait or stop active tasks."
                .to_string(),
        );
    }
    if let Some(download_state) = app.try_state::<crate::platform::model_downloads::DownloadState>()
        && download_state.has_active_downloads().await
    {
        return Err(
            "Cannot migrate storage while model downloads are in progress. Please wait for downloads to finish or cancel them."
                .to_string(),
        );
    }
    Ok(())
}

pub async fn check_storage_can_migrate<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    if is_migration_in_progress() {
        return Err("Another storage migration is already in progress.".to_string());
    }
    check_active_tasks_idle(app).await
}

pub const STORAGE_MIGRATION_PROGRESS_EVENT: &str = "storage-migration-progress";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageMigrationProgress {
    pub phase: String,
    pub current_file: String,
    pub copied_bytes: u64,
    pub total_bytes: u64,
    pub percent: f64,
}

pub struct MigrationProgressEmitter<'a, R: Runtime> {
    app: &'a AppHandle<R>,
    total_bytes: u64,
    copied_bytes: u64,
    last_emit: std::time::Instant,
}

impl<'a, R: Runtime> MigrationProgressEmitter<'a, R> {
    pub fn new(app: &'a AppHandle<R>, total_bytes: u64) -> Self {
        Self {
            app,
            total_bytes,
            copied_bytes: 0,
            last_emit: std::time::Instant::now(),
        }
    }

    pub fn emit(&mut self, phase: &str, current_file: &str) {
        let percent = if self.total_bytes > 0 {
            (self.copied_bytes as f64 / self.total_bytes as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            100.0
        };
        let _ = self.app.emit(
            STORAGE_MIGRATION_PROGRESS_EVENT,
            StorageMigrationProgress {
                phase: phase.to_string(),
                current_file: current_file.to_string(),
                copied_bytes: self.copied_bytes,
                total_bytes: self.total_bytes,
                percent,
            },
        );
        self.last_emit = std::time::Instant::now();
    }

    pub fn maybe_emit(&mut self, phase: &str, current_file: &str) {
        if self.last_emit.elapsed() >= std::time::Duration::from_millis(50) {
            self.emit(phase, current_file);
        }
    }

    pub fn copy_file_streaming(&mut self, src: &Path, dst: &Path) -> Result<(), std::io::Error> {
        use std::io::{Read, Write};
        let src_canonical = src.canonicalize().unwrap_or_else(|_| src.to_path_buf());
        if dst.exists() {
            let dst_canonical = dst.canonicalize().unwrap_or_else(|_| dst.to_path_buf());
            if src_canonical == dst_canonical {
                return Ok(());
            }
        }
        let mut reader = std::fs::File::open(src)?;
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp_dst = dst.with_extension(format!("tmp_{}", uuid::Uuid::new_v4().simple()));
        let mut writer = std::fs::File::create(&tmp_dst)?;
        let file_name = src
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let mut buffer = vec![0u8; 64 * 1024];
        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            writer.write_all(&buffer[..bytes_read])?;
            self.copied_bytes += bytes_read as u64;
            self.maybe_emit("copying", &file_name);
        }
        writer.flush()?;
        drop(writer);
        if let Err(e) = std::fs::rename(&tmp_dst, dst) {
            let _ = std::fs::remove_file(&tmp_dst);
            return Err(e);
        }
        self.emit("copying", &file_name);
        Ok(())
    }
}

pub fn calculate_directory_size(path: &Path, max_depth: usize) -> u64 {
    if max_depth == 0 || !path.exists() {
        return 0;
    }
    let mut total = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            if let Ok(ft) = entry.file_type() {
                if ft.is_symlink() {
                    continue;
                }
                if ft.is_dir() {
                    total += calculate_directory_size(&entry.path(), max_depth - 1);
                } else if ft.is_file()
                    && let Ok(meta) = entry.metadata()
                {
                    total += meta.len();
                }
            }
        }
    }
    total
}

pub fn copy_directory_contents_with_progress<R: Runtime>(
    src: &Path,
    dst: &Path,
    emitter: &mut MigrationProgressEmitter<'_, R>,
    max_depth: usize,
) -> Result<(), std::io::Error> {
    if max_depth == 0 || !src.exists() {
        return Ok(());
    }
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ft = entry.file_type()?;
        if ft.is_symlink() {
            continue;
        }
        let entry_path = entry.path();
        let target_path = dst.join(entry.file_name());
        if ft.is_dir() {
            copy_directory_contents_with_progress(
                &entry_path,
                &target_path,
                emitter,
                max_depth - 1,
            )?;
        } else if ft.is_file() {
            emitter.copy_file_streaming(&entry_path, &target_path)?;
        }
    }
    Ok(())
}

pub fn check_path_overlap(src: &Path, dst: &Path) -> Result<(), String> {
    let src_canonical = src.canonicalize().unwrap_or_else(|_| src.to_path_buf());
    let dst_canonical = dst.canonicalize().unwrap_or_else(|_| dst.to_path_buf());

    if src_canonical == dst_canonical {
        return Ok(());
    }
    if dst_canonical.starts_with(&src_canonical) {
        return Err(format!(
            "Target directory '{}' cannot be inside the current directory '{}'",
            dst.display(),
            src.display()
        ));
    }
    if src_canonical.starts_with(&dst_canonical) {
        return Err(format!(
            "Target directory '{}' cannot contain the current directory '{}'",
            dst.display(),
            src.display()
        ));
    }
    Ok(())
}
#[allow(clippy::permissions_set_readonly_false)]
fn clear_readonly_if_present(path: &Path) {
    if let Ok(metadata) = path.metadata() {
        let mut perms = metadata.permissions();
        if perms.readonly() {
            perms.set_readonly(false);
            let _ = std::fs::set_permissions(path, perms);
        }
    }
}

fn safe_remove_file(path: &Path) -> std::io::Result<()> {
    clear_readonly_if_present(path);
    std::fs::remove_file(path)
}

fn safe_remove_dir_all(path: &Path) -> std::io::Result<()> {
    clear_readonly_if_present(path);
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                let _ = safe_remove_dir_all(&p);
            } else {
                let _ = safe_remove_file(&p);
            }
        }
    }
    std::fs::remove_dir_all(path)
}

pub fn cleanup_data_directory_contents(
    src_data_dir: &Path,
    default_data_dir: &Path,
    models_were_copied: bool,
) -> Vec<PathBuf> {
    if !src_data_dir.exists() {
        return Vec::new();
    }

    let mut unremoved_files = Vec::new();

    for dir_name in DATA_MIGRATION_SUBDIRECTORIES {
        let sub = src_data_dir.join(dir_name);
        if sub.exists() {
            match safe_remove_dir_all(&sub) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("Failed to remove old subdirectory {}: {}", sub.display(), e);
                    unremoved_files.push(sub);
                }
            }
        }
    }

    if models_were_copied {
        let models_dir = src_data_dir.join("models");
        if models_dir.exists() {
            match safe_remove_dir_all(&models_dir) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!(
                        "Failed to remove old models directory {}: {}",
                        models_dir.display(),
                        e
                    );
                    unremoved_files.push(models_dir);
                }
            }
        }
    }

    for file_name in DATA_MIGRATION_FILE_NAMES {
        let file = src_data_dir.join(file_name);
        if file.exists() {
            match safe_remove_file(&file) {
                Ok(()) => {}
                Err(e) => {
                    log::warn!("Failed to remove old file {}: {}", file.display(), e);
                    unremoved_files.push(file);
                }
            }
        }
    }
    if src_data_dir != default_data_dir && unremoved_files.is_empty() {
        let _ = std::fs::remove_dir(src_data_dir);
    }

    unremoved_files
}

pub fn cleanup_pending_storage_locations(default_app_local_data_dir: &Path) {
    let mut bootstrap = load_bootstrap_config(default_app_local_data_dir);
    let active_data_dir = resolve_active_data_dir(default_app_local_data_dir);
    let active_models_dir = resolve_active_models_dir(default_app_local_data_dir, &active_data_dir);

    let mut changed = false;

    if !bootstrap.pending_cleanup_dirs.is_empty() {
        let pending = std::mem::take(&mut bootstrap.pending_cleanup_dirs);
        let mut still_pending = Vec::new();

        for dir in pending {
            if dir == active_data_dir || dir == active_models_dir {
                continue;
            }
            if dir.exists() {
                let unremoved =
                    cleanup_data_directory_contents(&dir, default_app_local_data_dir, true);
                if !unremoved.is_empty() {
                    still_pending.push(dir);
                }
            }
        }

        bootstrap.pending_cleanup_dirs = still_pending;
        changed = true;
    }

    if bootstrap.custom_data_dir.is_some() && active_data_dir != default_app_local_data_dir {
        let old_db = default_app_local_data_dir.join("sona.db");
        if old_db.exists() {
            let _ = cleanup_data_directory_contents(
                default_app_local_data_dir,
                default_app_local_data_dir,
                bootstrap.custom_models_dir.is_some()
                    || active_models_dir != default_app_local_data_dir.join("models"),
            );
        }
    }

    if changed {
        let _ = save_bootstrap_config(default_app_local_data_dir, &bootstrap);
    }
}

pub fn cleanup_pending_storage_locations_for_app<R: Runtime>(app: &AppHandle<R>) {
    if let Ok(default_dir) = default_app_local_data_dir_for_app(app) {
        cleanup_pending_storage_locations(&default_dir);
    }
}

pub async fn migrate_data_directory<R: Runtime>(
    app: &AppHandle<R>,
    target_dir_str: String,
    copy_existing: bool,
) -> Result<StorageDirectoriesInfo, String> {
    let _guard = MigrationGuard::acquire()?;
    check_active_tasks_idle(app).await?;
    let target_path = PathBuf::from(target_dir_str.trim());
    validate_target_directory(&target_path)?;

    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);

    let active_canonical = active_data_dir
        .canonicalize()
        .unwrap_or_else(|_| active_data_dir.clone());
    let target_canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.clone());
    if active_canonical == target_canonical {
        return get_storage_directories_info(app);
    }
    check_path_overlap(&active_data_dir, &target_path)?;

    if let Ok(current_db) = crate::platform::database::try_sqlite_database(app) {
        let _ = current_db.with_write_connection(|conn| {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            let _ = conn.execute_batch("PRAGMA analytics.wal_checkpoint(TRUNCATE);");
            Ok(())
        });
    }

    let mut unremoved = Vec::new();
    let mut models_were_copied = false;
    if copy_existing && active_data_dir.exists() {
        std::fs::create_dir_all(&target_path).map_err(|e| e.to_string())?;

        let bootstrap = load_bootstrap_config(&default_data_dir);
        let should_copy_models = bootstrap.custom_models_dir.is_none();
        if should_copy_models
            && let Some(asr_state) = app.try_state::<crate::integrations::asr::AsrState>()
        {
            asr_state.clear_model_caches().await;
        }

        let mut total_bytes = 0u64;
        for file_name in DATA_MIGRATION_COPY_FILE_NAMES {
            let src_file = active_data_dir.join(file_name);
            if let Ok(meta) = src_file.metadata() {
                total_bytes += meta.len();
            }
        }
        for dir_name in DATA_MIGRATION_SUBDIRECTORIES {
            let src_sub = active_data_dir.join(dir_name);
            total_bytes += calculate_directory_size(&src_sub, 16);
        }
        if should_copy_models {
            let src_models = active_data_dir.join("models");
            total_bytes += calculate_directory_size(&src_models, 16);
        }

        let mut emitter = MigrationProgressEmitter::new(app, total_bytes);
        emitter.emit("preparing", "");
        for file_name in DATA_MIGRATION_COPY_FILE_NAMES {
            let src_file = active_data_dir.join(file_name);
            if src_file.exists() && src_file.is_file() {
                let dst_file = target_path.join(file_name);
                emitter
                    .copy_file_streaming(&src_file, &dst_file)
                    .map_err(|e| format!("Failed to copy {}: {}", src_file.display(), e))?;
            }
        }

        for dir_name in DATA_MIGRATION_SUBDIRECTORIES {
            let src_sub = active_data_dir.join(dir_name);
            if src_sub.exists() && src_sub.is_dir() {
                let dst_sub = target_path.join(dir_name);
                copy_directory_contents_with_progress(&src_sub, &dst_sub, &mut emitter, 16)
                    .map_err(|e| {
                        format!("Failed to copy directory {}: {}", src_sub.display(), e)
                    })?;
            }
        }

        if should_copy_models {
            let src_models = active_data_dir.join("models");
            if src_models.exists() && src_models.is_dir() {
                let dst_models = target_path.join("models");
                copy_directory_contents_with_progress(&src_models, &dst_models, &mut emitter, 16)
                    .map_err(|e| format!("Failed to copy models directory: {}", e))?;
                models_were_copied = true;
            }
        }
    }

    let new_db = crate::platform::database::open_and_migrate_sqlite_for_path_with_prompt(
        &target_path,
        crate::platform::startup_dialog::prompt_legacy_database_migration,
    )
    .map_err(|e| format!("Failed to open database at new location: {e}"))?;
    let new_context = Arc::new(
        sona_sqlite::SqliteApplicationContext::from_database(&target_path, new_db.clone())
            .map_err(|e| format!("Failed to create SQLite application context: {e}"))?,
    );

    let target_history_dir = target_path.join("history");
    if let Err(e) = app
        .asset_protocol_scope()
        .allow_directory(&target_history_dir, true)
    {
        log::warn!("Failed to allow target history directory in asset scope: {e}");
    }
    let _ = crate::platform::database::reload_sqlite_application_context(app, new_context);

    let new_dashboard_service =
        crate::platform::dashboard::create_dashboard_service(target_path.clone(), new_db);
    if let Some(dashboard_state) =
        app.try_state::<crate::platform::dashboard::DesktopDashboardState>()
    {
        let _ = dashboard_state.reload(new_dashboard_service);
    }

    if let Some(sync_manager) = app.try_state::<crate::platform::sync::DesktopSyncManager>() {
        sync_manager.reset().await;
    }

    if copy_existing && active_data_dir.exists() {
        unremoved = cleanup_data_directory_contents(
            &active_data_dir,
            &default_data_dir,
            models_were_copied,
        );
    }

    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    if target_path == default_data_dir {
        bootstrap.custom_data_dir = None;
    } else {
        bootstrap.custom_data_dir = Some(target_path);
    }
    if !unremoved.is_empty() && !bootstrap.pending_cleanup_dirs.contains(&active_data_dir) {
        bootstrap.pending_cleanup_dirs.push(active_data_dir);
    }
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    let _ = app.emit(
        STORAGE_MIGRATION_PROGRESS_EVENT,
        StorageMigrationProgress {
            phase: "done".to_string(),
            current_file: String::new(),
            copied_bytes: 1,
            total_bytes: 1,
            percent: 100.0,
        },
    );

    get_storage_directories_info(app)
}

pub async fn reset_data_directory<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageDirectoriesInfo, String> {
    let _guard = MigrationGuard::acquire()?;
    check_active_tasks_idle(app).await?;
    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);

    let active_canonical = active_data_dir
        .canonicalize()
        .unwrap_or_else(|_| active_data_dir.clone());
    let default_canonical = default_data_dir
        .canonicalize()
        .unwrap_or_else(|_| default_data_dir.clone());
    if active_canonical == default_canonical {
        return get_storage_directories_info(app);
    }

    if let Ok(current_db) = crate::platform::database::try_sqlite_database(app) {
        let _ = current_db.with_write_connection(|conn| {
            conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
            let _ = conn.execute_batch("PRAGMA analytics.wal_checkpoint(TRUNCATE);");
            Ok(())
        });
    }

    let new_db = crate::platform::database::open_and_migrate_sqlite_for_path_with_prompt(
        &default_data_dir,
        crate::platform::startup_dialog::prompt_legacy_database_migration,
    )
    .map_err(|e| format!("Failed to open database at default location: {e}"))?;
    let new_context = Arc::new(
        sona_sqlite::SqliteApplicationContext::from_database(&default_data_dir, new_db.clone())
            .map_err(|e| format!("Failed to create SQLite application context: {e}"))?,
    );

    let default_history_dir = default_data_dir.join("history");
    if let Err(e) = app
        .asset_protocol_scope()
        .allow_directory(&default_history_dir, true)
    {
        log::warn!("Failed to allow default history directory in asset scope: {e}");
    }
    let _ = crate::platform::database::reload_sqlite_application_context(app, new_context);

    let new_dashboard_service =
        crate::platform::dashboard::create_dashboard_service(default_data_dir.clone(), new_db);
    if let Some(dashboard_state) =
        app.try_state::<crate::platform::dashboard::DesktopDashboardState>()
    {
        let _ = dashboard_state.reload(new_dashboard_service);
    }

    if let Some(sync_manager) = app.try_state::<crate::platform::sync::DesktopSyncManager>() {
        sync_manager.reset().await;
    }

    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    bootstrap.custom_data_dir = None;
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    get_storage_directories_info(app)
}
pub async fn set_models_directory<R: Runtime>(
    app: &AppHandle<R>,
    target_dir_str: String,
    move_existing: bool,
) -> Result<StorageDirectoriesInfo, String> {
    let _guard = MigrationGuard::acquire()?;
    check_active_tasks_idle(app).await?;
    let target_path = PathBuf::from(target_dir_str.trim());
    validate_target_directory(&target_path)?;

    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let active_data_dir = resolve_active_data_dir(&default_data_dir);
    let active_models_dir = resolve_active_models_dir(&default_data_dir, &active_data_dir);

    let active_canonical = active_models_dir
        .canonicalize()
        .unwrap_or_else(|_| active_models_dir.clone());
    let target_canonical = target_path
        .canonicalize()
        .unwrap_or_else(|_| target_path.clone());
    if active_canonical == target_canonical {
        return get_storage_directories_info(app);
    }

    check_path_overlap(&active_models_dir, &target_path)?;

    // Evict all loaded models before moving files so file locks are released!
    if let Some(asr_state) = app.try_state::<crate::integrations::asr::AsrState>() {
        asr_state.clear_model_caches().await;
    }

    if move_existing && active_models_dir.exists() {
        let total_bytes = calculate_directory_size(&active_models_dir, 16);
        let mut emitter = MigrationProgressEmitter::new(app, total_bytes);
        emitter.emit("preparing", "");
        copy_directory_contents_with_progress(&active_models_dir, &target_path, &mut emitter, 16)
            .map_err(|e| format!("Failed to copy model files: {}", e))?;

        if let Err(e) = safe_remove_dir_all(&active_models_dir) {
            log::warn!(
                "Failed to remove old models directory {}: {}",
                active_models_dir.display(),
                e
            );
        }
    }

    let default_models_dir = default_data_dir.join("models");
    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    if target_path == default_models_dir {
        bootstrap.custom_models_dir = None;
    } else {
        bootstrap.custom_models_dir = Some(target_path);
    }
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    let _ = app.emit(
        STORAGE_MIGRATION_PROGRESS_EVENT,
        StorageMigrationProgress {
            phase: "done".to_string(),
            current_file: String::new(),
            copied_bytes: 1,
            total_bytes: 1,
            percent: 100.0,
        },
    );

    get_storage_directories_info(app)
}

pub async fn reset_models_directory<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<StorageDirectoriesInfo, String> {
    let _guard = MigrationGuard::acquire()?;
    check_active_tasks_idle(app).await?;
    // Evict all loaded models before resetting path
    if let Some(asr_state) = app.try_state::<crate::integrations::asr::AsrState>() {
        asr_state.clear_model_caches().await;
    }

    let default_data_dir = default_app_local_data_dir_for_app(app)?;
    let mut bootstrap = load_bootstrap_config(&default_data_dir);
    bootstrap.custom_models_dir = None;
    save_bootstrap_config(&default_data_dir, &bootstrap).map_err(|e| e.to_string())?;

    get_storage_directories_info(app)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoragePathOpenTarget {
    RevealItem(PathBuf),
    OpenDirectory(PathBuf),
}

pub fn resolve_storage_path_open_target(raw_path: &Path) -> StoragePathOpenTarget {
    if raw_path.is_file() {
        StoragePathOpenTarget::RevealItem(raw_path.to_path_buf())
    } else if raw_path.is_dir() {
        StoragePathOpenTarget::OpenDirectory(raw_path.to_path_buf())
    } else if raw_path.exists() {
        StoragePathOpenTarget::RevealItem(raw_path.to_path_buf())
    } else if raw_path.extension().is_some() || raw_path.file_name().is_some_and(|n| n == "ffmpeg")
    {
        if let Some(parent) = raw_path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let _ = std::fs::create_dir_all(parent);
            StoragePathOpenTarget::OpenDirectory(parent.to_path_buf())
        } else {
            let _ = std::fs::create_dir_all(raw_path);
            StoragePathOpenTarget::OpenDirectory(raw_path.to_path_buf())
        }
    } else {
        let _ = std::fs::create_dir_all(raw_path);
        StoragePathOpenTarget::OpenDirectory(raw_path.to_path_buf())
    }
}

pub fn open_storage_path<R: Runtime>(app: &AppHandle<R>, path_str: String) -> Result<(), String> {
    let trimmed = path_str.trim().trim_matches('"').trim_matches('\'');
    if trimmed.is_empty() {
        return Err("Path cannot be empty".to_string());
    }
    let path = PathBuf::from(trimmed);
    let target = resolve_storage_path_open_target(&path);

    match target {
        StoragePathOpenTarget::RevealItem(target_path) => {
            if app.opener().reveal_item_in_dir(&target_path).is_ok() {
                return Ok(());
            }
            if let Some(parent) = target_path
                .parent()
                .filter(|p| !p.as_os_str().is_empty() && p.exists())
            {
                app.opener()
                    .open_path(parent.to_string_lossy(), None::<&str>)
                    .map_err(|e| e.to_string())?;
                return Ok(());
            }
            app.opener()
                .reveal_item_in_dir(&target_path)
                .map_err(|e| e.to_string())
        }
        StoragePathOpenTarget::OpenDirectory(target_path) => app
            .opener()
            .open_path(target_path.to_string_lossy(), None::<&str>)
            .map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn bootstrap_config_load_and_save() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path();

        let initial = load_bootstrap_config(default_dir);
        assert_eq!(initial, StorageBootstrapConfig::default());

        let custom_data = default_dir.join("custom_data");
        let custom_models = default_dir.join("custom_models");
        let config = StorageBootstrapConfig {
            custom_data_dir: Some(custom_data.clone()),
            custom_models_dir: Some(custom_models.clone()),
            pending_cleanup_dirs: Vec::new(),
        };

        save_bootstrap_config(default_dir, &config).unwrap();

        let loaded = load_bootstrap_config(default_dir);
        assert_eq!(loaded.custom_data_dir, Some(custom_data.clone()));
        assert_eq!(loaded.custom_models_dir, Some(custom_models.clone()));

        assert_eq!(resolve_active_data_dir(default_dir), custom_data);
        assert_eq!(
            resolve_active_models_dir(default_dir, &custom_data),
            custom_models
        );
    }

    #[test]
    fn fallback_to_default_when_custom_dir_empty() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path();

        let config = StorageBootstrapConfig {
            custom_data_dir: Some(PathBuf::from("")),
            custom_models_dir: Some(PathBuf::from("")),
            pending_cleanup_dirs: Vec::new(),
        };
        save_bootstrap_config(default_dir, &config).unwrap();

        assert_eq!(resolve_active_data_dir(default_dir), default_dir);
        assert_eq!(
            resolve_active_models_dir(default_dir, default_dir),
            default_dir.join("models")
        );
    }

    #[test]
    fn validate_target_directory_checks_write_permission() {
        let temp = TempDir::new().unwrap();
        let valid_dir = temp.path().join("valid_sub");
        assert!(validate_target_directory(&valid_dir).is_ok());

        assert!(validate_target_directory(Path::new("")).is_err());

        let file_path = temp.path().join("file.txt");
        std::fs::write(&file_path, b"hello").unwrap();
        assert!(validate_target_directory(&file_path).is_err());
    }

    #[test]
    fn copy_directory_contents_recursively() {
        let temp = TempDir::new().unwrap();
        let src = temp.path().join("src");
        let dst = temp.path().join("dst");

        std::fs::create_dir_all(src.join("sub")).unwrap();
        std::fs::write(src.join("a.txt"), b"aaa").unwrap();
        std::fs::write(src.join("sub").join("b.txt"), b"bbb").unwrap();

        copy_directory_contents(&src, &dst).unwrap();

        assert_eq!(std::fs::read(dst.join("a.txt")).unwrap(), b"aaa");
        assert_eq!(
            std::fs::read(dst.join("sub").join("b.txt")).unwrap(),
            b"bbb"
        );
    }
    #[test]
    fn check_path_overlap_detects_nesting() {
        let temp = TempDir::new().unwrap();
        let dir_a = temp.path().join("dir_a");
        let dir_b = temp.path().join("dir_b");
        let dir_a_sub = dir_a.join("nested");

        std::fs::create_dir_all(&dir_a_sub).unwrap();
        std::fs::create_dir_all(&dir_b).unwrap();

        assert!(check_path_overlap(&dir_a, &dir_b).is_ok());
        assert!(check_path_overlap(&dir_a, &dir_a).is_ok());
        assert!(check_path_overlap(&dir_a, &dir_a_sub).is_err());
        assert!(check_path_overlap(&dir_a_sub, &dir_a).is_err());
    }

    #[test]
    fn cleanup_data_directory_contents_removes_old_files_and_preserves_bootstrap() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path().join("default_data");
        std::fs::create_dir_all(&default_dir).unwrap();

        // Setup bootstrap file and data files in default_dir
        let bootstrap = StorageBootstrapConfig::default();
        save_bootstrap_config(&default_dir, &bootstrap).unwrap();

        let history_dir = default_dir.join("history");
        std::fs::create_dir_all(&history_dir).unwrap();
        std::fs::write(history_dir.join("rec1.wav"), b"wav_data").unwrap();

        let models_dir = default_dir.join("models");
        std::fs::create_dir_all(&models_dir).unwrap();
        std::fs::write(models_dir.join("model.bin"), b"model_data").unwrap();

        std::fs::write(default_dir.join("sona.db"), b"db_data").unwrap();
        std::fs::write(default_dir.join("sync.json"), b"{}").unwrap();

        let unremoved = cleanup_data_directory_contents(&default_dir, &default_dir, true);
        assert!(unremoved.is_empty());

        // Check that data files are removed
        assert!(!default_dir.join("sona.db").exists());
        assert!(!default_dir.join("sync.json").exists());
        assert!(!history_dir.exists());
        assert!(!models_dir.exists());

        // Check that bootstrap file and default_dir still exist
        assert!(default_dir.exists());
        assert!(default_dir.join(STORAGE_BOOTSTRAP_FILE_NAME).exists());
    }

    #[test]
    fn cleanup_data_directory_contents_removes_custom_directory_when_empty() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path().join("default_data");
        let custom_dir = temp.path().join("custom_data");
        std::fs::create_dir_all(&default_dir).unwrap();
        std::fs::create_dir_all(&custom_dir).unwrap();

        std::fs::write(custom_dir.join("sona.db"), b"db_data").unwrap();
        let history_dir = custom_dir.join("history");
        std::fs::create_dir_all(&history_dir).unwrap();
        std::fs::write(history_dir.join("rec1.wav"), b"wav_data").unwrap();

        let unremoved = cleanup_data_directory_contents(&custom_dir, &default_dir, false);
        assert!(unremoved.is_empty());

        // Custom directory itself should be removed when empty
        assert!(!custom_dir.exists());
    }

    #[test]
    fn cleanup_pending_storage_locations_cleans_orphaned_dirs() {
        let temp = TempDir::new().unwrap();
        let default_dir = temp.path().join("default_data");
        let custom_dir = temp.path().join("custom_data");
        let old_orphan_dir = temp.path().join("old_custom_data");
        std::fs::create_dir_all(&default_dir).unwrap();
        std::fs::create_dir_all(&custom_dir).unwrap();
        std::fs::create_dir_all(&old_orphan_dir).unwrap();

        std::fs::write(old_orphan_dir.join("sona.db"), b"db_data").unwrap();
        std::fs::write(custom_dir.join("sona.db"), b"new_db_data").unwrap();

        let config = StorageBootstrapConfig {
            custom_data_dir: Some(custom_dir.clone()),
            custom_models_dir: None,
            pending_cleanup_dirs: vec![old_orphan_dir.clone()],
        };
        save_bootstrap_config(&default_dir, &config).unwrap();

        cleanup_pending_storage_locations(&default_dir);

        // Old orphan dir should be gone
        assert!(!old_orphan_dir.exists());
        // Current custom dir should be untouched
        assert!(custom_dir.join("sona.db").exists());

        // Config should have cleared pending_cleanup_dirs
        let loaded = load_bootstrap_config(&default_dir);
        assert!(loaded.pending_cleanup_dirs.is_empty());
    }

    #[test]
    fn resolve_storage_path_target_for_existing_file() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("ffmpeg.exe");
        std::fs::write(&file_path, b"dummy").unwrap();

        assert_eq!(
            resolve_storage_path_open_target(&file_path),
            StoragePathOpenTarget::RevealItem(file_path)
        );
    }

    #[test]
    fn resolve_storage_path_target_for_existing_directory() {
        let temp = TempDir::new().unwrap();
        let dir_path = temp.path().join("models");
        std::fs::create_dir_all(&dir_path).unwrap();

        assert_eq!(
            resolve_storage_path_open_target(&dir_path),
            StoragePathOpenTarget::OpenDirectory(dir_path)
        );
    }

    #[test]
    fn resolve_storage_path_target_for_missing_file_with_extension() {
        let temp = TempDir::new().unwrap();
        let missing_file = temp.path().join("missing_ffmpeg.exe");

        assert_eq!(
            resolve_storage_path_open_target(&missing_file),
            StoragePathOpenTarget::OpenDirectory(temp.path().to_path_buf())
        );
        assert!(!missing_file.exists());
    }

    #[test]
    fn resolve_storage_path_target_for_missing_extensionless_ffmpeg() {
        let temp = TempDir::new().unwrap();
        let missing_ffmpeg = temp.path().join("ffmpeg");

        assert_eq!(
            resolve_storage_path_open_target(&missing_ffmpeg),
            StoragePathOpenTarget::OpenDirectory(temp.path().to_path_buf())
        );
        assert!(!missing_ffmpeg.exists());
    }

    #[test]
    fn resolve_storage_path_target_for_missing_directory_without_extension() {
        let temp = TempDir::new().unwrap();
        let missing_dir = temp.path().join("uncreated_models");

        assert_eq!(
            resolve_storage_path_open_target(&missing_dir),
            StoragePathOpenTarget::OpenDirectory(missing_dir.clone())
        );
        assert!(missing_dir.is_dir());
    }

    #[test]
    fn test_migration_guard_mutual_exclusion() {
        assert!(!is_migration_in_progress());
        let guard1 = MigrationGuard::acquire().unwrap();
        assert!(is_migration_in_progress());

        let guard2_err = MigrationGuard::acquire();
        assert!(guard2_err.is_err());

        drop(guard1);
        assert!(!is_migration_in_progress());

        let guard3 = MigrationGuard::acquire().unwrap();
        assert!(is_migration_in_progress());
        drop(guard3);
        assert!(!is_migration_in_progress());
    }

    #[test]
    fn test_calculate_directory_size() {
        let temp = TempDir::new().unwrap();
        let sub = temp.path().join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("file1.bin"), b"12345").unwrap();
        std::fs::write(temp.path().join("file2.bin"), b"abcdefgh").unwrap();

        let size = calculate_directory_size(temp.path(), 16);
        assert_eq!(size, 13);
    }
}
