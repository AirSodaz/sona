use std::sync::Arc;
use tauri::{Listener, Manager};

use crate::core::paths::{PathProvider, TauriPathProvider};

pub fn init(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let app_handle_for_listener = app.handle().clone();
    let controller = app_handle_for_listener.state::<crate::app::server::ApiServerController>();

    let path_provider = TauriPathProvider::from_app(&app_handle_for_listener);
    let app_local_data_dir = path_provider
        .resolve_path(crate::core::paths::PathKind::AppLocalData)
        .expect("Failed to get app_local_data_dir");

    // Initialize the SQLite database
    let db = Arc::new(crate::core::database::Database::open(&app_local_data_dir)?);
    crate::core::database::Database::set_global(Arc::clone(&db))?;

    // Migrate legacy JSON data to SQLite
    let migration_result = crate::core::database::legacy_migration::migrate_legacy_to_sqlite(
        db.as_ref(),
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
            crate::core::database::legacy_migration::move_legacy_domains_to_backup(
                &app_local_data_dir,
                migration_result.domains,
            )?;
        }
    }

    let history_repo = Arc::new(crate::repositories::history::SqliteHistoryStore::new(
        app_local_data_dir.clone(),
        Arc::clone(&db),
    ));
    let project_repo = Arc::new(crate::repositories::project::SqliteProjectRepository::new(
        Arc::clone(&db),
    ));
    let analytics_repo =
        Arc::new(crate::repositories::analytics::SqliteAnalyticsRepository::new(Arc::clone(&db)));

    let dashboard_service = Arc::new(crate::app::dashboard::AppDashboardService::new(
        history_repo,
        project_repo,
        analytics_repo,
    ));

    app.manage(dashboard_service);
    app.manage(db);

    let config_for_listener = controller.online_asr_config.clone();
    let listener_app_handle = app_handle_for_listener.clone();
    app.listen_any("asr-config-updated", move |_event| {
        let config_for_listener = config_for_listener.clone();
        let app_handle = listener_app_handle.clone();
        tauri::async_runtime::spawn(async move {
            let path_provider = crate::core::paths::TauriPathProvider::from_app(&app_handle);
            let new_config_map = crate::app::server::load_online_asr_config(&path_provider);
            *config_for_listener.write().await = new_config_map;
        });
    });

    crate::app::tray::setup_tray(app)?;

    crate::app::server::start_from_app_handle(&app.handle().clone());

    Ok(())
}
