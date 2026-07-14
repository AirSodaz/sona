use std::sync::Arc;

use sona_core::config::{
    AppConfigLibrary, AppConfigRepositoryService, AppConfigStartupProjection, AppConfigStore,
    AppConfigStoredState, HotwordRuleRecord, HotwordSetRecord, PolishKeywordSetRecord,
    PolishPresetRecord, SpeakerProfileRecord, SpeakerProfileSampleRecord, SummaryTemplateRecord,
    TextReplacementRuleRecord, TextReplacementSetRecord,
};
use sona_core::ports::time::UnixMillisClock;
use sona_sqlite::{Database, DatabaseError, SqliteConfigStore};

struct FixedClock;

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(1)
    }
}

fn state(label: &str, updated_at: i64) -> AppConfigStoredState {
    AppConfigStoredState {
        base_config_json: format!(r#"{{ "label" : "{label}", "nested": [1, 2] }}"#),
        library: AppConfigLibrary {
            summary_templates: vec![
                SummaryTemplateRecord {
                    id: format!("{label}-summary-b"),
                    name: "Summary B".into(),
                    instructions: "Second".into(),
                },
                SummaryTemplateRecord {
                    id: format!("{label}-summary-a"),
                    name: "Summary A".into(),
                    instructions: "First".into(),
                },
            ],
            polish_presets: vec![
                PolishPresetRecord {
                    id: format!("{label}-polish-b"),
                    name: "Polish B".into(),
                    context: "Second context".into(),
                },
                PolishPresetRecord {
                    id: format!("{label}-polish-a"),
                    name: "Polish A".into(),
                    context: "First context".into(),
                },
            ],
            text_replacement_sets: vec![
                TextReplacementSetRecord {
                    id: format!("{label}-replacement-b"),
                    name: "Replacement B".into(),
                    enabled: true,
                    ignore_case: true,
                    rules: vec![
                        TextReplacementRuleRecord {
                            id: format!("{label}-replace-b2"),
                            from: "B2".into(),
                            to: "Two B".into(),
                        },
                        TextReplacementRuleRecord {
                            id: format!("{label}-replace-b1"),
                            from: "B1".into(),
                            to: "One B".into(),
                        },
                    ],
                },
                TextReplacementSetRecord {
                    id: format!("{label}-replacement-a"),
                    name: "Replacement A".into(),
                    enabled: false,
                    ignore_case: false,
                    rules: vec![
                        TextReplacementRuleRecord {
                            id: format!("{label}-replace-a2"),
                            from: "A2".into(),
                            to: "Two A".into(),
                        },
                        TextReplacementRuleRecord {
                            id: format!("{label}-replace-a1"),
                            from: "A1".into(),
                            to: "One A".into(),
                        },
                    ],
                },
            ],
            hotword_sets: vec![
                HotwordSetRecord {
                    id: format!("{label}-hotwords-b"),
                    name: "Hotwords B".into(),
                    enabled: false,
                    rules: vec![
                        HotwordRuleRecord {
                            id: format!("{label}-hotword-b2"),
                            text: "Sona B2".into(),
                        },
                        HotwordRuleRecord {
                            id: format!("{label}-hotword-b1"),
                            text: "Sona B1".into(),
                        },
                    ],
                },
                HotwordSetRecord {
                    id: format!("{label}-hotwords-a"),
                    name: "Hotwords A".into(),
                    enabled: true,
                    rules: vec![
                        HotwordRuleRecord {
                            id: format!("{label}-hotword-a2"),
                            text: "Sona A2".into(),
                        },
                        HotwordRuleRecord {
                            id: format!("{label}-hotword-a1"),
                            text: "Sona A1".into(),
                        },
                    ],
                },
            ],
            polish_keyword_sets: vec![
                PolishKeywordSetRecord {
                    id: format!("{label}-keywords-b"),
                    name: "Keywords B".into(),
                    enabled: true,
                    keywords: "clear, concise".into(),
                },
                PolishKeywordSetRecord {
                    id: format!("{label}-keywords-a"),
                    name: "Keywords A".into(),
                    enabled: false,
                    keywords: "direct, exact".into(),
                },
            ],
            speaker_profiles: vec![
                SpeakerProfileRecord {
                    id: format!("{label}-speaker-b"),
                    name: "Speaker B".into(),
                    enabled: true,
                    samples: vec![
                        SpeakerProfileSampleRecord {
                            id: format!("{label}-sample-b2"),
                            file_path: "profiles/b2.wav".into(),
                            source_name: "Sample B2".into(),
                            duration_seconds: 12.5,
                        },
                        SpeakerProfileSampleRecord {
                            id: format!("{label}-sample-b1"),
                            file_path: "profiles/b1.wav".into(),
                            source_name: "Sample B1".into(),
                            duration_seconds: 3.25,
                        },
                    ],
                },
                SpeakerProfileRecord {
                    id: format!("{label}-speaker-a"),
                    name: "Speaker A".into(),
                    enabled: false,
                    samples: vec![
                        SpeakerProfileSampleRecord {
                            id: format!("{label}-sample-a2"),
                            file_path: "profiles/a2.wav".into(),
                            source_name: "Sample A2".into(),
                            duration_seconds: 8.75,
                        },
                        SpeakerProfileSampleRecord {
                            id: format!("{label}-sample-a1"),
                            file_path: "profiles/a1.wav".into(),
                            source_name: "Sample A1".into(),
                            duration_seconds: 1.5,
                        },
                    ],
                },
            ],
        },
        config_version: 7,
        updated_at,
        startup_projection: AppConfigStartupProjection {
            http_server_enabled: true,
            host: "0.0.0.0".into(),
            port: 15555,
            api_key: "secret".into(),
            max_concurrent: 4,
            max_queue_size: 32,
            max_upload_size_mb: 128,
            job_ttl_minutes: 15,
            max_streaming: 6,
            ip_whitelist: "127.0.0.1/32".into(),
            gpu_acceleration: "cpu".into(),
        },
    }
}

#[test]
fn independent_sql_seed_loads_exact_typed_state_and_ordered_rows() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    db.with_write_connection(|conn| {
        conn.execute_batch(
            r#"
            INSERT INTO app_config (
                id, config, config_version, updated_at, http_server_enabled,
                http_server_host, http_server_port, http_server_api_key,
                http_server_max_concurrent, http_server_max_queue_size,
                http_server_max_upload_size_mb, http_server_job_ttl_minutes,
                http_server_max_streaming, http_server_ip_whitelist, gpu_acceleration
            ) VALUES (
                1, '{ "label" : "seed", "nested": [1, 2] }', 7, 123, 1,
                '0.0.0.0', 15555, 'secret', 4, 32, 128, 15, 6,
                '127.0.0.1/32', 'cpu'
            );

            INSERT INTO summary_templates VALUES
                ('seed-summary-a', 'Summary A', 'First', 1, 123, 123),
                ('seed-summary-b', 'Summary B', 'Second', 0, 123, 123);
            INSERT INTO polish_presets VALUES
                ('seed-polish-a', 'Polish A', 'First context', 1, 123, 123),
                ('seed-polish-b', 'Polish B', 'Second context', 0, 123, 123);

            INSERT INTO vocabulary_sets VALUES
                ('seed-replacement-a', 'text_replacement', 'Replacement A', 0, 0, '', 1, 123, 123),
                ('seed-replacement-b', 'text_replacement', 'Replacement B', 1, 1, '', 0, 123, 123),
                ('seed-hotwords-a', 'hotword', 'Hotwords A', 1, 0, '', 1, 123, 123),
                ('seed-hotwords-b', 'hotword', 'Hotwords B', 0, 0, '', 0, 123, 123),
                ('seed-keywords-a', 'polish_keyword', 'Keywords A', 0, 0, 'direct, exact', 1, 123, 123),
                ('seed-keywords-b', 'polish_keyword', 'Keywords B', 1, 0, 'clear, concise', 0, 123, 123);
            INSERT INTO vocabulary_rules VALUES
                ('seed-replace-b1', 'text_replacement', 'seed-replacement-b', 'B1', 'One B', '', 1),
                ('seed-replace-b2', 'text_replacement', 'seed-replacement-b', 'B2', 'Two B', '', 0),
                ('seed-replace-a1', 'text_replacement', 'seed-replacement-a', 'A1', 'One A', '', 1),
                ('seed-replace-a2', 'text_replacement', 'seed-replacement-a', 'A2', 'Two A', '', 0),
                ('seed-hotword-b1', 'hotword', 'seed-hotwords-b', '', '', 'Sona B1', 1),
                ('seed-hotword-b2', 'hotword', 'seed-hotwords-b', '', '', 'Sona B2', 0),
                ('seed-hotword-a1', 'hotword', 'seed-hotwords-a', '', '', 'Sona A1', 1),
                ('seed-hotword-a2', 'hotword', 'seed-hotwords-a', '', '', 'Sona A2', 0);

            INSERT INTO speaker_profiles VALUES
                ('seed-speaker-a', 'Speaker A', 0, 1, 123, 123),
                ('seed-speaker-b', 'Speaker B', 1, 0, 123, 123);
            INSERT INTO speaker_profile_samples VALUES
                ('seed-sample-b1', 'seed-speaker-b', 'profiles/b1.wav', 'Sample B1', 3.25, 1),
                ('seed-sample-b2', 'seed-speaker-b', 'profiles/b2.wav', 'Sample B2', 12.5, 0),
                ('seed-sample-a1', 'seed-speaker-a', 'profiles/a1.wav', 'Sample A1', 1.5, 1),
                ('seed-sample-a2', 'seed-speaker-a', 'profiles/a2.wav', 'Sample A2', 8.75, 0);
            "#,
        )?;
        Ok(())
    })
    .unwrap();

    let actual = AppConfigStore::load_state(&SqliteConfigStore::new(db)).unwrap();

    assert_eq!(actual, Some(state("seed", 123)));
}

