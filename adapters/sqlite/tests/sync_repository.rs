use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::json;
use sona_core::automation::repository::{
    AutomationRuleRecord, AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig,
    AutomationStore,
};
use sona_core::history::mutation_repository::{
    HistoryMutationRepository, HistoryUpdateTranscriptRequest,
};
use sona_core::ports::time::UnixMillisClock;
use sona_core::project::{ProjectDefaults, ProjectRecord, ProjectStore};
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncConflictResolution, SyncEntityKey, SyncEntityKind,
    SyncLocalRepository, SyncOperation, SyncOperationKind, SyncPresetV1, SyncPublishedSegment,
    SyncRemoteSegment, SyncRepositoryFactory, SyncVersion,
};
use sona_sqlite::{
    Database, DatabaseError, SqliteAppConfigAdapter, SqliteAutomationRepository,
    SqliteHistoryStore, SqliteProjectRepository, SqliteSyncRepository, SqliteSyncRepositoryFactory,
    record_sync_operation_in_transaction,
};

struct FixedClock;

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, sona_core::ports::time::ClockError> {
        Ok(5_000)
    }
}

fn operation(id: &str, device_id: &str, value: &str) -> SyncOperation {
    SyncOperation {
        operation_id: id.to_string(),
        source_device_id: device_id.to_string(),
        source_sequence: 0,
        causal_context: SyncCausalContext {
            observed_sequences: BTreeMap::new(),
        },
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: 100,
                logical: 0,
            },
            device_id: device_id.to_string(),
            operation_id: id.to_string(),
        },
        entity: SyncEntityKey {
            kind: SyncEntityKind::Project,
            id: "project-1".to_string(),
        },
        kind: SyncOperationKind::SetField {
            field: "name".to_string(),
            value: json!(value),
        },
    }
}

fn field_operation(
    id: &str,
    device_id: &str,
    sequence: u64,
    kind: SyncEntityKind,
    entity_id: &str,
    field: &str,
    value: serde_json::Value,
) -> SyncOperation {
    SyncOperation {
        operation_id: id.to_string(),
        source_device_id: device_id.to_string(),
        source_sequence: sequence,
        causal_context: SyncCausalContext::default(),
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: 200,
                logical: 0,
            },
            device_id: device_id.to_string(),
            operation_id: id.to_string(),
        },
        entity: SyncEntityKey {
            kind,
            id: entity_id.to_string(),
        },
        kind: SyncOperationKind::SetField {
            field: field.to_string(),
            value,
        },
    }
}

fn delete_operation(
    id: &str,
    device_id: &str,
    sequence: u64,
    kind: SyncEntityKind,
    entity_id: &str,
) -> SyncOperation {
    SyncOperation {
        operation_id: id.to_string(),
        source_device_id: device_id.to_string(),
        source_sequence: sequence,
        causal_context: SyncCausalContext::default(),
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: 300,
                logical: 0,
            },
            device_id: device_id.to_string(),
            operation_id: id.to_string(),
        },
        entity: SyncEntityKey {
            kind,
            id: entity_id.to_string(),
        },
        kind: SyncOperationKind::DeleteEntity,
    }
}

#[test]
fn schema_v3_creates_all_sync_tables() {
    let db = Database::open_in_memory().unwrap();

    let tables = db
        .with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name LIKE 'sync_%' ORDER BY name",
            )?;
            statement
                .query_map([], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(DatabaseError::QueryError)
        })
        .unwrap();

    assert_eq!(
        tables,
        vec![
            "sync_conflicts",
            "sync_device_cursors",
            "sync_entity_versions",
            "sync_outbox",
            "sync_state",
        ]
    );
}

