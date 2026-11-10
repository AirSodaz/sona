use tauri::{AppHandle, Runtime};

pub fn record_usage(
    db: &sona_sqlite::Database,
    record: &sona_core::llm::usage::UsageRecord,
) -> Result<(), String> {
    sona_sqlite::llm_usage::record_usage(db, record).map_err(|error| error.to_string())
}

pub fn record_usage_for_app<R: Runtime>(
    app: &AppHandle<R>,
    record: &sona_core::llm::usage::UsageRecord,
) -> Result<(), String> {
    let db = crate::platform::database::try_sqlite_database(app)?;
    record_usage(db.as_ref(), record)
}

pub fn read_raw(db: &sona_sqlite::Database) -> Result<String, String> {
    sona_sqlite::llm_usage::read_raw(db).map_err(|error| error.to_string())
}

pub fn read_raw_for_app<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let db = crate::platform::database::try_sqlite_database(app)?;
    read_raw(db.as_ref())
}

pub fn replace_raw(db: &sona_sqlite::Database, content: String) -> Result<(), String> {
    sona_sqlite::llm_usage::replace_raw(db, &content).map_err(|error| error.to_string())
}

pub fn replace_raw_for_app<R: Runtime>(app: &AppHandle<R>, content: String) -> Result<(), String> {
    let db = crate::platform::database::try_sqlite_database(app)?;
    replace_raw(db.as_ref(), content)
}
