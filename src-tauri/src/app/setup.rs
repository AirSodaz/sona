use std::sync::Arc;
use tauri::{Listener, Manager};

use crate::core::paths::PathProvider;

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle_for_listener = app.handle().clone();
    let controller = app_handle_for_listener.state::<crate::app::server::ApiServerController>();

    let app_local_data_dir = (&app_handle_for_listener as &dyn PathProvider)
        .resolve_path(crate::core::paths::PathKind::AppLocalData)
        .expect("Failed to get app_local_data_dir");

    // Initialize the SQLite database
    let db = crate::core::database::Database::open(&app_local_data_dir)
        .expect("Failed to open database");
    crate::core::database::Database::set_global(db).expect("Database already initialized");

    // Migrate legacy JSON data to SQLite
    let migration_result = crate::core::database::legacy_migration::migrate_legacy_to_sqlite(
        crate::core::database::Database::global()?,
        &app_local_data_dir,
    )?;
    if migration_result.migrated {
        log::info!(
            "Migrated legacy data: {} history items, {} projects",
            migration_result.history_count,
            migration_result.project_count,
        );

        if !migration_result.errors.is_empty() {
            for err in &migration_result.errors {
                log::error!("[Migration] {err}");
            }
            log::warn!(
                "[Migration] {} error(s) occurred; legacy data preserved at original location",
                migration_result.errors.len(),
            );
        }

        if migration_result.errors.is_empty() {
            crate::core::database::legacy_migration::move_legacy_to_backup(&app_local_data_dir)?;
        }
    }

    let history_repo = Arc::new(crate::repositories::history::SqliteHistoryStore::new(
        app_local_data_dir.clone(),
    ));
    let project_repo = Arc::new(crate::repositories::project::SqliteProjectRepository::new(
        app_local_data_dir.clone(),
    ));
    let analytics_repo = Arc::new(crate::repositories::analytics::AnalyticsRepositoryImpl::new());

    let dashboard_service = Arc::new(crate::app::dashboard::AppDashboardService::new(
        history_repo,
        project_repo,
        analytics_repo,
    ));

    app.manage(dashboard_service);

    let config_for_listener = controller.online_asr_config.clone();
    let listener_app_handle = app_handle_for_listener.clone();
    app.listen_any("asr-config-updated", move |_event| {
        let config_for_listener = config_for_listener.clone();
        let app_handle = listener_app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let new_config_map = crate::app::server::load_online_asr_config(
                &app_handle as &dyn crate::core::paths::PathProvider,
            );
            *config_for_listener.write().await = new_config_map;
        });
    });

    crate::app::tray::setup_tray(app)?;

    crate::app::server::start_from_app_handle(&app.handle().clone());

    Ok(())
}
