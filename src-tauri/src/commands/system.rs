use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::core::database::DatabaseError;
use crate::core::paths::PathProvider;

// task_ledger helper functions (copied from core/task_ledger/commands.rs)
use crate::core::task_ledger::types::{
    TASK_LEDGER_UPDATED_EVENT, TaskLedgerRecord, TaskLedgerSnapshot,
};
use crate::core::task_ledger_sqlite::SqliteLedgerRepository;

async fn run_task_ledger_repository_task<R, T, F>(app: &AppHandle<R>, task: F) -> Result<T, String>
where
    R: Runtime,
    T: Send + 'static,
    F: FnOnce(SqliteLedgerRepository) -> Result<T, DatabaseError> + Send + 'static,
{
    let db = Arc::clone(app.state::<Arc<crate::core::database::Database>>().inner());
    tauri::async_runtime::spawn_blocking(move || {
        task(SqliteLedgerRepository::new(db)).map_err(|e| e.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}

fn emit_task_ledger_snapshot<R: Runtime>(
    app: &AppHandle<R>,
    snapshot: &TaskLedgerSnapshot,
) -> Result<(), String> {
    app.emit(TASK_LEDGER_UPDATED_EVENT, snapshot)
        .map_err(|error| error.to_string())
}

// recovery helper functions (copied from core/recovery/commands.rs)
use crate::repositories::recovery::FsRecoveryRepository;
use sona_core::recovery::repository::RecoveryRepository;
use sona_core::recovery::types::RecoverySnapshot;

async fn run_recovery_repository_task<T, F>(
    provider: &dyn PathProvider,
    task: F,
) -> Result<T, String>
where
    T: Send + 'static,
    F: FnOnce(FsRecoveryRepository) -> Result<T, String> + Send + 'static,
{
    let app_local_data_dir = provider.resolve_path(crate::core::paths::PathKind::AppLocalData)?;
    tauri::async_runtime::spawn_blocking(move || {
        task(FsRecoveryRepository::new(app_local_data_dir))
    })
    .await
    .map_err(|error| error.to_string())?
}

// Command wrappers & implementations

#[tauri::command]
pub fn greet(name: &str) -> String {
    crate::app::system::greet(name)
}

#[tauri::command]
pub fn force_exit(app: AppHandle) {
    crate::app::system::force_exit(app);
}

#[tauri::command]
pub fn inject_text(
    text: String,
    shortcut_modifiers: Option<Vec<crate::app::system::ShortcutModifier>>,
) -> Result<(), String> {
    crate::app::system::inject_text(text, shortcut_modifiers)
}

#[tauri::command]
pub fn get_mouse_position() -> Result<(i32, i32), String> {
    crate::app::system::get_mouse_position()
}

#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    crate::app::system::get_text_cursor_position()
}

#[tauri::command]
pub async fn get_dashboard_snapshot(
    service: State<'_, Arc<crate::app::dashboard::AppDashboardService>>,
    request: crate::app::dashboard::DashboardSnapshotRequest,
) -> Result<Value, String> {
    crate::app::dashboard::get_dashboard_snapshot(service, request).await
}

#[tauri::command]
pub async fn check_gpu_availability() -> Result<bool, String> {
    crate::app::hardware::check_gpu_availability().await
}

#[tauri::command]
pub async fn update_tray_menu(
    app: AppHandle,
    show_text: String,
    settings_text: String,
    updates_text: String,
    quit_text: String,
    caption_text: String,
    caption_checked: bool,
) -> Result<(), String> {
    crate::app::tray::update_tray_menu(
        app,
        show_text,
        settings_text,
        updates_text,
        quit_text,
        caption_text,
        caption_checked,
    )
    .await
}

#[tauri::command]
pub fn set_minimize_to_tray(state: State<'_, crate::app::settings::AppSettings>, enabled: bool) {
    crate::app::settings::set_minimize_to_tray(state, enabled);
}

#[tauri::command]
pub fn set_log_level(
    state: State<'_, crate::app::settings::AppSettings>,
    level: String,
) -> Result<(), String> {
    crate::app::settings::set_log_level(state, level)
}

#[tauri::command]
pub fn set_aux_window_state(
    state: State<'_, crate::app::window_state::AuxWindowStateStore>,
    label: String,
    payload: Value,
) -> Result<(), String> {
    crate::app::window_state::set_aux_window_state(state, label, payload)
}

#[tauri::command]
pub fn get_aux_window_state(
    state: State<'_, crate::app::window_state::AuxWindowStateStore>,
    label: String,
) -> Result<Option<Value>, String> {
    crate::app::window_state::get_aux_window_state(state, label)
}

#[tauri::command]
pub fn clear_aux_window_state(
    state: State<'_, crate::app::window_state::AuxWindowStateStore>,
    label: String,
) -> Result<(), String> {
    crate::app::window_state::clear_aux_window_state(state, label)
}

#[tauri::command]
pub async fn open_log_folder(app: AppHandle) -> Result<(), String> {
    crate::app::runtime_status::open_log_folder(app).await
}

#[tauri::command]
pub async fn get_runtime_environment_status(
    app: AppHandle,
) -> Result<crate::app::runtime_status::RuntimeEnvironmentStatus, String> {
    crate::app::runtime_status::get_runtime_environment_status(app).await
}

#[tauri::command]
pub async fn get_path_statuses(
    paths: Vec<String>,
) -> Result<Vec<crate::app::runtime_status::RuntimePathStatus>, String> {
    crate::app::runtime_status::get_path_statuses(paths).await
}

#[tauri::command]
pub async fn webdav_test_connection(
    config: crate::integrations::webdav::WebDavConfigPayload,
) -> Result<crate::integrations::webdav::WebDavConnectionResult, String> {
    crate::integrations::webdav::webdav_test_connection(config).await
}

#[tauri::command]
pub async fn webdav_list_backups(
    config: crate::integrations::webdav::WebDavConfigPayload,
) -> Result<Vec<crate::integrations::webdav::RemoteBackupEntry>, String> {
    crate::integrations::webdav::webdav_list_backups(config).await
}

#[tauri::command]
pub async fn webdav_upload_backup(
    config: crate::integrations::webdav::WebDavConfigPayload,
    local_archive_path: String,
) -> Result<(), String> {
    crate::integrations::webdav::webdav_upload_backup(config, local_archive_path).await
}

#[tauri::command]
pub async fn webdav_download_backup(
    config: crate::integrations::webdav::WebDavConfigPayload,
    href: String,
    output_path: String,
) -> Result<(), String> {
    crate::integrations::webdav::webdav_download_backup(config, href, output_path).await
}

#[tauri::command]
pub async fn get_model_catalog_snapshot(
    app: AppHandle,
) -> Result<crate::core::preset_models::ModelCatalogSnapshot, String> {
    crate::core::preset_models::get_model_catalog_snapshot(&app as &dyn PathProvider).await
}

#[tauri::command(rename = "resolve_model_catalog_selected_ids")]
pub async fn resolve_model_catalog_selected_ids_command(
    app: AppHandle,
    paths: crate::core::preset_models::ModelSelectionPaths,
) -> Result<crate::core::preset_models::ModelCatalogSelectedIds, String> {
    crate::core::preset_models::resolve_model_catalog_selected_ids_command(
        &app as &dyn PathProvider,
        paths,
    )
    .await
}

#[tauri::command]
pub async fn get_diagnostics_core_snapshot(
    app: AppHandle,
    state: State<'_, crate::integrations::asr::AsrState>,
    input: crate::core::diagnostics::DiagnosticsCoreInput,
) -> Result<crate::core::diagnostics::DiagnosticsCoreSnapshot, String> {
    crate::core::diagnostics::get_diagnostics_core_snapshot(&app as &dyn PathProvider, state, input)
        .await
}

// Relocated task_ledger commands

#[tauri::command]
pub async fn task_ledger_load_snapshot(app: AppHandle) -> Result<TaskLedgerSnapshot, String> {
    run_task_ledger_repository_task(&app, |repository| repository.load_snapshot()).await
}

#[tauri::command]
pub async fn task_ledger_upsert_task(
    app: AppHandle,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    run_task_ledger_repository_task(&app, move |repository| {
        repository.upsert_task(record)?;
        repository.load_snapshot()
    })
    .await
    .inspect(|snapshot| {
        let _ = emit_task_ledger_snapshot(&app, snapshot);
    })
}

#[tauri::command]
pub async fn task_ledger_patch_task(
    app: AppHandle,
    id: String,
    patch: Value,
) -> Result<TaskLedgerSnapshot, String> {
    run_task_ledger_repository_task(&app, move |repository| {
        repository.patch_task(&id, patch)?;
        repository.load_snapshot()
    })
    .await
    .inspect(|snapshot| {
        let _ = emit_task_ledger_snapshot(&app, snapshot);
    })
}

#[tauri::command]
pub async fn task_ledger_remove_task(
    app: AppHandle,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    run_task_ledger_repository_task(&app, move |repository| {
        repository.remove_task(&id)?;
        repository.load_snapshot()
    })
    .await
    .inspect(|snapshot| {
        let _ = emit_task_ledger_snapshot(&app, snapshot);
    })
}

#[tauri::command]
pub async fn task_ledger_clear_resolved(app: AppHandle) -> Result<TaskLedgerSnapshot, String> {
    run_task_ledger_repository_task(&app, |repository| {
        repository.clear_resolved()?;
        repository.load_snapshot()
    })
    .await
    .inspect(|snapshot| {
        let _ = emit_task_ledger_snapshot(&app, snapshot);
    })
}

// Relocated recovery commands

#[tauri::command]
pub async fn recovery_load_snapshot(app: AppHandle) -> Result<RecoverySnapshot, String> {
    run_recovery_repository_task(&app as &dyn PathProvider, |repository| {
        repository.load_snapshot()
    })
    .await
}

#[tauri::command]
pub async fn recovery_save_snapshot(
    app: AppHandle,
    items: Vec<Value>,
) -> Result<RecoverySnapshot, String> {
    run_recovery_repository_task(&app as &dyn PathProvider, move |repository| {
        repository.save_snapshot(items)
    })
    .await
}

#[tauri::command]
pub async fn recovery_persist_queue_snapshot(
    app: AppHandle,
    queue_items: Vec<Value>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    run_recovery_repository_task(&app as &dyn PathProvider, move |repository| {
        repository
            .persist_queue_snapshot_with_resolved_ids(queue_items, resolved_ids.unwrap_or_default())
            .map(|_| ())
    })
    .await
}

// Relocated / Wrapped speaker commands

#[tauri::command]
pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<crate::integrations::asr::TranscriptSegment>,
    speaker_processing: Option<crate::integrations::speaker::SpeakerProcessingConfig>,
) -> Result<Vec<crate::integrations::asr::TranscriptSegment>, String> {
    crate::integrations::speaker::annotate_speaker_segments_from_file(
        file_path,
        segments,
        speaker_processing,
    )
    .await
}

#[tauri::command]
pub async fn import_speaker_profile_sample(
    app: AppHandle,
    profile_id: String,
    source_path: String,
    source_name: Option<String>,
) -> Result<crate::integrations::speaker::SpeakerProfileSample, String> {
    crate::integrations::speaker::import_speaker_profile_sample(
        &app as &dyn PathProvider,
        profile_id,
        source_path,
        source_name,
    )
    .await
}

#[tauri::command]
pub fn build_speaker_review_snapshot(
    segments: Vec<crate::integrations::asr::TranscriptSegment>,
    active_filter: crate::core::speaker_review::SpeakerReviewFilter,
) -> crate::core::speaker_review::SpeakerReviewSnapshot {
    crate::core::speaker_review::build_speaker_review_snapshot(segments, active_filter)
}

#[tauri::command]
pub async fn apply_speaker_profile_to_group(
    request: crate::core::speaker_correction::ApplySpeakerProfileToGroupRequest,
) -> Result<crate::core::speaker_correction::SpeakerCorrectionResponse, String> {
    crate::core::speaker_correction::apply_speaker_profile_to_group(request).await
}

#[tauri::command]
pub async fn reset_speaker_group_to_anonymous(
    request: crate::core::speaker_correction::SpeakerGroupRequest,
) -> Result<crate::core::speaker_correction::SpeakerCorrectionResponse, String> {
    crate::core::speaker_correction::reset_speaker_group_to_anonymous(request).await
}

#[tauri::command]
pub async fn confirm_speaker_group_review(
    request: crate::core::speaker_correction::SpeakerGroupRequest,
) -> Result<crate::core::speaker_correction::SpeakerCorrectionResponse, String> {
    crate::core::speaker_correction::confirm_speaker_group_review(request).await
}

// Wrapped API server commands

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn start_api_server(
    app: AppHandle,
    controller: State<'_, crate::app::server::ApiServerController>,
    host: String,
    port: u16,
    api_key: String,
    max_concurrent: usize,
    max_queue_size: usize,
    max_upload_size_mb: usize,
    job_ttl_minutes: u64,
    max_streaming: usize,
    ip_whitelist: String,
    gpu_acceleration: String,
) -> Result<String, String> {
    crate::app::server::start_api_server(
        app,
        controller,
        host,
        port,
        api_key,
        max_concurrent,
        max_queue_size,
        max_upload_size_mb,
        job_ttl_minutes,
        max_streaming,
        ip_whitelist,
        gpu_acceleration,
    )
    .await
}

#[tauri::command]
pub async fn stop_api_server(
    controller: State<'_, crate::app::server::ApiServerController>,
) -> Result<(), String> {
    crate::app::server::stop_api_server(controller).await
}

// Wrapped media formats commands

#[tauri::command]
pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    crate::integrations::media_detector::check_media_formats(paths).await
}