#[test]
fn local_operation_and_outbox_publish_are_atomic() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    let mut pending = operation("op-a", "device-a", "local");
    pending.entity.kind = SyncEntityKind::Tag;

    repository.record_local_operation(&pending).unwrap();
    assert_eq!(
        repository
            .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
            .unwrap(),
        vec![pending.clone()]
    );

    repository
        .mark_segment_published(&SyncPublishedSegment {
            sequence: 1,
            cipher_hash: "hash-a".to_string(),
            operations: vec![SyncOperation {
                source_sequence: 1,
                ..pending.clone()
            }],
            encrypted_bytes: 512,
        })
        .unwrap();

    assert!(
        repository
            .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
            .unwrap()
            .is_empty()
    );
    let state = repository.load_runtime_state().unwrap();
    assert_eq!(state.next_sequence, 2);
    assert_eq!(state.previous_cipher_hash.as_deref(), Some("hash-a"));
    assert_eq!(state.operations_since_checkpoint, 1);
    assert_eq!(state.bytes_since_checkpoint, 512);
}

#[test]
fn rolled_back_business_transaction_leaves_no_outbox_operation() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    let rolled_back = operation("op-rollback", "device-a", "discarded");

    let result: Result<(), DatabaseError> = db.with_rw_transaction(|transaction| {
        record_sync_operation_in_transaction(transaction, &rolled_back)?;
        Err(DatabaseError::Internal("force rollback".to_string()))
    });

    assert!(result.is_err());
    assert!(
        repository
            .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn remote_segment_updates_cursor_once_and_persists_concurrent_conflict() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    repository
        .record_local_operation(&operation("op-local", "device-a", "local"))
        .unwrap();
    let mut remote = operation("op-remote", "device-b", "remote");
    remote.source_sequence = 1;
    remote.version.device_id = "device-b".to_string();
    let segment = SyncRemoteSegment {
        device_id: "device-b".to_string(),
        sequence: 1,
        cipher_hash: "hash-b".to_string(),
        operations: vec![remote],
    };

    let applied = repository.apply_remote_segment(&segment).unwrap();
    let duplicate = repository.apply_remote_segment(&segment).unwrap();

    assert_eq!(applied.applied_operation_count, 1);
    assert_eq!(applied.conflict_count, 1);
    assert_eq!(duplicate.applied_operation_count, 0);
    assert_eq!(repository.list_unresolved_conflicts().unwrap().len(), 1);
    assert_eq!(
        repository.load_runtime_state().unwrap().remote_cursors["device-b"].sequence,
        1
    );
}

#[test]
fn remote_project_winner_is_projected_without_creating_an_outbox_echo() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO tags (id, name, created_at, updated_at)
             VALUES ('project-1', 'Local', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    repository
        .record_local_operation(&operation("op-local", "device-a", "Local"))
        .unwrap();
    let remote = field_operation(
        "op-remote",
        "device-b",
        1,
        SyncEntityKind::Project,
        "project-1",
        "name",
        json!("Remote"),
    );

    repository
        .apply_remote_segment(&SyncRemoteSegment {
            device_id: "device-b".to_string(),
            sequence: 1,
            cipher_hash: "hash-b".to_string(),
            operations: vec![remote],
        })
        .unwrap();

    let (name, outbox_count) = db
        .with_connection(|connection| {
            Ok((
                connection.query_row(
                    "SELECT name FROM tags WHERE id = 'project-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                connection.query_row("SELECT COUNT(*) FROM sync_outbox", [], |row| {
                    row.get::<_, i64>(0)
                })?,
            ))
        })
        .unwrap();
    assert_eq!(name, "Remote");
    assert_eq!(outbox_count, 1);
}

#[test]
fn remote_document_is_projected_as_a_whole_document() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO history_items (id, timestamp) VALUES ('history-1', 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let document = json!([{"text": "private transcript", "start": 0, "end": 1}]);

    repository
        .apply_remote_segment(&SyncRemoteSegment {
            device_id: "device-b".to_string(),
            sequence: 1,
            cipher_hash: "hash-b".to_string(),
            operations: vec![field_operation(
                "op-transcript",
                "device-b",
                1,
                SyncEntityKind::HistoryTranscript,
                "history-1",
                "document",
                document.clone(),
            )],
        })
        .unwrap();

    let stored = db
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT segments FROM history_transcripts WHERE history_id = 'history-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .map_err(DatabaseError::QueryError)
        })
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored).unwrap(),
        document
    );
}

