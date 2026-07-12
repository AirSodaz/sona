use rusqlite::{Connection, Transaction};

use crate::DatabaseError;

pub(super) fn load(conn: &Connection, key: &str) -> Result<Option<String>, DatabaseError> {
    let mut statement = conn.prepare_cached("SELECT value FROM app_settings WHERE key = ?1")?;
    let mut rows = statement.query([key])?;
    Ok(rows.next()?.map(|row| row.get(0)).transpose()?)
}

pub(super) fn set(tx: &Transaction<'_>, key: &str, value_json: &str) -> Result<(), DatabaseError> {
    tx.execute(
        "INSERT INTO app_settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value_json],
    )?;
    Ok(())
}
