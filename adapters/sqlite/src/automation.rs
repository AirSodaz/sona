use crate::DatabaseError;
use crate::ports::Database as DatabasePort;
use serde_json::Value;
pub use sona_core::automation::repository::AutomationRepositoryState;
use sona_core::automation::repository::{
    AutomationProcessedRecord, AutomationRuleRecord, AutomationRuleRecordExportConfig,
    AutomationRuleRecordStageConfig, AutomationStore,
};
use sona_core::automation::service::{AutomationIdGenerator, AutomationRepositoryService};
use std::sync::Arc;

#[derive(Clone)]
pub struct SqliteAutomationRepository<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteAutomationRepository);

pub struct SqliteAutomationAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    repository: SqliteAutomationRepository<D>,
    ids: Arc<dyn AutomationIdGenerator>,
}

impl<D> SqliteAutomationAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>, ids: Arc<dyn AutomationIdGenerator>) -> Self {
        Self {
            repository: SqliteAutomationRepository::new(db),
            ids,
        }
    }

    pub fn load_state(&self) -> Result<AutomationRepositoryState, String> {
        self.service().load_state()
    }

    pub fn replace_rules_json(&self, rules: Vec<Value>) -> Result<(), String> {
        self.service().replace_rules_json(rules)
    }

    pub fn replace_processed_entries_json(&self, entries: Vec<Value>) -> Result<(), String> {
        self.service().replace_processed_entries_json(entries)
    }

    pub fn replace_state_json(
        &self,
        rules: Vec<Value>,
        processed_entries: Vec<Value>,
    ) -> Result<(), String> {
        self.service().replace_state_json(rules, processed_entries)
    }

    fn service(&self) -> AutomationRepositoryService<'_> {
        AutomationRepositoryService::new(&self.repository, self.ids.as_ref())
    }
}

pub(crate) fn load_automation_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<AutomationRepositoryState, DatabaseError> {
    Ok(AutomationRepositoryState {
        rules: load_rules(tx)?,
        processed_entries: load_processed_entries(tx)?,
    })
}

pub(crate) fn replace_automation_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    state: &AutomationRepositoryState,
) -> Result<(), DatabaseError> {
    delete_automation_in_transaction(tx)?;
    insert_automation_in_transaction(tx, state)
}

pub(crate) fn delete_automation_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM automation_processed", [])?;
    tx.execute("DELETE FROM automation_rules", [])?;
    Ok(())
}

pub(crate) fn insert_automation_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    state: &AutomationRepositoryState,
) -> Result<(), DatabaseError> {
    insert_rules(tx, &state.rules)?;
    insert_processed_entries(tx, &state.processed_entries)
}

fn load_rules(conn: &rusqlite::Connection) -> Result<Vec<AutomationRuleRecord>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, name, project_id, preset_id, watch_directory, recursive, enabled,
                stage_auto_polish, stage_polish_preset_id, stage_auto_translate,
                stage_translation_language, stage_export_enabled,
                export_directory, export_format, export_mode, export_prefix,
                created_at, updated_at
         FROM automation_rules
         ORDER BY id",
    )?;
    let rows = stmt.query_map([], map_row_to_rule_record)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

fn load_processed_entries(
    conn: &rusqlite::Connection,
) -> Result<Vec<AutomationProcessedRecord>, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, rule_id, file_path, source_fingerprint, size, mtime_ms, status,
                processed_at, history_id, export_path, error_message
         FROM automation_processed
         ORDER BY id",
    )?;
    let rows = stmt.query_map([], map_row_to_processed_record)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(DatabaseError::QueryError)
}

impl<D> AutomationStore for SqliteAutomationRepository<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<AutomationRepositoryState, String> {
        self.get_db()
            .and_then(|db| {
                db.with_read_connection(|conn| {
                    let tx = conn
                        .unchecked_transaction()
                        .map_err(DatabaseError::QueryError)?;
                    let state = load_automation_in_transaction(&tx)?;
                    tx.commit().map_err(DatabaseError::QueryError)?;
                    Ok(state)
                })
            })
            .map_err(|error| error.to_string())
    }

    fn replace_rules(&self, rules: &[AutomationRuleRecord]) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute("DELETE FROM automation_rules", [])?;
                    insert_rules(tx, rules)?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())
    }

    fn replace_processed_entries(
        &self,
        entries: &[AutomationProcessedRecord],
    ) -> Result<(), String> {
        self.get_db()
            .and_then(|db| {
                db.with_transaction(|tx| {
                    tx.execute("DELETE FROM automation_processed", [])?;
                    insert_processed_entries(tx, entries)?;
                    Ok(())
                })
            })
            .map_err(|error| error.to_string())
    }

    fn replace_state(&self, state: &AutomationRepositoryState) -> Result<(), String> {
        self.get_db()
            .and_then(|db| db.with_transaction(|tx| replace_automation_in_transaction(tx, state)))
            .map_err(|error| error.to_string())
    }
}

