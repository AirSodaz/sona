use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::platform::paths::{PathKind, PathPort, TauriPathProvider};
use crate::platform::startup_dialog::LegacyDatabaseAction;
use tauri::{AppHandle, Manager, Runtime};

pub const DATABASE_FILE_NAMES: [&str; 6] = [
    "sona.db",
    "sona.db-wal",
    "sona.db-shm",
    "sona-analytics.db",
    "sona-analytics.db-wal",
    "sona-analytics.db-shm",
];

pub fn backup_and_reset_database(
    app_local_data_dir: &Path,
    found_version: i64,
) -> Result<PathBuf, std::io::Error> {
    let backups_root = app_local_data_dir.join("backups");
    std::fs::create_dir_all(&backups_root)?;

    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
    let base_name = format!("db_v{found_version}_{timestamp}");
    let mut backup_dir = backups_root.join(&base_name);
    let mut counter = 1;
    while backup_dir.exists() {
        backup_dir = backups_root.join(format!("{base_name}_{counter}"));
        counter += 1;
    }
    std::fs::create_dir_all(&backup_dir)?;

    for file_name in DATABASE_FILE_NAMES {
        let src = app_local_data_dir.join(file_name);
        if src.is_file() {
            let dst = backup_dir.join(file_name);
            std::fs::copy(&src, &dst)?;
        }
    }

    let metadata = serde_json::json!({
        "schemaVersion": found_version,
        "backedUpAt": chrono::Utc::now().to_rfc3339(),
        "reason": "unsupported_legacy_schema_version",
    });
    let metadata_path = backup_dir.join("backup_info.json");
    let _ = std::fs::write(
        metadata_path,
        serde_json::to_string_pretty(&metadata).unwrap_or_default(),
    );

    for file_name in DATABASE_FILE_NAMES {
        let src = app_local_data_dir.join(file_name);
        if src.exists() {
            std::fs::remove_file(&src)?;
        }
    }

    let lock_file = app_local_data_dir.join(".history.lock");
    if lock_file.exists() {
        let _ = std::fs::remove_file(&lock_file);
    }

    Ok(backup_dir)
}

pub fn open_and_migrate_sqlite_for_path_with_prompt<P>(
    app_local_data_dir: &Path,
    mut prompt: P,
) -> Result<Arc<sona_sqlite::Database>, Box<dyn std::error::Error>>
where
    P: FnMut(i64, i64) -> LegacyDatabaseAction,
{
    match sona_sqlite::Database::open(app_local_data_dir) {
        Ok(db) => Ok(Arc::new(db)),
        Err(sona_sqlite::DatabaseError::UnsupportedLegacySchemaVersion { found, minimum }) => {
            log::warn!(
                "Database schema version {found} is below minimum supported version {minimum}"
            );
            let action = prompt(found, minimum);
            match action {
                LegacyDatabaseAction::BackupAndReset => {
                    let backup_dir = backup_and_reset_database(app_local_data_dir, found)?;
                    log::info!(
                        "Database backed up to {} and reset. Reopening fresh database...",
                        backup_dir.display()
                    );
                    let db = sona_sqlite::Database::open(app_local_data_dir)?;
                    Ok(Arc::new(db))
                }
                LegacyDatabaseAction::Exit => {
                    log::info!("User chose to exit on legacy schema version {found} < {minimum}");
                    #[cfg(not(test))]
                    std::process::exit(0);
                    #[cfg(test)]
                    Err(Box::new(
                        sona_sqlite::DatabaseError::UnsupportedLegacySchemaVersion {
                            found,
                            minimum,
                        },
                    ))
                }
            }
        }
        Err(err) => Err(Box::new(err)),
    }
}

pub fn open_and_migrate_sqlite_for_app<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<(Arc<sona_sqlite::Database>, PathBuf), Box<dyn std::error::Error>> {
    let path_provider = TauriPathProvider::from_app(app);
    let app_local_data_dir = path_provider
        .resolve_path(PathKind::AppLocalData)
        .map_err(std::io::Error::other)?;

    let db = open_and_migrate_sqlite_for_path_with_prompt(
        &app_local_data_dir,
        crate::platform::startup_dialog::prompt_legacy_database_migration,
    )?;
    Ok((db, app_local_data_dir))
}

pub fn try_sqlite_application_context<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Arc<sona_sqlite::SqliteApplicationContext>, String> {
    app.try_state::<Arc<sona_sqlite::SqliteApplicationContext>>()
        .map(|s| Arc::clone(s.inner()))
        .ok_or_else(|| "Database application context has not been initialized".to_string())
}

pub fn try_sqlite_database<R: Runtime>(
    app: &AppHandle<R>,
) -> Result<Arc<sona_sqlite::Database>, String> {
    try_sqlite_application_context(app).map(|ctx| ctx.database())
}

