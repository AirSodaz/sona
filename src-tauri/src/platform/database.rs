use std::sync::Arc;

use tauri::{AppHandle, Manager, Runtime};

pub fn sqlite_database<R: Runtime>(app: &AppHandle<R>) -> Arc<sona_sqlite::Database> {
    Arc::clone(app.state::<Arc<sona_sqlite::Database>>().inner())
}

pub fn sqlite_config_store<R: Runtime>(
    app: &AppHandle<R>,
) -> sona_sqlite::config_store::SqliteConfigStore {
    sona_sqlite::config_store::SqliteConfigStore::new(sqlite_database(app))
}