fn insert_rules(
    tx: &rusqlite::Transaction,
    rules: &[AutomationRuleRecord],
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO automation_rules (
            id, name, project_id, preset_id, watch_directory, recursive, enabled,
            stage_auto_polish, stage_polish_preset_id, stage_auto_translate,
            stage_translation_language, stage_export_enabled,
            export_directory, export_format, export_mode, export_prefix,
            created_at, updated_at
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
    )?;
    for rule in rules {
        stmt.execute(rusqlite::params![
            &rule.id,
            &rule.name,
            &rule.project_id,
            &rule.preset_id,
            &rule.watch_directory,
            rule.recursive as i64,
            rule.enabled as i64,
            rule.stage_config.auto_polish as i64,
            &rule.stage_config.polish_preset_id,
            rule.stage_config.auto_translate as i64,
            &rule.stage_config.translation_language,
            rule.stage_config.export_enabled as i64,
            &rule.export_config.directory,
            &rule.export_config.format,
            &rule.export_config.mode,
            &rule.export_config.prefix,
            rule.created_at,
            rule.updated_at,
        ])?;
    }
    Ok(())
}

fn insert_processed_entries(
    tx: &rusqlite::Transaction,
    entries: &[AutomationProcessedRecord],
) -> Result<(), DatabaseError> {
    let mut stmt = tx.prepare_cached(
        "INSERT INTO automation_processed (
            id, rule_id, file_path, source_fingerprint, size, mtime_ms, status,
            processed_at, history_id, export_path, error_message
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
    )?;
    for entry in entries {
        stmt.execute(rusqlite::params![
            &entry.id,
            &entry.rule_id,
            &entry.file_path,
            &entry.source_fingerprint,
            entry.size,
            entry.mtime_ms,
            &entry.status,
            entry.processed_at,
            entry.history_id.as_deref(),
            entry.export_path.as_deref(),
            entry.error_message.as_deref(),
        ])?;
    }
    Ok(())
}