#[test]
fn typed_replace_state_writes_exact_raw_sql_rows_and_columns() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let store = SqliteConfigStore::new(Arc::clone(&db));
    let expected = state("typed", 1_725_000_000_123);

    AppConfigStore::replace_state(&store, expected.clone()).unwrap();

    assert_eq!(AppConfigStore::load_state(&store).unwrap(), Some(expected));
    db.with_connection(|conn| {
        let row = conn.query_row(
            "SELECT config, config_version, updated_at, http_server_enabled,
                    http_server_host, http_server_port, http_server_api_key,
                    http_server_max_concurrent, http_server_max_queue_size,
                    http_server_max_upload_size_mb, http_server_job_ttl_minutes,
                    http_server_max_streaming, http_server_ip_whitelist, gpu_acceleration
             FROM app_config WHERE id = 1",
            [],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                    row.get::<_, i64>(10)?,
                    row.get::<_, i64>(11)?,
                    row.get::<_, String>(12)?,
                    row.get::<_, String>(13)?,
                ))
            },
        )?;
        assert_eq!(row.0, r#"{ "label" : "typed", "nested": [1, 2] }"#);
        assert_eq!(
            (
                row.1, row.2, row.3, row.5, row.7, row.8, row.9, row.10, row.11
            ),
            (7, 1_725_000_000_123, 1, 15555, 4, 32, 128, 15, 6)
        );
        assert_eq!(
            (
                row.4.as_str(),
                row.6.as_str(),
                row.12.as_str(),
                row.13.as_str()
            ),
            ("0.0.0.0", "secret", "127.0.0.1/32", "cpu")
        );

        let mut summary = conn.prepare(
            "SELECT id, name, instructions, sort_order, created_at, updated_at
             FROM summary_templates ORDER BY sort_order, id",
        )?;
        let summary = summary
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            summary,
            vec![
                (
                    "typed-summary-b".into(),
                    "Summary B".into(),
                    "Second".into(),
                    0,
                    1_725_000_000_123,
                    1_725_000_000_123,
                ),
                (
                    "typed-summary-a".into(),
                    "Summary A".into(),
                    "First".into(),
                    1,
                    1_725_000_000_123,
                    1_725_000_000_123,
                ),
            ]
        );

        let mut polish = conn.prepare(
            "SELECT id, name, context, sort_order, created_at, updated_at
             FROM polish_presets ORDER BY sort_order, id",
        )?;
        let polish = polish
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            polish,
            vec![
                (
                    "typed-polish-b".into(),
                    "Polish B".into(),
                    "Second context".into(),
                    0,
                    1_725_000_000_123,
                    1_725_000_000_123,
                ),
                (
                    "typed-polish-a".into(),
                    "Polish A".into(),
                    "First context".into(),
                    1,
                    1_725_000_000_123,
                    1_725_000_000_123,
                ),
            ]
        );

        let mut sets = conn.prepare(
            "SELECT id, kind, name, enabled, ignore_case, keywords,
                    sort_order, created_at, updated_at
             FROM vocabulary_sets ORDER BY kind, sort_order, id",
        )?;
        let sets = sets
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, i64>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            sets,
            vec![
                (
                    "typed-hotwords-b".into(),
                    "hotword".into(),
                    "Hotwords B".into(),
                    0,
                    0,
                    "".into(),
                    0,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
                (
                    "typed-hotwords-a".into(),
                    "hotword".into(),
                    "Hotwords A".into(),
                    1,
                    0,
                    "".into(),
                    1,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
                (
                    "typed-keywords-b".into(),
                    "polish_keyword".into(),
                    "Keywords B".into(),
                    1,
                    0,
                    "clear, concise".into(),
                    0,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
                (
                    "typed-keywords-a".into(),
                    "polish_keyword".into(),
                    "Keywords A".into(),
                    0,
                    0,
                    "direct, exact".into(),
                    1,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
                (
                    "typed-replacement-b".into(),
                    "text_replacement".into(),
                    "Replacement B".into(),
                    1,
                    1,
                    "".into(),
                    0,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
                (
                    "typed-replacement-a".into(),
                    "text_replacement".into(),
                    "Replacement A".into(),
                    0,
                    0,
                    "".into(),
                    1,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
            ]
        );

        let mut rules = conn.prepare(
            "SELECT id, set_kind, set_id, from_text, to_text, text, sort_order
             FROM vocabulary_rules ORDER BY set_kind, set_id, sort_order, id",
        )?;
        let rules = rules
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(rules.len(), 8);
        assert_eq!(
            rules[0],
            (
                "typed-hotword-a2".into(),
                "hotword".into(),
                "typed-hotwords-a".into(),
                "".into(),
                "".into(),
                "Sona A2".into(),
                0
            )
        );
        assert_eq!(
            rules[1],
            (
                "typed-hotword-a1".into(),
                "hotword".into(),
                "typed-hotwords-a".into(),
                "".into(),
                "".into(),
                "Sona A1".into(),
                1
            )
        );
        assert_eq!(
            rules[2],
            (
                "typed-hotword-b2".into(),
                "hotword".into(),
                "typed-hotwords-b".into(),
                "".into(),
                "".into(),
                "Sona B2".into(),
                0
            )
        );
        assert_eq!(
            rules[3],
            (
                "typed-hotword-b1".into(),
                "hotword".into(),
                "typed-hotwords-b".into(),
                "".into(),
                "".into(),
                "Sona B1".into(),
                1
            )
        );
        assert_eq!(
            rules[4],
            (
                "typed-replace-a2".into(),
                "text_replacement".into(),
                "typed-replacement-a".into(),
                "A2".into(),
                "Two A".into(),
                "".into(),
                0
            )
        );
        assert_eq!(
            rules[5],
            (
                "typed-replace-a1".into(),
                "text_replacement".into(),
                "typed-replacement-a".into(),
                "A1".into(),
                "One A".into(),
                "".into(),
                1
            )
        );
        assert_eq!(
            rules[6],
            (
                "typed-replace-b2".into(),
                "text_replacement".into(),
                "typed-replacement-b".into(),
                "B2".into(),
                "Two B".into(),
                "".into(),
                0
            )
        );
        assert_eq!(
            rules[7],
            (
                "typed-replace-b1".into(),
                "text_replacement".into(),
                "typed-replacement-b".into(),
                "B1".into(),
                "One B".into(),
                "".into(),
                1
            )
        );

        let mut profiles = conn.prepare(
            "SELECT id, name, enabled, sort_order, created_at, updated_at
             FROM speaker_profiles ORDER BY sort_order, id",
        )?;
        let profiles = profiles
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            profiles,
            vec![
                (
                    "typed-speaker-b".into(),
                    "Speaker B".into(),
                    1,
                    0,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
                (
                    "typed-speaker-a".into(),
                    "Speaker A".into(),
                    0,
                    1,
                    1_725_000_000_123,
                    1_725_000_000_123
                ),
            ]
        );

        let mut samples = conn.prepare(
            "SELECT id, profile_id, file_path, source_name, duration_seconds, sort_order
             FROM speaker_profile_samples ORDER BY profile_id, sort_order, id",
        )?;
        let samples = samples
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, i64>(5)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(
            samples,
            vec![
                (
                    "typed-sample-a2".into(),
                    "typed-speaker-a".into(),
                    "profiles/a2.wav".into(),
                    "Sample A2".into(),
                    8.75,
                    0
                ),
                (
                    "typed-sample-a1".into(),
                    "typed-speaker-a".into(),
                    "profiles/a1.wav".into(),
                    "Sample A1".into(),
                    1.5,
                    1
                ),
                (
                    "typed-sample-b2".into(),
                    "typed-speaker-b".into(),
                    "profiles/b2.wav".into(),
                    "Sample B2".into(),
                    12.5,
                    0
                ),
                (
                    "typed-sample-b1".into(),
                    "typed-speaker-b".into(),
                    "profiles/b1.wav".into(),
                    "Sample B1".into(),
                    3.25,
                    1
                ),
            ]
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn open_read_only_loads_the_complete_wal_snapshot() {
    let dir = tempfile::tempdir().unwrap();
    let expected = state("wal", 22);
    let writer = Arc::new(Database::open(dir.path()).unwrap());
    AppConfigStore::replace_state(
        &SqliteConfigStore::new(Arc::clone(&writer)),
        expected.clone(),
    )
    .unwrap();
    let wal_path = dir.path().join("sona.db-wal");
    let shm_path = dir.path().join("sona.db-shm");
    assert!(wal_path.is_file());
    assert!(std::fs::metadata(&wal_path).unwrap().len() > 0);
    assert!(shm_path.is_file());

    let read_only = Arc::new(Database::open_read_only(dir.path()).unwrap());
    let actual = AppConfigStore::load_state(&SqliteConfigStore::new(read_only)).unwrap();

    assert_eq!(actual, Some(expected));
    drop(writer);
}

#[test]
fn replacing_state_removes_every_previous_library_row() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let store = SqliteConfigStore::new(Arc::clone(&db));
    AppConfigStore::replace_state(&store, state("old", 10)).unwrap();
    let mut replacement = state("new", 20);
    replacement.library = AppConfigLibrary::default();

    AppConfigStore::replace_state(&store, replacement.clone()).unwrap();

    assert_eq!(
        AppConfigStore::load_state(&store).unwrap(),
        Some(replacement)
    );
    db.with_connection(|conn| {
        for table in [
            "summary_templates",
            "polish_presets",
            "vocabulary_sets",
            "vocabulary_rules",
            "speaker_profiles",
            "speaker_profile_samples",
        ] {
            let count: i64 =
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
                    row.get(0)
                })?;
            assert_eq!(count, 0, "{table}");
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn nested_rule_trigger_failure_rolls_back_base_and_every_library_table() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let store = SqliteConfigStore::new(Arc::clone(&db));
    let original = state("original", 30);
    AppConfigStore::replace_state(&store, original.clone()).unwrap();
    db.with_write_connection(|conn| {
        conn.execute_batch(
            "CREATE TRIGGER reject_bad_rule BEFORE INSERT ON vocabulary_rules
             WHEN NEW.id = 'bad-rule' BEGIN SELECT RAISE(ABORT, 'bad nested rule'); END;",
        )?;
        Ok(())
    })
    .unwrap();
    let mut rejected = state("rejected", 40);
    rejected.library.text_replacement_sets[0].rules[0].id = "bad-rule".into();

    assert!(AppConfigStore::replace_state(&store, rejected).is_err());
    assert_eq!(AppConfigStore::load_state(&store).unwrap(), Some(original));
}

#[test]
fn startup_reads_do_not_depend_on_unrelated_library_tables() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let store = SqliteConfigStore::new(Arc::clone(&db));
    AppConfigStore::replace_state(&store, state("startup", 50)).unwrap();
    db.with_write_connection(|connection| {
        connection.execute("DROP TABLE summary_templates", [])?;
        Ok(())
    })
    .unwrap();
    let service = AppConfigRepositoryService::new(&store, &FixedClock);

    let payload = service.load_app_config_payload().unwrap().unwrap();
    let settings = service.load_serve_startup_settings().unwrap().unwrap();

    assert_eq!(payload["label"], "startup");
    assert!(settings.enabled);
    assert_eq!(settings.config.host.as_deref(), Some("0.0.0.0"));
    assert_eq!(settings.config.port, Some(15555));
    assert_eq!(settings.config.api_key.as_deref(), Some("secret"));
}

#[test]
fn settings_preserve_exact_raw_json_text() {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let store = SqliteConfigStore::new(db);
    let raw = r#"{ "b": [2, 1], "a" : true }"#.to_string();

    AppConfigStore::set_setting_json(&store, "raw", raw.clone()).unwrap();

    assert_eq!(
        AppConfigStore::load_setting_json(&store, "raw").unwrap(),
        Some(raw)
    );
}

#[test]
fn read_only_open_keeps_future_schema_error_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).unwrap();
    db.with_write_connection(|conn| {
        conn.execute("INSERT INTO schema_version (version) VALUES (99)", [])?;
        Ok(())
    })
    .unwrap();
    drop(db);

    let error = Database::open_read_only(dir.path()).unwrap_err();

    assert!(matches!(
        error,
        DatabaseError::UnsupportedSchemaVersion {
            found: 99,
            current: 2
        }
    ));
}

#[test]
fn read_only_open_keeps_schema_migration_required_error_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let db = Database::open(dir.path()).unwrap();
    db.with_write_connection(|conn| {
        conn.execute("DELETE FROM schema_version", [])?;
        Ok(())
    })
    .unwrap();
    drop(db);

    let error = Database::open_read_only(dir.path()).unwrap_err();

    assert!(matches!(
        error,
        DatabaseError::SchemaMigrationRequired {
            found: 0,
            current: 2
        }
    ));
}
