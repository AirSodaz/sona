use std::sync::Arc;

use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
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
    Ok((db, app_local_data_dir))
}

pub fn sqlite_database<R: Runtime>(app: &AppHandle<R>) -> Arc<sona_sqlite::Database> {
    sqlite_application_context(app).database()
}

pub fn sqlite_application_context<R: Runtime>(
    app: &AppHandle<R>,
) -> Arc<sona_sqlite::SqliteApplicationContext> {
    Arc::clone(
        app.state::<Arc<sona_sqlite::SqliteApplicationContext>>()
            .inner(),
    )
}