pub fn sqlite_application_context<R: Runtime>(
    app: &AppHandle<R>,
) -> Arc<sona_sqlite::SqliteApplicationContext> {
    try_sqlite_application_context(app)
        .expect("Database application context is requested before being managed")
}

pub fn sqlite_database<R: Runtime>(app: &AppHandle<R>) -> Arc<sona_sqlite::Database> {
    sqlite_application_context(app).database()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn create_legacy_v6_database(dir: &Path) {
        let db_path = dir.join("sona.db");
        let conn = Connection::open(db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
             INSERT INTO schema_version (version) VALUES (6);
             CREATE TABLE legacy_test_table (id INTEGER PRIMARY KEY, content TEXT);
             INSERT INTO legacy_test_table (id, content) VALUES (1, 'legacy_data');",
        )
        .unwrap();
    }

    #[test]
    fn test_open_fresh_database_succeeds() {
        let temp = tempfile::tempdir().unwrap();
        let db = open_and_migrate_sqlite_for_path_with_prompt(temp.path(), |_found, _min| {
            panic!("Prompt should not be called for a fresh database");
        })
        .unwrap();

        let version: i64 = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                    [],
                    |row| row.get(0),
                )
                .map_err(sona_sqlite::DatabaseError::QueryError)
            })
            .unwrap();
        assert_eq!(version, 9);
    }

    #[test]
    fn test_legacy_schema_version_exit_action() {
        let temp = tempfile::tempdir().unwrap();
        create_legacy_v6_database(temp.path());

        let mut prompt_called = false;
        let err = open_and_migrate_sqlite_for_path_with_prompt(temp.path(), |found, minimum| {
            prompt_called = true;
            assert_eq!(found, 6);
            assert_eq!(minimum, 7);
            LegacyDatabaseAction::Exit
        })
        .unwrap_err();

        assert!(prompt_called);
        let db_err = err.downcast_ref::<sona_sqlite::DatabaseError>().unwrap();
        assert!(matches!(
            db_err,
            sona_sqlite::DatabaseError::UnsupportedLegacySchemaVersion {
                found: 6,
                minimum: 7,
            }
        ));

        // Verify original legacy db file is preserved
        assert!(temp.path().join("sona.db").exists());
        let conn = Connection::open(temp.path().join("sona.db")).unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM legacy_test_table WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content, "legacy_data");
    }

    #[test]
    fn test_legacy_schema_version_backup_and_reset_action() {
        let temp = tempfile::tempdir().unwrap();
        create_legacy_v6_database(temp.path());

        // Also create a dummy lock file and real analytics sqlite file
        std::fs::write(temp.path().join(".history.lock"), b"lock").unwrap();
        drop(Connection::open(temp.path().join("sona-analytics.db")).unwrap());
        let mut prompt_called = false;
        let db = open_and_migrate_sqlite_for_path_with_prompt(temp.path(), |found, minimum| {
            prompt_called = true;
            assert_eq!(found, 6);
            assert_eq!(minimum, 7);
            LegacyDatabaseAction::BackupAndReset
        })
        .unwrap();

        assert!(prompt_called);

        // The new database should be at schema version 8
        let version: i64 = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                    [],
                    |row| row.get(0),
                )
                .map_err(sona_sqlite::DatabaseError::QueryError)
            })
            .unwrap();
        assert_eq!(version, 9);

        // The legacy table should not exist in the new database
        let table_exists: bool = db
            .with_connection(|conn| {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='legacy_test_table'",
                    [],
                    |row| row.get(0),
                )?;
                Ok(count > 0)
            })
            .unwrap();
        assert!(!table_exists);

        // Check backups directory
        let backups_root = temp.path().join("backups");
        assert!(backups_root.is_dir());
        let mut entries = std::fs::read_dir(&backups_root)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect::<Vec<_>>();
        assert_eq!(entries.len(), 1);

        let backup_dir = entries.pop().unwrap();
        let dir_name = backup_dir.file_name().unwrap().to_string_lossy();
        assert!(dir_name.starts_with("db_v6_"));

        // Verify backed up files
        assert!(backup_dir.join("sona.db").exists());
        assert!(backup_dir.join("sona-analytics.db").exists());
        assert!(backup_dir.join("backup_info.json").exists());

        let info_str = std::fs::read_to_string(backup_dir.join("backup_info.json")).unwrap();
        let info: serde_json::Value = serde_json::from_str(&info_str).unwrap();
        assert_eq!(info["schemaVersion"], 6);
        assert_eq!(info["reason"], "unsupported_legacy_schema_version");

        // Verify backed up database still has original legacy data
        let conn = Connection::open(backup_dir.join("sona.db")).unwrap();
        let content: String = conn
            .query_row(
                "SELECT content FROM legacy_test_table WHERE id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(content, "legacy_data");
    }

    #[test]
    fn test_v7_database_is_auto_migrated_to_v8() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("sona.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_version (version INTEGER PRIMARY KEY);
             INSERT INTO schema_version (version) VALUES (7);
             CREATE TABLE tags (
                 id TEXT PRIMARY KEY,
                 name TEXT NOT NULL,
                 description TEXT NOT NULL DEFAULT '',
                 icon TEXT,
                 color TEXT,
                 sort_order INTEGER NOT NULL DEFAULT 0,
                 created_at INTEGER NOT NULL DEFAULT 0,
                 updated_at INTEGER NOT NULL DEFAULT 0
             );
             INSERT INTO tags (id, name) VALUES ('proj-a', 'Project Alpha');
             CREATE TABLE history_items (
                 id TEXT PRIMARY KEY,
                 timestamp INTEGER NOT NULL,
                 duration REAL NOT NULL DEFAULT 0.0,
                 title TEXT NOT NULL DEFAULT ''
             );
             INSERT INTO history_items (id, timestamp, title) VALUES ('item-1', 12345, 'Sample');
             CREATE TABLE history_item_tags (
                 history_id TEXT NOT NULL,
                 tag_id TEXT NOT NULL,
                 PRIMARY KEY (history_id, tag_id)
             );
             INSERT INTO history_item_tags (history_id, tag_id) VALUES ('item-1', 'proj-a');",
        )
        .unwrap();
        drop(conn);

        let db = open_and_migrate_sqlite_for_path_with_prompt(temp.path(), |_found, _min| {
            panic!("Prompt must NOT be called for a v7 database since it can be migrated!");
        })
        .unwrap();

        // The database should be automatically migrated to version 8
        let version: i64 = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT COALESCE(MAX(version), 0) FROM schema_version",
                    [],
                    |row| row.get(0),
                )
                .map_err(sona_sqlite::DatabaseError::QueryError)
            })
            .unwrap();
        assert_eq!(version, 9);

        // project_pipelines table should now exist
        let pipeline_exists: bool = db
            .with_connection(|conn| {
                let count: i64 = conn.query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='project_pipelines'",
                    [],
                    |row| row.get(0),
                ).map_err(sona_sqlite::DatabaseError::QueryError)?;
                Ok(count > 0)
            })
            .unwrap();
        assert!(pipeline_exists);

        // history_items.project_id should be backfilled from history_item_tags
        let project_id: String = db
            .with_connection(|conn| {
                conn.query_row(
                    "SELECT project_id FROM history_items WHERE id = 'item-1'",
                    [],
                    |row| row.get(0),
                )
                .map_err(sona_sqlite::DatabaseError::QueryError)
            })
            .unwrap();
        assert_eq!(project_id, "proj-a");
    }

    #[test]
    fn test_backup_and_reset_database_copies_and_removes_files() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();

        std::fs::write(root.join("sona.db"), b"main_db").unwrap();
        std::fs::write(root.join("sona.db-wal"), b"main_wal").unwrap();
        std::fs::write(root.join("sona.db-shm"), b"main_shm").unwrap();
        std::fs::write(root.join("sona-analytics.db"), b"analytics_db").unwrap();
        std::fs::write(root.join("sona-analytics.db-wal"), b"analytics_wal").unwrap();
        std::fs::write(root.join("sona-analytics.db-shm"), b"analytics_shm").unwrap();
        std::fs::write(root.join(".history.lock"), b"lock").unwrap();
        std::fs::write(root.join("sync.json"), b"keep_this").unwrap();

        let backup_dir = backup_and_reset_database(root, 7).unwrap();

        // In backup dir
        assert_eq!(
            std::fs::read(backup_dir.join("sona.db")).unwrap(),
            b"main_db"
        );
        assert_eq!(
            std::fs::read(backup_dir.join("sona.db-wal")).unwrap(),
            b"main_wal"
        );
        assert_eq!(
            std::fs::read(backup_dir.join("sona.db-shm")).unwrap(),
            b"main_shm"
        );
        assert_eq!(
            std::fs::read(backup_dir.join("sona-analytics.db")).unwrap(),
            b"analytics_db"
        );
        assert_eq!(
            std::fs::read(backup_dir.join("sona-analytics.db-wal")).unwrap(),
            b"analytics_wal"
        );
        assert_eq!(
            std::fs::read(backup_dir.join("sona-analytics.db-shm")).unwrap(),
            b"analytics_shm"
        );

        // Original database files and lock should be removed from root
        assert!(!root.join("sona.db").exists());
        assert!(!root.join("sona.db-wal").exists());
        assert!(!root.join("sona.db-shm").exists());
        assert!(!root.join("sona-analytics.db").exists());
        assert!(!root.join("sona-analytics.db-wal").exists());
        assert!(!root.join("sona-analytics.db-shm").exists());
        assert!(!root.join(".history.lock").exists());

        // Non-database file sync.json must be preserved!
        assert!(root.join("sync.json").exists());
        assert_eq!(std::fs::read(root.join("sync.json")).unwrap(), b"keep_this");
    }
}