// Wrapped config commands

fn sqlite_config_store<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<crate::core::config::sqlite_store::SqliteConfigStore, String> {
    let db = Arc::clone(app.state::<Arc<crate::core::database::Database>>().inner());
    Ok(crate::core::config::sqlite_store::SqliteConfigStore::new(
        db,
    ))
}

#[tauri::command]
pub fn load_app_config<R: Runtime>(app: AppHandle<R>) -> Result<Option<Value>, String> {
    sqlite_config_store(&app)?
        .load_config()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn save_app_config<R: Runtime>(app: AppHandle<R>, config: Value) -> Result<(), String> {
    sqlite_config_store(&app)?
        .save_config(&config)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
) -> Result<Option<Value>, String> {
    sqlite_config_store(&app)?
        .get_setting(&key)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn set_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
    value: Value,
) -> Result<(), String> {
    sqlite_config_store(&app)?
        .set_setting(&key, &value)
        .map_err(|error| error.to_string())
}

#[tauri::command(rename_all = "camelCase")]
pub fn migrate_app_config(
    saved_config: Option<Value>,
    legacy_config: Option<Value>,
    default_rule_set_name: String,
) -> crate::core::config::MigrationResult {
    crate::core::config::migrate_app_config(saved_config, legacy_config, default_rule_set_name)
}

#[tauri::command(rename_all = "camelCase")]
pub fn resolve_effective_config(global_config: Value, project: Option<Value>) -> Value {
    crate::core::config::resolve_effective_config(global_config, project)
}