#[test]
fn excluded_remote_field_rolls_back_the_entire_batch_and_cursor() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO tags (id, name, created_at, updated_at)
             VALUES ('project-1', 'Before', 1, 1)",
            [],
        )?;
        transaction.execute(
            "INSERT INTO history_items (id, timestamp, audio_path)
             VALUES ('history-1', 1, 'local.wav')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let segment = SyncRemoteSegment {
        device_id: "device-b".to_string(),
        sequence: 1,
        cipher_hash: "hash-b".to_string(),
        operations: vec![
            field_operation(
                "op-valid",
                "device-b",
                1,
                SyncEntityKind::Project,
                "project-1",
                "name",
                json!("After"),
            ),
            field_operation(
                "op-path",
                "device-b",
                1,
                SyncEntityKind::HistoryItem,
                "history-1",
                "audioPath",
                json!("remote.wav"),
            ),
        ],
    };

    assert!(repository.apply_remote_segment(&segment).is_err());

    let (name, audio_path, cursor_count) = db
        .with_connection(|connection| {
            Ok((
                connection.query_row(
                    "SELECT name FROM tags WHERE id = 'project-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                connection.query_row(
                    "SELECT audio_path FROM history_items WHERE id = 'history-1'",
                    [],
                    |row| row.get::<_, String>(0),
                )?,
                connection.query_row("SELECT COUNT(*) FROM sync_device_cursors", [], |row| {
                    row.get::<_, i64>(0)
                })?,
            ))
        })
        .unwrap();
    assert_eq!(name, "Before");
    assert_eq!(audio_path, "local.wav");
    assert_eq!(cursor_count, 0);
}

#[test]
fn pending_operations_are_filtered_by_preset_and_portable_field_whitelist() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    for pending in [
        operation("project", "device-a", "project"),
        field_operation(
            "setting",
            "device-a",
            0,
            SyncEntityKind::Setting,
            "app-config::translationLanguage",
            "value",
            json!("ja"),
        ),
        field_operation(
            "automation",
            "device-a",
            0,
            SyncEntityKind::AutomationRule,
            "rule-1",
            "name",
            json!("Rule"),
        ),
        field_operation(
            "excluded-path",
            "device-a",
            0,
            SyncEntityKind::HistoryItem,
            "history-1",
            "audioPath",
            json!("private.wav"),
        ),
    ] {
        repository.record_local_operation(&pending).unwrap();
    }

    let ids = |preset| {
        repository
            .load_pending_operations(preset, 256, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|operation| operation.operation_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids(SyncPresetV1::Content), vec!["project"]);
    assert_eq!(ids(SyncPresetV1::Standard), vec!["project", "setting"]);
    assert_eq!(
        ids(SyncPresetV1::Full),
        vec!["project", "automation", "setting"]
    );
}

#[test]
fn project_repository_writes_business_row_and_outbox_in_one_transaction() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let sync = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    let projects = SqliteProjectRepository::new(Arc::clone(&db));
    let project = ProjectRecord {
        id: "project-captured".to_string(),
        name: "Captured".to_string(),
        description: "Portable".to_string(),
        icon: "folder".to_string(),
        created_at: 1_000,
        updated_at: 1_000,
        defaults: ProjectDefaults {
            summary_template_id: "meeting".to_string(),
            translation_language: "ja".to_string(),
            polish_preset_id: "general".to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: "notes-".to_string(),
            enabled_text_replacement_set_ids: vec!["replace-1".to_string()],
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
        },
    };

    ProjectStore::insert_project(&projects, project.clone()).unwrap();

    let pending = sync
        .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
        .unwrap();
    assert!(pending.iter().any(|operation| {
        operation.entity.id == project.id
            && matches!(
                &operation.kind,
                SyncOperationKind::SetField { field, value }
                    if field == "name" && value == "Captured"
            )
    }));
    assert!(pending.iter().all(|operation| {
        operation.entity.kind == SyncEntityKind::Tag && operation.entity.id == project.id
    }));
}

