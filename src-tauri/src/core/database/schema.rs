use super::Database;

type MigrationFn = fn(&rusqlite::Transaction) -> Result<(), rusqlite::Error>;

/// Ordered list of schema migrations. Each entry is (version, description, fn).
/// New migrations MUST be appended at the end with an incremented version number.
const MIGRATIONS: &[(i64, &str, MigrationFn)] = &[
    (1, "Initial schema", migrate_v1),
    (2, "LLM usage analytics table", migrate_v2),
    (3, "Add FTS search table and triggers", migrate_v3),
];

/// Runs all pending migrations against the database. Migrations that have
/// already been applied (tracked in the `schema_version` table) are skipped.
pub fn run_migrations(db: &Database) -> Result<(), String> {
    // Ensure the version-tracking table exists (idempotent).
    db.with_connection(|conn| {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS schema_version (
                version INTEGER PRIMARY KEY,
                applied_at TEXT NOT NULL DEFAULT (datetime('now'))
            );",
        )?;
        Ok(())
    })?;

    for &(version, description, migrate_fn) in MIGRATIONS {
        let already_applied = db.with_connection(|conn| {
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM schema_version WHERE version = ?1",
                [version],
                |row| row.get(0),
            )?;
            Ok(count > 0)
        })?;

        if !already_applied {
            log::info!(
                "[Database] Applying migration v{}: {}",
                version,
                description
            );
            db.with_transaction(|tx| {
                migrate_fn(tx)?;
                tx.execute(
                    "INSERT INTO schema_version (version) VALUES (?1)",
                    [version],
                )?;
                Ok(())
            })?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// v1: Initial schema — all tables for the six data domains
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
        CREATE INDEX idx_history_items_kind ON history_items(kind);
        CREATE INDEX idx_history_items_status ON history_items(status);

        -- Transcripts (replaces per-item transcript.json files)
        CREATE TABLE history_transcripts (
            history_id TEXT PRIMARY KEY REFERENCES history_items(id) ON DELETE CASCADE,
            segments TEXT NOT NULL DEFAULT '[]'
        );

        -- Summaries (replaces per-item summary.json files)
        CREATE TABLE history_summaries (
            history_id TEXT PRIMARY KEY REFERENCES history_items(id) ON DELETE CASCADE,
            payload TEXT NOT NULL DEFAULT '{}'
        );

        -- Transcript Snapshots (replaces history/{id}/versions/*.json)
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

        -- App Settings (replaces projects/settings.json KV pairs)
        CREATE TABLE app_settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- App Config (replaces config migration managed global config)
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
        ",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// v2: LLM usage analytics table
// ---------------------------------------------------------------------------
fn migrate_v2(tx: &rusqlite::Transaction) -> Result<(), rusqlite::Error> {
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS analytics.llm_usage (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            occurred_at TEXT NOT NULL,
            provider TEXT NOT NULL DEFAULT '',
            category TEXT NOT NULL DEFAULT '',
            prompt_tokens INTEGER NOT NULL DEFAULT 0,
            completion_tokens INTEGER NOT NULL DEFAULT 0,
            total_tokens INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS analytics.idx_llm_usage_occurred_at ON llm_usage(occurred_at);",
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// v3: Add FTS search table and triggers
// ---------------------------------------------------------------------------
fn migrate_v3(tx: &rusqlite::Transaction) -> Result<(), rusqlite::Error> {
    tx.execute_batch(
        "CREATE VIRTUAL TABLE history_items_fts USING fts5(
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
        END;

        INSERT INTO history_items_fts(rowid, title, preview_text, search_content)
        SELECT rowid, title, preview_text, search_content FROM history_items;"
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::database::Database;

    #[test]
    fn test_migrations_are_idempotent() {
        let db = Database::open_in_memory().unwrap();
        // Migrations already ran during open_in_memory. Running again should be a no-op.
        run_migrations(&db).unwrap();

        db.with_connection(|conn| {
            let count: i64 =
                conn.query_row("SELECT COUNT(*) FROM schema_version", [], |row| row.get(0))?;
            assert_eq!(count, 3); // v1 + v2 + v3
            Ok(())
        })
        .unwrap();
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
}
