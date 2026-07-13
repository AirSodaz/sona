use serde_json::Value;
use std::sync::Arc;
use tauri::{AppHandle, Runtime, State};

use crate::platform::webdav::{RemoteBackupEntry, WebDavConfigPayload, WebDavConnectionResult};

use sona_core::recovery::types::RecoverySnapshot;
use sona_core::task_ledger::types::{TaskLedgerRecord, TaskLedgerSnapshot};

// Command wrappers & implementations

#[tauri::command]
pub fn greet(name: &str) -> String {
    crate::platform::system::greet(name)
}

#[tauri::command]
pub fn force_exit(app: AppHandle) {
    crate::platform::system::force_exit(app);
}

#[tauri::command]
pub fn inject_text(
    text: String,
    shortcut_modifiers: Option<Vec<crate::platform::system::ShortcutModifier>>,
) -> Result<(), String> {
    crate::platform::system::inject_text(text, shortcut_modifiers)
}

#[tauri::command]
pub fn get_mouse_position() -> Result<(i32, i32), String> {
    crate::platform::system::get_mouse_position()
}

#[tauri::command]
pub fn get_text_cursor_position() -> Result<Option<(i32, i32)>, String> {
    crate::platform::system::get_text_cursor_position()
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
    crate::platform::hardware::check_gpu_availability().await
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
    crate::platform::runtime_status::open_log_folder(app).await
}

#[tauri::command]
pub async fn get_runtime_environment_status(
    app: AppHandle,
) -> Result<crate::platform::runtime_status::RuntimeEnvironmentStatus, String> {
    crate::platform::runtime_status::get_runtime_environment_status(app).await
}

#[tauri::command]
pub async fn get_path_statuses(
    paths: Vec<String>,
) -> Result<Vec<crate::platform::runtime_status::RuntimePathStatus>, String> {
    crate::platform::runtime_status::get_path_statuses(paths).await
}

#[tauri::command]
pub async fn webdav_test_connection(
    config: WebDavConfigPayload,
) -> Result<WebDavConnectionResult, String> {
    crate::platform::webdav::test_connection(config).await
}

#[tauri::command]
pub async fn webdav_list_backups(
    config: WebDavConfigPayload,
) -> Result<Vec<RemoteBackupEntry>, String> {
    crate::platform::webdav::list_backups(config).await
}

#[tauri::command]
pub async fn webdav_upload_backup(
    config: WebDavConfigPayload,
    local_archive_path: String,
) -> Result<(), String> {
    crate::platform::webdav::upload_backup(config, local_archive_path).await
}

#[tauri::command]
pub async fn webdav_download_backup(
    config: WebDavConfigPayload,
    href: String,
    output_path: String,
) -> Result<(), String> {
    crate::platform::webdav::download_backup(config, href, output_path).await
}

#[tauri::command]
pub async fn get_model_catalog_snapshot(
    app: AppHandle,
) -> Result<crate::platform::preset_models::ModelCatalogSnapshot, String> {
    crate::platform::preset_models::get_model_catalog_snapshot_for_app(&app).await
}

#[tauri::command(rename = "resolve_model_catalog_selected_ids")]
pub async fn resolve_model_catalog_selected_ids_command(
    app: AppHandle,
    paths: crate::platform::preset_models::ModelSelectionPaths,
) -> Result<crate::platform::preset_models::ModelCatalogSelectedIds, String> {
    crate::platform::preset_models::resolve_model_catalog_selected_ids_for_app(&app, paths).await
}

#[tauri::command]
pub async fn get_diagnostics_core_snapshot(
    app: AppHandle,
    state: State<'_, crate::integrations::asr::AsrState>,
    input: crate::platform::diagnostics::DiagnosticsCoreInput,
) -> Result<crate::platform::diagnostics::DiagnosticsCoreSnapshot, String> {
    crate::platform::diagnostics::get_diagnostics_core_snapshot_for_app(&app, state, input).await
}

// Relocated task_ledger commands

#[tauri::command]
pub async fn task_ledger_load_snapshot(app: AppHandle) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::load_snapshot(&app).await
}

#[tauri::command]
pub async fn task_ledger_upsert_task(
    app: AppHandle,
    record: TaskLedgerRecord,
) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::upsert_task(&app, record).await
}

#[tauri::command]
pub async fn task_ledger_patch_task(
    app: AppHandle,
    id: String,
    patch: Value,
) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::patch_task(&app, id, patch).await
}

#[tauri::command]
pub async fn task_ledger_remove_task(
    app: AppHandle,
    id: String,
) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::remove_task(&app, id).await
}

#[tauri::command]
pub async fn task_ledger_clear_resolved(app: AppHandle) -> Result<TaskLedgerSnapshot, String> {
    crate::platform::task_ledger_repository::clear_resolved(&app).await
}