#[test]
fn join_preview_projects_conflicts_and_rolls_back_every_write() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let projects = SqliteProjectRepository::new(Arc::clone(&db));
    let project = ProjectRecord {
        id: "preview-project".to_string(),
        name: "Local name".to_string(),
        description: "Local description".to_string(),
        icon: "folder".to_string(),
        created_at: 1_000,
        updated_at: 1_000,
        defaults: ProjectDefaults {
            summary_template_id: "general".to_string(),
            translation_language: "zh".to_string(),
            polish_preset_id: "general".to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: String::new(),
            enabled_text_replacement_set_ids: Vec::new(),
            enabled_hotword_set_ids: Vec::new(),
            enabled_polish_keyword_set_ids: Vec::new(),
            enabled_speaker_profile_ids: Vec::new(),
        },
    };
    ProjectStore::insert_project(&projects, project.clone()).unwrap();
    let remote = SyncRemoteSegment {
        device_id: "remote-device".to_string(),
        sequence: 1,
        cipher_hash: "remote-hash".to_string(),
        operations: vec![field_operation(
            "remote-name",
            "remote-device",
            1,
            SyncEntityKind::Project,
            &project.id,
            "name",
            json!("Remote name"),
        )],
    };

    let preview = SqliteSyncRepository::preview_join(
        Arc::clone(&db),
        "preview-vault",
        "preview-device",
        SyncPresetV1::Standard,
        &[remote],
    )
    .unwrap();

    assert!(preview.local_operation_count > 0);
    assert_eq!(preview.remote_operation_count, 1);
    assert_eq!(preview.projected_conflict_count, 1);
    assert_eq!(
        ProjectStore::load_state(&projects).unwrap().projects,
        vec![project]
    );
    db.with_connection(|connection| {
        for table in [
            "sync_state",
            "sync_outbox",
            "sync_entity_versions",
            "sync_conflicts",
        ] {
            let count =
                connection.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get::<_, i64>(0)
                })?;
            assert_eq!(count, 0, "preview left rows in {table}");
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn config_repository_captures_only_portable_redacted_settings() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let sync = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    let config = SqliteAppConfigAdapter::new(Arc::clone(&db), Arc::new(FixedClock));

    config
        .save_config(&json!({
            "configVersion": 7,
            "theme": "dark",
            "microphoneId": "private-microphone",
            "translationLanguage": "ja",
            "llmSettings": {
                "activeProvider": "openai",
                "providers": {
                    "openai": {
                        "apiHost": "https://api.example.com",
                        "apiKey": "llm-secret"
                    }
                },
                "models": {},
                "modelOrder": [],
                "selections": {}
            },
            "asr": {
                "selections": {
                    "batch": {
                        "engine": "local-sherpa",
                        "mode": "batch",
                        "modelPath": "C:\\private\\model"
                    }
                },
                "providers": {
                    "online": {
                        "vendor": { "apiKey": "asr-secret", "language": "ja" }
                    }
                }
            }
        }))
        .unwrap();

    let pending = sync
        .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
        .unwrap();
    let fields = pending
        .iter()
        .filter(|operation| operation.entity.kind == SyncEntityKind::Setting)
        .map(|operation| operation.entity.id.as_str())
        .collect::<Vec<_>>();
    assert!(fields.contains(&"app-config::translationLanguage"));
    assert!(fields.contains(&"app-config::llmSettings"));
    assert!(fields.contains(&"app-config::asr"));
    assert!(!fields.contains(&"app-config::theme"));
    assert!(!fields.contains(&"app-config::microphoneId"));
    let serialized = serde_json::to_string(&pending).unwrap();
    assert!(!serialized.contains("llm-secret"));
    assert!(!serialized.contains("asr-secret"));
    assert!(!serialized.contains("private\\\\model"));
    assert!(!serialized.contains("apiKey"));
    assert!(!serialized.contains("modelPath"));
}

