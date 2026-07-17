use std::sync::Arc;

use crate::platform::paths::{PathKind, PathProvider, TauriPathProvider};
use std::path::PathBuf;
use tauri::{AppHandle, Manager, Runtime};

pub fn open_and_migrate_sqlite_for_app<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(Arc<sona_sqlite::Database>, PathBuf), Box<dyn std::error::Error>> {
    let path_provider = TauriPathProvider::from_app(app);
    let app_local_data_dir = path_provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(std::io::Error::other)?;

    let db = Arc::new(sona_sqlite::Database::open(&app_local_data_dir)?);
    sona_sqlite::Database::set_global(Arc::clone(&db))?;

    let migration_result =
        sona_sqlite::legacy_migration::migrate_legacy_to_sqlite(db.as_ref(), &app_local_data_dir)?;
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
            sona_sqlite::legacy_migration::move_legacy_domains_to_backup(
                &app_local_data_dir,
                migration_result.domains,
            )?;
        }
    }

    Ok((db, app_local_data_dir))
}

pub fn sqlite_database<R: Runtime>(app: &AppHandle<R>) -> Arc<sona_sqlite::Database> {
    Arc::clone(app.state::<Arc<sona_sqlite::Database>>().inner())
}
