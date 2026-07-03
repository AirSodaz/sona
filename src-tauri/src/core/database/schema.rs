use super::{Database, DatabaseError};

const CURRENT_SCHEMA_VERSION: i64 = 1;
type MigrationFn = fn(&rusqlite::Transaction) -> Result<(), rusqlite::Error>;

const MIGRATIONS: &[(i64, &str, MigrationFn)] =
    &[(1, "Initial complete SQLite schema", migrate_v1)];

/// Runs pending schema migrations in version order.
///
/// Sona's public SQLite baseline starts after the v0.7.4 JSON storage era, so
/// historical in-development SQLite migrations are intentionally squashed into
/// v1. Future schema changes must be appended as v2, v3, etc.
pub fn run_migrations(db: &Database) -> Result<(), DatabaseError> {
    db.with_transaction(|tx| {
        bootstrap_schema_version(tx)?;
        let applied_version = current_applied_schema_version(tx)?;

        if applied_version > CURRENT_SCHEMA_VERSION {
            return Err(DatabaseError::UnsupportedSchemaVersion {
                found: applied_version,
                current: CURRENT_SCHEMA_VERSION,
            });
        }

        for &(version, _description, migration) in MIGRATIONS {
            if version > applied_version {
                migration(tx)?;
                tx.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [version],
                )?;
            }
        }
        Ok(())
    })
}

fn bootstrap_schema_version(tx: &rusqlite::Transaction) -> Result<(), rusqlite::Error> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_version (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT (datetime('now'))
        );",
    )
}

fn current_applied_schema_version(tx: &rusqlite::Transaction) -> Result<i64, rusqlite::Error> {
    tx.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_version",
        [],
        |row| row.get(0),
    )
}