fn map_row_to_rule_record(row: &rusqlite::Row) -> rusqlite::Result<AutomationRuleRecord> {
    Ok(AutomationRuleRecord {
        id: row.get("id")?,
        name: row.get("name")?,
        project_id: row.get("project_id")?,
        preset_id: row.get("preset_id")?,
        watch_directory: row.get("watch_directory")?,
        recursive: row.get::<_, i64>("recursive")? != 0,
        enabled: row.get::<_, i64>("enabled")? != 0,
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: row.get::<_, i64>("stage_auto_polish")? != 0,
            polish_preset_id: row.get("stage_polish_preset_id")?,
            auto_translate: row.get::<_, i64>("stage_auto_translate")? != 0,
            translation_language: row.get("stage_translation_language")?,
            export_enabled: row.get::<_, i64>("stage_export_enabled")? != 0,
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: row.get("export_directory")?,
            format: row.get("export_format")?,
            mode: row.get("export_mode")?,
            prefix: row.get("export_prefix")?,
        },
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

fn map_row_to_processed_record(row: &rusqlite::Row) -> rusqlite::Result<AutomationProcessedRecord> {
    Ok(AutomationProcessedRecord {
        id: row.get("id")?,
        rule_id: row.get("rule_id")?,
        file_path: row.get("file_path")?,
        source_fingerprint: row.get("source_fingerprint")?,
        size: row.get("size")?,
        mtime_ms: row.get("mtime_ms")?,
        status: row.get("status")?,
        processed_at: row.get("processed_at")?,
        history_id: row.get("history_id")?,
        export_path: row.get("export_path")?,
        error_message: row.get("error_message")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use sona_core::automation::repository::{
        AutomationProcessedRecord, AutomationRuleRecord, AutomationRuleRecordExportConfig,
        AutomationRuleRecordStageConfig, AutomationStore,
    };
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;

    struct SequenceIds(Mutex<Vec<String>>);

    impl AutomationIdGenerator for SequenceIds {
        fn generate_id(&self) -> String {
            self.0.lock().unwrap().remove(0)
        }
    }

    fn object<T: serde::de::DeserializeOwned>(fields: &[(&str, &str)]) -> T {
        T::deserialize(serde::de::value::MapDeserializer::<
            _,
            serde::de::value::Error,
        >::new(
            fields
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string())),
        ))
        .unwrap()
    }

    fn rule_record(id: &str, name: &str) -> AutomationRuleRecord {
        AutomationRuleRecord {
            id: id.into(),
            name: name.into(),
            project_id: "project-1".into(),
            preset_id: "preset-1".into(),
            watch_directory: "C:\\watch".into(),
            recursive: true,
            enabled: true,
            stage_config: AutomationRuleRecordStageConfig {
                auto_polish: true,
                polish_preset_id: "polish-1".into(),
                auto_translate: true,
                translation_language: "zh".into(),
                export_enabled: true,
            },
            export_config: AutomationRuleRecordExportConfig {
                directory: "C:\\export".into(),
                format: "srt".into(),
                mode: "polished".into(),
                prefix: "done-".into(),
            },
            created_at: 100,
            updated_at: 200,
        }
    }

    fn processed_record(id: &str, rule_id: &str) -> AutomationProcessedRecord {
        AutomationProcessedRecord {
            id: id.into(),
            rule_id: rule_id.into(),
            file_path: "C:\\watch\\audio.wav".into(),
            source_fingerprint: "fingerprint".into(),
            size: 42,
            mtime_ms: 300,
            status: "complete".into(),
            processed_at: 400,
            history_id: Some("history-1".into()),
            export_path: Some("C:\\export\\audio.srt".into()),
            error_message: Some("previous warning".into()),
        }
    }

    fn repository() -> (Arc<Database>, SqliteAutomationRepository) {
        let db = Arc::new(Database::open_in_memory().unwrap());
        let repo = SqliteAutomationRepository::new(Arc::clone(&db));
        (db, repo)
    }

    #[test]
    fn typed_state_round_trips() {
        let (_, repo) = repository();
        let state = AutomationRepositoryState {
            rules: vec![rule_record("rule-1", "Rule")],
            processed_entries: vec![processed_record("entry-1", "rule-1")],
        };

        AutomationStore::replace_state(&repo, &state).unwrap();

        assert_eq!(AutomationStore::load_state(&repo).unwrap(), state);
    }

    #[test]
    fn typed_state_loads_from_read_only_database() {
        let temp = tempdir().unwrap();
        let state = AutomationRepositoryState {
            rules: vec![rule_record("rule-1", "Rule")],
            processed_entries: vec![processed_record("entry-1", "rule-1")],
        };
        {
            let db = Arc::new(Database::open(temp.path()).unwrap());
            let repo = SqliteAutomationRepository::new(db);
            AutomationStore::replace_state(&repo, &state).unwrap();
        }

        let db = Arc::new(Database::open_read_only(temp.path()).unwrap());
        let repo = SqliteAutomationRepository::new(db);

        assert_eq!(AutomationStore::load_state(&repo).unwrap(), state);
    }

    #[test]
    fn replacing_rules_preserves_processed_entries() {
        let (_, repo) = repository();
        let initial = AutomationRepositoryState {
            rules: vec![rule_record("rule-old", "Old")],
            processed_entries: vec![processed_record("entry-1", "rule-old")],
        };
        AutomationStore::replace_state(&repo, &initial).unwrap();

        AutomationStore::replace_rules(&repo, &[rule_record("rule-new", "New")]).unwrap();

        let state = AutomationStore::load_state(&repo).unwrap();
        assert_eq!(state.rules, vec![rule_record("rule-new", "New")]);
        assert_eq!(state.processed_entries, initial.processed_entries);
    }

    #[test]
    fn replacing_processed_entries_preserves_rules() {
        let (_, repo) = repository();
        let initial = AutomationRepositoryState {
            rules: vec![rule_record("rule-1", "Rule")],
            processed_entries: vec![processed_record("entry-old", "rule-1")],
        };
        AutomationStore::replace_state(&repo, &initial).unwrap();

        let replacement = processed_record("entry-new", "rule-1");
        AutomationStore::replace_processed_entries(&repo, &[replacement.clone()]).unwrap();

        let state = AutomationStore::load_state(&repo).unwrap();
        assert_eq!(state.rules, initial.rules);
        assert_eq!(state.processed_entries, vec![replacement]);
    }

    #[test]
    fn loaded_records_are_ordered_by_id() {
        let (_, repo) = repository();
        AutomationStore::replace_state(
            &repo,
            &AutomationRepositoryState {
                rules: vec![rule_record("rule-z", "Z"), rule_record("rule-a", "A")],
                processed_entries: vec![
                    processed_record("entry-z", "rule-z"),
                    processed_record("entry-a", "rule-a"),
                ],
            },
        )
        .unwrap();

        let state = AutomationStore::load_state(&repo).unwrap();
        assert_eq!(
            state
                .rules
                .iter()
                .map(|r| r.id.as_str())
                .collect::<Vec<_>>(),
            ["rule-a", "rule-z"]
        );
        assert_eq!(
            state
                .processed_entries
                .iter()
                .map(|e| e.id.as_str())
                .collect::<Vec<_>>(),
            ["entry-a", "entry-z"]
        );
    }

    #[test]
    fn optional_columns_and_signed_numbers_round_trip() {
        let (_, repo) = repository();
        let mut rule = rule_record("rule-1", "Rule");
        rule.created_at = -9;
        rule.updated_at = i64::MIN;
        let mut entry = processed_record("entry-1", "rule-1");
        entry.size = -1;
        entry.mtime_ms = i64::MIN;
        entry.processed_at = -400;
        entry.history_id = None;
        entry.export_path = None;
        entry.error_message = None;
        let state = AutomationRepositoryState {
            rules: vec![rule],
            processed_entries: vec![entry],
        };

        AutomationStore::replace_state(&repo, &state).unwrap();

        assert_eq!(AutomationStore::load_state(&repo).unwrap(), state);
    }

    #[test]
    fn full_state_replacement_rolls_back_both_collections() {
        let (db, repo) = repository();
        let initial = AutomationRepositoryState {
            rules: vec![rule_record("rule-old", "Old")],
            processed_entries: vec![processed_record("entry-old", "rule-old")],
        };
        AutomationStore::replace_state(&repo, &initial).unwrap();
        db.with_write_connection(|conn| {
            conn.execute_batch(
                "CREATE TEMP TRIGGER abort_automation_processed_insert
                 BEFORE INSERT ON automation_processed
                 BEGIN SELECT RAISE(ABORT, 'forced processed insert failure'); END;",
            )
            .map_err(DatabaseError::QueryError)
        })
        .unwrap();

        let replacement = AutomationRepositoryState {
            rules: vec![rule_record("rule-new", "New")],
            processed_entries: vec![processed_record("entry-new", "rule-new")],
        };
        assert!(AutomationStore::replace_state(&repo, &replacement).is_err());

        assert_eq!(AutomationStore::load_state(&repo).unwrap(), initial);
    }

    #[test]
    fn automation_adapter_persists_canonical_defaults_and_generated_ids() {
        let (db, _) = repository();
        let ids = SequenceIds(Mutex::new(vec![
            "rule-generated".into(),
            "entry-generated".into(),
        ]));
        let adapter = SqliteAutomationAdapter::new(db, Arc::new(ids));

        adapter
            .replace_state_json(vec![object(&[])], vec![object(&[])])
            .unwrap();

        let state = adapter.load_state().unwrap();
        assert_eq!(
            state.rules,
            vec![AutomationRuleRecord {
                id: "rule-generated".into(),
                name: "".into(),
                project_id: "".into(),
                preset_id: "custom".into(),
                watch_directory: "".into(),
                recursive: false,
                enabled: false,
                stage_config: AutomationRuleRecordStageConfig {
                    auto_polish: false,
                    polish_preset_id: "general".into(),
                    auto_translate: false,
                    translation_language: "en".into(),
                    export_enabled: false,
                },
                export_config: AutomationRuleRecordExportConfig {
                    directory: "".into(),
                    format: "txt".into(),
                    mode: "original".into(),
                    prefix: "".into(),
                },
                created_at: 0,
                updated_at: 0,
            }]
        );
        assert_eq!(state.processed_entries[0].id, "entry-generated");
        assert_eq!(state.processed_entries[0].status, "complete");
    }

    #[test]
    fn service_full_state_persistence_is_atomic() {
        let (db, repo) = repository();
        let initial = AutomationRepositoryState {
            rules: vec![rule_record("rule-old", "Old")],
            processed_entries: vec![processed_record("entry-old", "rule-old")],
        };
        AutomationStore::replace_state(&repo, &initial).unwrap();
        db.with_write_connection(|conn| {
            conn.execute_batch(
                "CREATE TEMP TRIGGER abort_service_processed_insert
                 BEFORE INSERT ON automation_processed
                 BEGIN SELECT RAISE(ABORT, 'forced service failure'); END;",
            )
            .map_err(DatabaseError::QueryError)
        })
        .unwrap();
        let ids = SequenceIds(Mutex::new(Vec::new()));

        let result = AutomationRepositoryService::new(&repo, &ids).replace_state_json(
            vec![object(&[("id", "rule-new"), ("name", "New")])],
            vec![object(&[("id", "entry-new"), ("ruleId", "rule-new")])],
        );

        assert!(result.is_err());
        assert_eq!(AutomationStore::load_state(&repo).unwrap(), initial);
    }
}
