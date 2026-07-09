use tauri::{AppHandle, Runtime};

pub fn read_raw<R: Runtime>(app: &AppHandle<R>) -> Result<String, String> {
    let db = crate::platform::database::sqlite_database(app);
    sona_sqlite::llm_usage::read_raw(db.as_ref()).map_err(|error| error.to_string())
}

pub fn replace_raw<R: Runtime>(app: &AppHandle<R>, content: String) -> Result<(), String> {
    let db = crate::platform::database::sqlite_database(app);
    sona_sqlite::llm_usage::replace_raw(db.as_ref(), &content).map_err(|error| error.to_string())
}