#[test]
fn automation_capture_excludes_paths_enabled_state_and_execution_history() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let sync = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Full,
    )
    .unwrap();
    let automation = SqliteAutomationRepository::new(Arc::clone(&db));
    let rule = AutomationRuleRecord {
        id: "rule-1".to_string(),
        name: "Portable rule".to_string(),
        save_history: true,
        tag_ids: Vec::new(),
        preset_id: "custom".to_string(),
        watch_directory: "C:\\private\\watch".to_string(),
        recursive: true,
        enabled: true,
        stage_config: AutomationRuleRecordStageConfig {
            auto_polish: true,
            polish_preset_id: "general".to_string(),
            auto_translate: true,
            translation_language: "ja".to_string(),
            export_enabled: true,
        },
        export_config: AutomationRuleRecordExportConfig {
            directory: "C:\\private\\export".to_string(),
            format: "srt".to_string(),
            mode: "polished".to_string(),
            prefix: "done-".to_string(),
        },
        created_at: 1_000,
        updated_at: 2_000,
    };

    AutomationStore::replace_rules(&automation, &[rule]).unwrap();

    let pending = sync
        .load_pending_operations(SyncPresetV1::Full, 256, usize::MAX)
        .unwrap();
    let fields = pending
        .iter()
        .filter_map(|operation| operation.kind.field())
        .collect::<Vec<_>>();
    assert!(fields.contains(&"name"));
    assert!(fields.contains(&"saveHistory"));
    assert!(fields.contains(&"tagIds"));
    assert!(fields.contains(&"stageAutoTranslate"));
    assert!(fields.contains(&"exportFormat"));
    assert!(!fields.contains(&"watchDirectory"));
    assert!(!fields.contains(&"exportDirectory"));
    assert!(!fields.contains(&"enabled"));
    let serialized = serde_json::to_string(&pending).unwrap();
    assert!(!serialized.contains("private\\\\watch"));
    assert!(!serialized.contains("private\\\\export"));
}

#[test]
fn transcript_update_captures_a_whole_document_without_audio_fields() {
    let temp = tempfile::tempdir().unwrap();
    let db = Arc::new(Database::open(temp.path()).unwrap());
    let sync = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Content,
    )
    .unwrap();
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO history_items (id, timestamp, audio_path, title)
             VALUES ('history-1', 1, 'private.wav', 'Before')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let history = SqliteHistoryStore::new(temp.path().to_path_buf(), Arc::clone(&db));
    let document: Vec<sona_core::transcription::transcript::TranscriptSegment> =
        serde_json::from_value(json!([{
            "id": "segment-1",
            "text": "synced transcript",
            "start": 0.0,
            "end": 1.0,
            "isFinal": true
        }]))
        .unwrap();

    HistoryMutationRepository::update_transcript(
        &history,
        HistoryUpdateTranscriptRequest {
            history_id: "history-1".to_string(),
            segments: document.clone(),
        },
    )
    .unwrap();

    let pending = sync
        .load_pending_operations(SyncPresetV1::Content, 256, usize::MAX)
        .unwrap();
    assert!(pending.iter().any(|operation| {
        operation.entity.kind == SyncEntityKind::HistoryTranscript
            && matches!(
                &operation.kind,
                SyncOperationKind::SetField { field, value }
                    if field == "document"
                        && value.as_array().is_some_and(|segments| {
                            segments.first().and_then(|segment| segment.get("text"))
                                == Some(&json!("synced transcript"))
                        })
            )
    }));
    assert!(pending.iter().any(|operation| {
        operation.entity.kind == SyncEntityKind::HistoryItem
            && operation.kind.field() == Some("previewText")
    }));
    let serialized = serde_json::to_string(&pending).unwrap();
    assert!(!serialized.contains("private.wav"));
    assert!(!serialized.contains("audioPath"));
}