// ---------------------------------------------------------------------------
// v1: Current schema — all SQLite-backed data domains
// ---------------------------------------------------------------------------
fn migrate_v1(tx: &rusqlite::Transaction) -> Result<(), rusqlite::Error> {
    tx.execute_batch(
        "
        -- History Items (replaces history/index.json)
        CREATE TABLE history_items (
            id TEXT PRIMARY KEY,
            timestamp INTEGER NOT NULL,
            duration REAL NOT NULL DEFAULT 0.0,
            audio_path TEXT NOT NULL DEFAULT '',
            transcript_path TEXT NOT NULL DEFAULT '',
            title TEXT NOT NULL DEFAULT '',
            preview_text TEXT NOT NULL DEFAULT '',
            icon TEXT,
            kind TEXT NOT NULL DEFAULT 'recording',
            search_content TEXT NOT NULL DEFAULT '',
            project_id TEXT,
            status TEXT NOT NULL DEFAULT 'complete',
            draft_source TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX idx_history_items_timestamp ON history_items(timestamp);
        CREATE INDEX idx_history_items_project_id ON history_items(project_id);
        CREATE INDEX idx_history_items_project_timestamp ON history_items(project_id, timestamp DESC);
        CREATE INDEX idx_history_items_kind ON history_items(kind);
        CREATE INDEX idx_history_items_status ON history_items(status);

        -- Transcripts (replaces history/{id}.json files)
        CREATE TABLE history_transcripts (
            history_id TEXT PRIMARY KEY REFERENCES history_items(id) ON DELETE CASCADE,
            segments TEXT NOT NULL DEFAULT '[]'
        );

        -- Summaries (replaces history/{id}.summary.json files)
        CREATE TABLE history_summaries (
            history_id TEXT PRIMARY KEY REFERENCES history_items(id) ON DELETE CASCADE,
            payload TEXT NOT NULL DEFAULT '{}'
        );

        -- Transcript Snapshots (replaces history/versions/{id}/*.json)
        CREATE TABLE transcript_snapshots (
            id TEXT NOT NULL,
            history_id TEXT NOT NULL REFERENCES history_items(id) ON DELETE CASCADE,
            reason TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            segment_count INTEGER NOT NULL DEFAULT 0,
            segments TEXT NOT NULL DEFAULT '[]',
            PRIMARY KEY (history_id, id)
        );

        CREATE INDEX idx_snapshots_history_id ON transcript_snapshots(history_id);

        -- Projects (replaces projects/index.json)
        CREATE TABLE projects (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL DEFAULT '',
            icon TEXT,
            color TEXT,
            sort_order INTEGER NOT NULL DEFAULT 0,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            summary_template_id TEXT DEFAULT 'general',
            translation_language TEXT DEFAULT 'zh',
            polish_preset_id TEXT DEFAULT 'general',
            settings TEXT DEFAULT '{}'
        );

        -- App Settings (replaces settings.json KV pairs except sona-config)
        CREATE TABLE app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- App Config (replaces settings.json sona-config)
        CREATE TABLE app_config (
            id INTEGER PRIMARY KEY CHECK (id = 1),
            config TEXT NOT NULL DEFAULT '{}',
            migrated_version INTEGER NOT NULL DEFAULT 0
        );

        -- Automation Rules (replaces automation/rules.json)
        CREATE TABLE automation_rules (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL DEFAULT '{}'
        );

        -- Automation Processed Entries (replaces automation/processed.json)
        CREATE TABLE automation_processed (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL DEFAULT '{}'
        );

        -- Task Ledger (replaces task-ledger/ledger.json)
        CREATE TABLE task_ledger (
            id TEXT PRIMARY KEY,
            data TEXT NOT NULL DEFAULT '{}',
            version INTEGER NOT NULL DEFAULT 1,
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE analytics.llm_usage (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX analytics.idx_llm_usage_occurred_at ON llm_usage(occurred_at);

        CREATE VIRTUAL TABLE history_items_fts USING fts5(
            title,
            preview_text,
            search_content,
            content='history_items',
            content_rowid='rowid',
            tokenize='trigram'
        );

        CREATE TRIGGER tbl_history_items_ai AFTER INSERT ON history_items BEGIN
          INSERT INTO history_items_fts(rowid, title, preview_text, search_content)
          VALUES (new.rowid, new.title, new.preview_text, new.search_content);
        END;

        CREATE TRIGGER tbl_history_items_ad AFTER DELETE ON history_items BEGIN
          INSERT INTO history_items_fts(history_items_fts, rowid, title, preview_text, search_content)
          VALUES ('delete', old.rowid, old.title, old.preview_text, old.search_content);
        END;

        CREATE TRIGGER tbl_history_items_au AFTER UPDATE OF title, preview_text, search_content ON history_items BEGIN
          INSERT INTO history_items_fts(history_items_fts, rowid, title, preview_text, search_content)
          VALUES ('delete', old.rowid, old.title, old.preview_text, old.search_content);
          INSERT INTO history_items_fts(rowid, title, preview_text, search_content)
          VALUES (new.rowid, new.title, new.preview_text, new.search_content);
        END;"
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;

    fn schema_versions(db: &Database) -> Vec<i64> {
        db.with_connection(|conn| {
            let mut stmt = conn.prepare("SELECT version FROM schema_version ORDER BY version")?;
            stmt.query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(DatabaseError::QueryError)
        })
        .unwrap()
    }

    #[test]
    fn test_migrations_are_idempotent() {
        let db = Database::open_in_memory().unwrap();
        // Migrations already ran during open_in_memory. Running again should be a no-op.
        run_migrations(&db).unwrap();

        assert_eq!(schema_versions(&db), vec![1]);
    }

    #[test]
    fn test_future_schema_version_is_rejected() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute("INSERT INTO schema_version (version) VALUES (2)", [])?;
            Ok(())
        })
        .unwrap();

        let err = run_migrations(&db).unwrap_err();
        assert!(matches!(
            err,
            DatabaseError::UnsupportedSchemaVersion {
                found: 2,
                current: 1
            }
        ));
        assert_eq!(schema_versions(&db), vec![1, 2]);
    }

    #[test]
    fn test_lower_schema_version_records_are_preserved() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute("INSERT INTO schema_version (version) VALUES (0)", [])?;
            Ok(())
        })
        .unwrap();

        run_migrations(&db).unwrap();

        assert_eq!(schema_versions(&db), vec![0, 1]);
    }

    #[test]
    fn test_all_tables_created() {
        let db = Database::open_in_memory().unwrap();
        let expected_tables = [
            "schema_version",
            "history_items",
            "history_transcripts",
            "history_summaries",
            "transcript_snapshots",
            "projects",
            "app_settings",
            "app_config",
            "automation_rules",
            "automation_processed",
            "task_ledger",
            "history_items_fts",
        ];

        db.with_connection(|conn| {
            for table in &expected_tables {
                let count: i64 =
                    conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                        row.get(0)
                    })?;
                assert!(count >= 0, "Table {table} should exist");
            }
            // Verify analytics.llm_usage table exists in attached database
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM analytics.llm_usage", [], |row| {
                    row.get(0)
                })?;
            assert_eq!(count, 0);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_trigram_support() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            conn.execute_batch(
                "CREATE VIRTUAL TABLE test_fts USING fts5(content, tokenize='trigram');
                 INSERT INTO test_fts(content) VALUES ('中华人民共和国');
                 INSERT INTO test_fts(content) VALUES ('Hello World');",
            )?;
            let mut stmt =
                conn.prepare_cached("SELECT content FROM test_fts WHERE test_fts MATCH ?1")?;
            {
                let mut rows = stmt.query(["华人民"])?;
                assert!(rows.next()?.is_some());
            }
            {
                let mut rows2 = stmt.query(["hello"])?;
                assert!(rows2.next()?.is_some());
            }
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn test_fts5_triggers_sync_with_history_items() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            // Verify trigger definition to prevent write amplification
            let trigger_sql: String = conn.query_row(
                "SELECT sql FROM sqlite_master WHERE type='trigger' AND name='tbl_history_items_au'",
                [],
                |row| row.get(0),
            )?;
            assert!(
                trigger_sql.contains("AFTER UPDATE OF title, preview_text, search_content ON history_items"),
                "Trigger tbl_history_items_au must restrict updates to title, preview_text, and search_content to prevent write amplification"
            );

            // 1. Test INSERT sync trigger
            conn.execute(
                "INSERT INTO history_items (id, timestamp, title, preview_text, search_content)
                 VALUES ('test-1', 12345, 'Testing Title One', 'Testing Preview One', 'Testing Search Content One')",
                [],
            )?;

            let count_fts: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts WHERE title MATCH 'Title'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts, 1);

            // 2. Test UPDATE sync trigger with non-FTS field (duration)
            conn.execute(
                "UPDATE history_items SET duration = 45.0 WHERE id = 'test-1'",
                [],
            )?;

            // Verify search still works and content is intact
            let count_fts_after_duration: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts WHERE title MATCH 'Title'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts_after_duration, 1);

            // 3. Test UPDATE sync trigger with FTS field (title)
            conn.execute(
                "UPDATE history_items SET title = 'Updated Title One' WHERE id = 'test-1'",
                [],
            )?;

            let count_fts_updated: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts WHERE title MATCH 'Updated'",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts_updated, 1);

            // 4. Test DELETE sync trigger
            conn.execute(
                "DELETE FROM history_items WHERE id = 'test-1'",
                [],
            )?;

            let count_fts_deleted: i64 = conn.query_row(
                "SELECT COUNT(*) FROM history_items_fts",
                [],
                |row| row.get(0),
            )?;
            assert_eq!(count_fts_deleted, 0);

            Ok(())
        }).unwrap();
    }

    #[test]
    fn test_project_timestamp_index_exists() {
        let db = Database::open_in_memory().unwrap();
        db.with_connection(|conn| {
            let exists: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM sqlite_master
                 WHERE type = 'index'
                   AND name = 'idx_history_items_project_timestamp'",
                [],
                |row| row.get(0),
            )?;
            assert!(exists);
            Ok(())
        })
        .unwrap();
    }
}