// Relocated recovery commands

#[tauri::command]
pub async fn recovery_load_snapshot(app: AppHandle) -> Result<RecoverySnapshot, String> {
    crate::platform::recovery_repository::load_snapshot_for_app(&app).await
}

#[tauri::command]
pub async fn recovery_save_snapshot(
    app: AppHandle,
    items: Vec<Value>,
) -> Result<RecoverySnapshot, String> {
    crate::platform::recovery_repository::save_snapshot_for_app(&app, items).await
}

#[tauri::command]
pub async fn recovery_persist_queue_snapshot(
    app: AppHandle,
    queue_items: Vec<Value>,
    resolved_ids: Option<Vec<String>>,
) -> Result<(), String> {
    crate::platform::recovery_repository::persist_queue_snapshot_for_app(
        &app,
        queue_items,
        resolved_ids,
    )
    .await
}

// Relocated / Wrapped speaker commands

#[tauri::command]
pub async fn annotate_speaker_segments_from_file(
    file_path: String,
    segments: Vec<crate::integrations::asr::TranscriptSegment>,
    speaker_processing: Option<sona_core::transcription::speaker::SpeakerProcessingConfig>,
) -> Result<Vec<crate::integrations::asr::TranscriptSegment>, String> {
    crate::platform::speaker_processing::annotate_speaker_segments_from_file(
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
) -> Result<sona_core::transcription::speaker::SpeakerProfileSample, String> {
    crate::platform::speaker_processing::import_speaker_profile_sample_for_app(
        &app,
        profile_id,
        source_path,
        source_name,
    )
    .await
}

#[tauri::command]
pub fn build_speaker_review_snapshot(
    segments: Vec<crate::integrations::asr::TranscriptSegment>,
    active_filter: sona_core::transcription::speaker_review::SpeakerReviewFilter,
) -> sona_core::transcription::speaker_review::SpeakerReviewSnapshot {
    sona_core::transcription::speaker_review::build_speaker_review_snapshot(segments, active_filter)
}

#[tauri::command]
pub async fn apply_speaker_profile_to_group(
    request: sona_core::transcription::speaker_correction::ApplySpeakerProfileToGroupRequest,
) -> Result<sona_core::transcription::speaker_correction::SpeakerCorrectionResponse, String> {
    sona_core::transcription::speaker_correction::apply_speaker_profile_to_group(request).await
}

#[tauri::command]
pub async fn reset_speaker_group_to_anonymous(
    request: sona_core::transcription::speaker_correction::SpeakerGroupRequest,
) -> Result<sona_core::transcription::speaker_correction::SpeakerCorrectionResponse, String> {
    sona_core::transcription::speaker_correction::reset_speaker_group_to_anonymous(request).await
}

#[tauri::command]
pub async fn confirm_speaker_group_review(
    request: sona_core::transcription::speaker_correction::SpeakerGroupRequest,
) -> Result<sona_core::transcription::speaker_correction::SpeakerCorrectionResponse, String> {
    sona_core::transcription::speaker_correction::confirm_speaker_group_review(request).await
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

#[tauri::command]
pub async fn get_api_server_dashboard_snapshot(
    controller: State<'_, crate::app::server::ApiServerController>,
) -> Result<sona_api_server::ApiServerDashboardSnapshot, String> {
    controller.dashboard_snapshot().await
}

// Wrapped media formats commands

#[tauri::command]
pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    crate::platform::media_detector::check_media_formats(paths).await
}

// Wrapped config commands

#[tauri::command]
pub fn load_app_config<R: Runtime>(app: AppHandle<R>) -> Result<Option<Value>, String> {
    crate::platform::app_config::load_config(&app)
}

#[tauri::command]
pub fn save_app_config<R: Runtime>(app: AppHandle<R>, config: Value) -> Result<(), String> {
    crate::platform::app_config::save_config(&app, config)
}

#[tauri::command]
pub fn get_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
) -> Result<Option<Value>, String> {
    crate::platform::app_config::get_setting(&app, key)
}

#[tauri::command]
pub fn set_app_setting<R: Runtime>(
    app: AppHandle<R>,
    key: String,
    value: Value,
) -> Result<(), String> {
    crate::platform::app_config::set_setting(&app, key, value)
}

#[tauri::command(rename_all = "camelCase")]
pub fn migrate_app_config(
    saved_config: Option<Value>,
    legacy_config: Option<Value>,
    default_rule_set_name: String,
) -> sona_core::config::MigrationResult {
    sona_core::config::migrate_app_config(saved_config, legacy_config, default_rule_set_name)
}

#[tauri::command(rename_all = "camelCase")]
pub fn resolve_effective_config(global_config: Value, project: Option<Value>) -> Value {
    sona_core::config::resolve_effective_config(global_config, project)
}