#[test]
fn entity_tombstone_wins_concurrent_fields_but_allows_a_causally_later_restore() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO tags (id, name, created_at, updated_at)
             VALUES ('project-1', 'Local', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let mut local = operation("op-local", "device-a", "Local edit");
    local.source_sequence = 1;
    repository.record_local_operation(&local).unwrap();

    let deleted = repository
        .apply_remote_segment(&SyncRemoteSegment {
            device_id: "device-b".to_string(),
            sequence: 1,
            cipher_hash: "delete-hash".to_string(),
            operations: vec![delete_operation(
                "op-delete",
                "device-b",
                1,
                SyncEntityKind::Project,
                "project-1",
            )],
        })
        .unwrap();
    assert_eq!(deleted.conflict_count, 1);

    let stale = field_operation(
        "op-stale",
        "device-c",
        1,
        SyncEntityKind::Project,
        "project-1",
        "name",
        json!("Stale resurrection"),
    );
    repository
        .apply_remote_segment(&SyncRemoteSegment {
            device_id: "device-c".to_string(),
            sequence: 1,
            cipher_hash: "stale-hash".to_string(),
            operations: vec![stale],
        })
        .unwrap();
    let exists_after_stale = db
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM tags WHERE id = 'project-1')",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(DatabaseError::QueryError)
        })
        .unwrap();
    assert!(!exists_after_stale);

    let mut restored = field_operation(
        "op-restore",
        "device-c",
        2,
        SyncEntityKind::Project,
        "project-1",
        "name",
        json!("Restored"),
    );
    restored
        .causal_context
        .observed_sequences
        .insert("device-b".to_string(), 1);
    repository
        .apply_remote_segment(&SyncRemoteSegment {
            device_id: "device-c".to_string(),
            sequence: 2,
            cipher_hash: "restore-hash".to_string(),
            operations: vec![restored],
        })
        .unwrap();
    let restored_name = db
        .with_connection(|connection| {
            connection
                .query_row("SELECT name FROM tags WHERE id = 'project-1'", [], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(DatabaseError::QueryError)
        })
        .unwrap();
    assert_eq!(restored_name, "Restored");
}

#[test]
fn conflict_can_be_listed_viewed_and_resolved_with_the_conflicting_value() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO tags (id, name, created_at, updated_at)
             VALUES ('project-1', 'Local', 1, 1)",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    repository
        .record_local_operation(&operation("op-local", "device-a", "Local"))
        .unwrap();
    repository
        .apply_remote_segment(&SyncRemoteSegment {
            device_id: "device-b".to_string(),
            sequence: 1,
            cipher_hash: "hash-b".to_string(),
            operations: vec![field_operation(
                "op-remote",
                "device-b",
                1,
                SyncEntityKind::Project,
                "project-1",
                "name",
                json!("Remote"),
            )],
        })
        .unwrap();

    let summaries = repository.list_conflict_summaries().unwrap();
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].field.as_deref(), Some("name"));
    let detail = repository
        .get_conflict_detail(&summaries[0].conflict_id)
        .unwrap()
        .unwrap();
    assert_eq!(detail.current.operation_id, "op-remote");
    assert_eq!(detail.conflicting.operation_id, "op-local");

    repository
        .resolve_conflict(
            &summaries[0].conflict_id,
            SyncConflictResolution::UseConflicting,
            500,
        )
        .unwrap();

    let name = db
        .with_connection(|connection| {
            connection
                .query_row("SELECT name FROM tags WHERE id = 'project-1'", [], |row| {
                    row.get::<_, String>(0)
                })
                .map_err(DatabaseError::QueryError)
        })
        .unwrap();
    assert_eq!(name, "Local");
    assert!(repository.list_conflict_summaries().unwrap().is_empty());
    let pending = repository
        .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
        .unwrap();
    assert_eq!(pending.len(), 1);
    assert_ne!(pending[0].operation_id, "op-local");
    assert!(pending[0].version.clock > detail.current.version.clock);
}

