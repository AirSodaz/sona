use crate::platform::database::DesktopSqliteState;

pub fn record_usage(
    sqlite: &DesktopSqliteState,
    record: &sona_core::llm::usage::UsageRecord,
) -> Result<(), String> {
    let db = sqlite.current_database()?;
    sona_sqlite::llm_usage::record_usage(db.as_ref(), record).map_err(|error| error.to_string())
}

pub fn read_raw(sqlite: &DesktopSqliteState) -> Result<String, String> {
    let db = sqlite.current_database()?;
    sona_sqlite::llm_usage::read_raw(db.as_ref()).map_err(|error| error.to_string())
}

pub fn replace_raw(sqlite: &DesktopSqliteState, content: String) -> Result<(), String> {
    let db = sqlite.current_database()?;
    sona_sqlite::llm_usage::replace_raw(db.as_ref(), &content).map_err(|error| error.to_string())
}