#[test]
fn initializing_sync_seeds_existing_domain_state_and_requires_an_initial_checkpoint() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO tags (id, name, icon, color, created_at, updated_at)
             VALUES ('existing-project', 'Existing', '', '', 10, 20)",
            [],
        )?;
        Ok(())
    })
    .unwrap();

    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Standard,
    )
    .unwrap();

    let pending = repository
        .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
        .unwrap();
    assert!(pending.iter().any(|operation| {
        operation.entity.id == "existing-project"
            && matches!(
                &operation.kind,
                SyncOperationKind::SetField { field, value }
                    if field == "name" && value == "Existing"
            )
    }));
    assert!(repository.load_runtime_state().unwrap().checkpoint_required);
}

#[test]
fn shrinking_preset_requires_confirmation_and_publishes_removed_domain_tombstones() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    db.with_rw_transaction(|transaction| {
        transaction.execute(
            "INSERT INTO automation_rules (id, name) VALUES ('rule-1', 'Local rule')",
            [],
        )?;
        Ok(())
    })
    .unwrap();
    let repository = SqliteSyncRepository::initialize(
        Arc::clone(&db),
        "vault-a",
        "device-a",
        SyncPresetV1::Full,
    )
    .unwrap();

    assert!(
        repository
            .change_preset(SyncPresetV1::Standard, false)
            .is_err()
    );
    assert_eq!(
        repository.load_runtime_state().unwrap().preset,
        SyncPresetV1::Full
    );

    let tombstone_count = repository
        .change_preset(SyncPresetV1::Standard, true)
        .unwrap();
    assert_eq!(tombstone_count, 1);
    let pending = repository
        .load_pending_operations(SyncPresetV1::Standard, 256, usize::MAX)
        .unwrap();
    assert!(pending.iter().any(|operation| {
        operation.entity.kind == SyncEntityKind::AutomationRule
            && operation.entity.id == "rule-1"
            && matches!(operation.kind, SyncOperationKind::DeleteEntity)
    }));
    let rule_exists = db
        .with_connection(|connection| {
            connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM automation_rules WHERE id = 'rule-1')",
                    [],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(DatabaseError::QueryError)
        })
        .unwrap();
    let state = repository.load_runtime_state().unwrap();
    assert!(rule_exists);
    assert_eq!(state.preset, SyncPresetV1::Standard);
    assert!(state.checkpoint_required);
}

#[test]
fn sync_repository_factory_opens_initializes_previews_and_exposes_application_state() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let factory = SqliteSyncRepositoryFactory::new(Arc::clone(&db));

    assert!(factory.open().unwrap().is_none());
    let preview = factory
        .preview(
            "preview-vault",
            "preview-device",
            SyncPresetV1::Standard,
            &[],
        )
        .unwrap();
    assert_eq!(preview.remote_operation_count, 0);

    let repository = factory
        .initialize("vault-a", "device-a", SyncPresetV1::Standard)
        .unwrap();
    assert_eq!(
        repository
            .runtime_repository()
            .load_runtime_state()
            .unwrap()
            .vault_id,
        "vault-a"
    );
    assert!(!repository.is_paused().unwrap());
    assert_eq!(repository.pending_operation_count().unwrap(), 0);
    assert_eq!(repository.unresolved_conflict_count().unwrap(), 0);
    assert!(repository.list_conflict_summaries().unwrap().is_empty());
    assert!(
        repository
            .get_conflict_detail("missing-conflict")
            .unwrap()
            .is_none()
    );

    repository.set_paused(true).unwrap();
    assert!(repository.is_paused().unwrap());

    let reopened = factory.open().unwrap().expect("initialized repository");
    assert!(reopened.is_paused().unwrap());

    reopened.disconnect().unwrap();
    assert!(factory.open().unwrap().is_none());
}
