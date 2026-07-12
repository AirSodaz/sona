use std::collections::HashMap;
use std::sync::Mutex;

use serde_json::{Value, json};
use sona_core::config::{
    AppConfigLibrary, AppConfigRepositoryService, AppConfigRepositorySnapshot,
    AppConfigStartupProjection, AppConfigStore, AppConfigStoredState, HotwordRuleRecord,
    HotwordSetRecord, PolishKeywordSetRecord, PolishPresetRecord, SpeakerProfileRecord,
    SpeakerProfileSampleRecord, SummaryTemplateRecord, TextReplacementRuleRecord,
    TextReplacementSetRecord,
};
use sona_core::ports::time::UnixMillisClock;

#[derive(Clone, Debug, PartialEq)]
enum StoreCall {
    LoadState,
    LoadBaseConfigJson,
    LoadStartupProjection,
    ReplaceState(Box<AppConfigStoredState>),
    LoadSetting(String),
    SetSetting(String, String),
}

#[derive(Default)]
struct MemoryAppConfigStore {
    state: Mutex<Option<AppConfigStoredState>>,
    settings: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<StoreCall>>,
}

impl MemoryAppConfigStore {
    fn with_state(state: AppConfigStoredState) -> Self {
        Self {
            state: Mutex::new(Some(state)),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<StoreCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl AppConfigStore for MemoryAppConfigStore {
    fn load_state(&self) -> Result<Option<AppConfigStoredState>, String> {
        self.calls.lock().unwrap().push(StoreCall::LoadState);
        Ok(self.state.lock().unwrap().clone())
    }

    fn load_base_config_json(&self) -> Result<Option<String>, String> {
        self.calls
            .lock()
            .unwrap()
            .push(StoreCall::LoadBaseConfigJson);
        Ok(self
            .state
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.base_config_json.clone()))
    }

    fn load_startup_projection(&self) -> Result<Option<AppConfigStartupProjection>, String> {
        self.calls
            .lock()
            .unwrap()
            .push(StoreCall::LoadStartupProjection);
        Ok(self
            .state
            .lock()
            .unwrap()
            .as_ref()
            .map(|state| state.startup_projection.clone()))
    }

    fn replace_state(&self, state: AppConfigStoredState) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(StoreCall::ReplaceState(Box::new(state.clone())));
        *self.state.lock().unwrap() = Some(state);
        Ok(())
    }

    fn load_setting_json(&self, key: &str) -> Result<Option<String>, String> {
        self.calls
            .lock()
            .unwrap()
            .push(StoreCall::LoadSetting(key.to_string()));
        Ok(self.settings.lock().unwrap().get(key).cloned())
    }

    fn set_setting_json(&self, key: &str, value_json: String) -> Result<(), String> {
        self.calls
            .lock()
            .unwrap()
            .push(StoreCall::SetSetting(key.to_string(), value_json.clone()));
        self.settings
            .lock()
            .unwrap()
            .insert(key.to_string(), value_json);
        Ok(())
    }
}

struct FixedClock(u64);

struct FailingClock;

static FIXED_CLOCK: FixedClock = FixedClock(1_234_567);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(self.0)
    }
}

impl UnixMillisClock for FailingClock {
    fn now_ms(&self) -> Result<u64, String> {
        Err("clock before Unix epoch".into())
    }
}

fn service(store: &MemoryAppConfigStore) -> AppConfigRepositoryService<'_> {
    AppConfigRepositoryService::new(store, &FIXED_CLOCK)
}

fn empty_projection() -> AppConfigStartupProjection {
    AppConfigStartupProjection {
        http_server_enabled: false,
        host: "127.0.0.1".to_string(),
        port: 14200,
        api_key: String::new(),
        max_concurrent: 2,
        max_queue_size: 100,
        max_upload_size_mb: 50,
        job_ttl_minutes: 60,
        max_streaming: 2,
        ip_whitelist: "localhost".to_string(),
        gpu_acceleration: "auto".to_string(),
    }
}

fn state(base_config: Value, library: AppConfigLibrary) -> AppConfigStoredState {
    AppConfigStoredState {
        base_config_json: serde_json::to_string(&base_config).unwrap(),
        library,
        config_version: 7,
        updated_at: 101,
        startup_projection: empty_projection(),
    }
}

#[test]
fn absent_state_loads_are_none_and_inspection_does_one_read() {
    let store = MemoryAppConfigStore::default();
    let service = service(&store);

    assert_eq!(service.load_config().unwrap(), None);
    assert_eq!(service.load_app_config_payload().unwrap(), None);
    assert!(service.load_serve_startup_settings().unwrap().is_none());
    assert_eq!(service.inspect_state().unwrap(), None);
    assert_eq!(
        store.calls(),
        vec![
            StoreCall::LoadState,
            StoreCall::LoadBaseConfigJson,
            StoreCall::LoadStartupProjection,
            StoreCall::LoadState,
        ]
    );
}

#[test]
fn load_config_reinserts_all_six_typed_library_arrays() {
    let library = AppConfigLibrary {
        summary_templates: vec![SummaryTemplateRecord {
            id: "summary".into(),
            name: "Summary".into(),
            instructions: "Explain".into(),
        }],
        polish_presets: vec![PolishPresetRecord {
            id: "polish".into(),
            name: "Polish".into(),
            context: "Formal".into(),
        }],
        text_replacement_sets: vec![TextReplacementSetRecord {
            id: "text".into(),
            name: "Text".into(),
            enabled: true,
            ignore_case: true,
            rules: vec![TextReplacementRuleRecord {
                id: "text-rule".into(),
                from: "a".into(),
                to: "b".into(),
            }],
        }],
        hotword_sets: vec![HotwordSetRecord {
            id: "hotword".into(),
            name: "Hotword".into(),
            enabled: false,
            rules: vec![HotwordRuleRecord {
                id: "hotword-rule".into(),
                text: "Sona".into(),
            }],
        }],
        polish_keyword_sets: vec![PolishKeywordSetRecord {
            id: "keyword".into(),
            name: "Keyword".into(),
            enabled: true,
            keywords: "clear, concise".into(),
        }],
        speaker_profiles: vec![SpeakerProfileRecord {
            id: "speaker".into(),
            name: "Alice".into(),
            enabled: true,
            samples: vec![SpeakerProfileSampleRecord {
                id: "sample".into(),
                file_path: "alice.wav".into(),
                source_name: "Alice sample".into(),
                duration_seconds: 12.5,
            }],
        }],
    };
    let store =
        MemoryAppConfigStore::with_state(state(json!({"sona-config": {"theme": "dark"}}), library));

    let payload = service(&store).load_app_config_payload().unwrap().unwrap();
    assert_eq!(payload, json!({"theme": "dark"}));
    assert!(payload.get("summaryCustomTemplates").is_none());
    assert!(payload.get("polishCustomPresets").is_none());
    assert!(payload.get("textReplacementSets").is_none());
    assert!(payload.get("hotwordSets").is_none());
    assert!(payload.get("polishKeywordSets").is_none());
    assert!(payload.get("speakerProfiles").is_none());

    let loaded = service(&store).load_config().unwrap().unwrap();

    assert_eq!(
        loaded,
        json!({
            "sona-config": {
                "theme": "dark",
                "summaryCustomTemplates": [{"id":"summary","name":"Summary","instructions":"Explain"}],
                "polishCustomPresets": [{"id":"polish","name":"Polish","context":"Formal"}],
                "textReplacementSets": [{
                    "id":"text","name":"Text","enabled":true,"ignoreCase":true,
                    "rules":[{"id":"text-rule","from":"a","to":"b"}]
                }],
                "hotwordSets": [{
                    "id":"hotword","name":"Hotword","enabled":false,
                    "rules":[{"id":"hotword-rule","text":"Sona"}]
                }],
                "polishKeywordSets": [{
                    "id":"keyword","name":"Keyword","enabled":true,"keywords":"clear, concise"
                }],
                "speakerProfiles": [{
                    "id":"speaker","name":"Alice","enabled":true,
                    "samples":[{"id":"sample","filePath":"alice.wav","sourceName":"Alice sample","durationSeconds":12.5}]
                }]
            }
        })
    );
    assert_eq!(
        store.calls(),
        vec![StoreCall::LoadBaseConfigJson, StoreCall::LoadState]
    );
}

#[test]
fn save_selects_object_wrappers_in_priority_order_with_complete_stored_states() {
    let cases = [
        (
            json!({
                "sona-config": {
                    "configVersion": 3,
                    "httpServerEnabled": true,
                    "httpServerHost": "dash-host",
                    "summaryCustomTemplates": [{"id":"dash-summary","name":"Dash","instructions":"D"}],
                    "polishCustomPresets": [{"id":"dash-polish","name":"Dash","context":"D"}],
                    "textReplacementSets": [{"id":"dash-text","name":"Dash","enabled":false,"ignoreCase":true,"rules":[]}],
                    "hotwordSets": [{"id":"dash-hotword","name":"Dash","enabled":false,"rules":[]}],
                    "polishKeywordSets": [{"id":"dash-keyword","name":"Dash","enabled":false,"keywords":"D"}],
                    "speakerProfiles": [{"id":"dash-speaker","name":"Dash","enabled":false,"samples":[]}]
                },
                "sona_config": {"configVersion":4,"summaryCustomTemplates":[{"id":"wrong-underscore"}]},
                "config": {"configVersion":5,"summaryCustomTemplates":[{"id":"wrong-config"}]}
            }),
            json!({
                "sona-config": {
                    "configVersion": 3,
                    "httpServerEnabled": true,
                    "httpServerHost": "dash-host"
                },
                "sona_config": {"configVersion":4,"summaryCustomTemplates":[{"id":"wrong-underscore"}]},
                "config": {"configVersion":5,"summaryCustomTemplates":[{"id":"wrong-config"}]}
            }),
            AppConfigLibrary {
                summary_templates: vec![SummaryTemplateRecord {
                    id: "dash-summary".into(),
                    name: "Dash".into(),
                    instructions: "D".into(),
                }],
                polish_presets: vec![PolishPresetRecord {
                    id: "dash-polish".into(),
                    name: "Dash".into(),
                    context: "D".into(),
                }],
                text_replacement_sets: vec![TextReplacementSetRecord {
                    id: "dash-text".into(),
                    name: "Dash".into(),
                    enabled: false,
                    ignore_case: true,
                    rules: vec![],
                }],
                hotword_sets: vec![HotwordSetRecord {
                    id: "dash-hotword".into(),
                    name: "Dash".into(),
                    enabled: false,
                    rules: vec![],
                }],
                polish_keyword_sets: vec![PolishKeywordSetRecord {
                    id: "dash-keyword".into(),
                    name: "Dash".into(),
                    enabled: false,
                    keywords: "D".into(),
                }],
                speaker_profiles: vec![SpeakerProfileRecord {
                    id: "dash-speaker".into(),
                    name: "Dash".into(),
                    enabled: false,
                    samples: vec![],
                }],
            },
            3,
            AppConfigStartupProjection {
                host: "dash-host".into(),
                http_server_enabled: true,
                ..empty_projection()
            },
        ),
        (
            json!({
                "sona-config": null,
                "sona_config": {
                    "configVersion": 4,
                    "httpServerPort": 14444,
                    "summaryCustomTemplates": [{"id":"underscore-summary","name":"Under","instructions":"U"}],
                    "polishCustomPresets": [], "textReplacementSets": [], "hotwordSets": [],
                    "polishKeywordSets": [], "speakerProfiles": []
                },
                "config": {"configVersion":5,"summaryCustomTemplates":[{"id":"wrong-config"}]}
            }),
            json!({
                "sona-config": null,
                "sona_config": {"configVersion":4,"httpServerPort":14444},
                "config": {"configVersion":5,"summaryCustomTemplates":[{"id":"wrong-config"}]}
            }),
            AppConfigLibrary {
                summary_templates: vec![SummaryTemplateRecord {
                    id: "underscore-summary".into(),
                    name: "Under".into(),
                    instructions: "U".into(),
                }],
                ..AppConfigLibrary::default()
            },
            4,
            AppConfigStartupProjection {
                port: 14444,
                ..empty_projection()
            },
        ),
        (
            json!({
                "sona-config": [],
                "sona_config": false,
                "config": {
                    "configVersion": 5,
                    "gpuAcceleration": "cpu",
                    "summaryCustomTemplates": [], "polishCustomPresets": [],
                    "textReplacementSets": [], "hotwordSets": [], "polishKeywordSets": [],
                    "speakerProfiles": [{"id":"config-speaker","name":"Config","enabled":true,"samples":[]}]
                }
            }),
            json!({
                "sona-config": [],
                "sona_config": false,
                "config": {"configVersion":5,"gpuAcceleration":"cpu"}
            }),
            AppConfigLibrary {
                speaker_profiles: vec![SpeakerProfileRecord {
                    id: "config-speaker".into(),
                    name: "Config".into(),
                    enabled: true,
                    samples: vec![],
                }],
                ..AppConfigLibrary::default()
            },
            5,
            AppConfigStartupProjection {
                gpu_acceleration: "cpu".into(),
                ..empty_projection()
            },
        ),
    ];

    for (input, expected_base, library, config_version, startup_projection) in cases {
        let store = MemoryAppConfigStore::default();
        service(&store).save_config(&input).unwrap();
        assert_eq!(
            store.state.lock().unwrap().clone().unwrap(),
            AppConfigStoredState {
                base_config_json: serde_json::to_string(&expected_base).unwrap(),
                library,
                config_version,
                updated_at: 1_234_567,
                startup_projection,
            }
        );
        assert_eq!(store.calls().len(), 1);
    }
}

#[test]
fn wrapper_priority_is_object_only_for_payload_load_and_library_injection() {
    let cases = [
        (
            json!({
                "sona-config": {"selected": "dash"},
                "sona_config": {"selected": "underscore"},
                "config": {"selected": "config"}
            }),
            json!({"selected": "dash"}),
            "sona-config",
        ),
        (
            json!({
                "sona-config": null,
                "sona_config": {"selected": "underscore"},
                "config": {"selected": "config"}
            }),
            json!({"selected": "underscore"}),
            "sona_config",
        ),
        (
            json!({
                "sona-config": [],
                "sona_config": false,
                "config": {"selected": "config"}
            }),
            json!({"selected": "config"}),
            "config",
        ),
        (
            json!({"sona-config": null, "sona_config": [], "config": "bad", "selected": "top"}),
            json!({"sona-config": null, "sona_config": [], "config": "bad", "selected": "top"}),
            "top",
        ),
    ];

    for (base, expected_payload, selected_key) in cases {
        let store = MemoryAppConfigStore::with_state(state(base, AppConfigLibrary::default()));
        let payload = service(&store).load_app_config_payload().unwrap().unwrap();
        assert_eq!(payload, expected_payload, "payload for {selected_key}");
        assert!(payload.get("summaryCustomTemplates").is_none());

        let loaded = service(&store).load_config().unwrap().unwrap();
        let selected = if selected_key == "top" {
            &loaded
        } else {
            &loaded[selected_key]
        };
        assert_eq!(selected["summaryCustomTemplates"], json!([]));
        assert_eq!(selected["polishCustomPresets"], json!([]));
        assert_eq!(selected["textReplacementSets"], json!([]));
        assert_eq!(selected["hotwordSets"], json!([]));
        assert_eq!(selected["polishKeywordSets"], json!([]));
        assert_eq!(selected["speakerProfiles"], json!([]));
    }
}

#[test]
fn save_extracts_typed_records_with_exact_defaults_and_drops_invalid_entries() {
    let store = MemoryAppConfigStore::default();
    let input = json!({
        "theme": "dark",
        "summaryCustomTemplates": [null, {"id":"s","name":9,"instructions":"I"}],
        "polishCustomPresets": "not-array",
        "textReplacementSets": [{"id":"t","rules":[false,{"id":"r","from":1,"to":"T"}]}],
        "hotwordSets": [{"id":"h","name":"H","enabled":false,"rules":"bad"}],
        "polishKeywordSets": [{"id":"k","enabled":"bad"}],
        "speakerProfiles": [{
            "id":"p","samples":[null,{"id":"sample","filePath":1,"sourceName":"source","durationSeconds":"bad"}]
        }]
    });

    service(&store).save_config(&input).unwrap();

    let stored = store.state.lock().unwrap().clone().unwrap();
    assert_eq!(stored.base_config_json, "{\"theme\":\"dark\"}");
    assert_eq!(
        stored.library,
        AppConfigLibrary {
            summary_templates: vec![SummaryTemplateRecord {
                id: "s".into(),
                name: "".into(),
                instructions: "I".into(),
            }],
            polish_presets: vec![],
            text_replacement_sets: vec![TextReplacementSetRecord {
                id: "t".into(),
                name: "".into(),
                enabled: true,
                ignore_case: false,
                rules: vec![TextReplacementRuleRecord {
                    id: "r".into(),
                    from: "".into(),
                    to: "T".into(),
                }],
            }],
            hotword_sets: vec![HotwordSetRecord {
                id: "h".into(),
                name: "H".into(),
                enabled: false,
                rules: vec![],
            }],
            polish_keyword_sets: vec![PolishKeywordSetRecord {
                id: "k".into(),
                name: "".into(),
                enabled: true,
                keywords: "".into(),
            }],
            speaker_profiles: vec![SpeakerProfileRecord {
                id: "p".into(),
                name: "".into(),
                enabled: true,
                samples: vec![SpeakerProfileSampleRecord {
                    id: "sample".into(),
                    file_path: "".into(),
                    source_name: "source".into(),
                    duration_seconds: 0.0,
                }],
            }],
        }
    );
    assert_eq!(store.calls().len(), 1);
    assert!(matches!(store.calls()[0], StoreCall::ReplaceState(_)));
}

#[test]
fn save_repairs_whitespace_missing_and_duplicate_ids_with_exact_legacy_hashes() {
    let store = MemoryAppConfigStore::default();
    let input = json!({
        "summaryCustomTemplates": [
            {"name":"No ID","instructions":"Keep me"},
            {"id":"duplicate","name":"First duplicate","instructions":"First"},
            {"id":"duplicate","name":"Second duplicate","instructions":"Second"}
        ],
        "polishCustomPresets": [{"id":"  kept-trimmed  "}],
        "textReplacementSets": [{"id":"text-set","rules":[
            {"from":"a","to":"b"},
            {"id":"same-rule","from":"c","to":"d"},
            {"id":"same-rule","from":"e","to":"f"}
        ]}],
        "hotwordSets": [
            {"id":"h1","rules":[{"text":"alpha"}]},
            {"id":"h2","rules":[{"text":"alpha"}]}
        ],
        "polishKeywordSets": [{"id":"   ","keywords":"one"}],
        "speakerProfiles": [{"id":"speaker","samples":[
            {"filePath":"a.wav","sourceName":"A","durationSeconds":1.0}
        ]}]
    });

    service(&store).save_config(&input).unwrap();
    let first = store.state.lock().unwrap().clone().unwrap().library;

    assert_eq!(first.summary_templates[0].id, "summary-template-cf490801");
    assert_eq!(first.summary_templates[1].id, "duplicate");
    assert_eq!(first.summary_templates[2].id, "summary-template-cb777487");
    assert_eq!(first.polish_presets[0].id, "kept-trimmed");
    assert_eq!(
        first.text_replacement_sets[0].rules[0].id,
        "text-replacement-rule-fecefa3c"
    );
    assert_eq!(first.text_replacement_sets[0].rules[1].id, "same-rule");
    assert_eq!(
        first.text_replacement_sets[0].rules[2].id,
        "text-replacement-rule-616f775c"
    );
    assert_eq!(first.hotword_sets[0].rules[0].id, "hotword-rule-f2d71ccd");
    assert_eq!(
        first.hotword_sets[1].rules[0].id, "hotword-rule-f2d71ccd",
        "nested scopes have independent seen sets"
    );
    assert!(
        first.polish_keyword_sets[0]
            .id
            .starts_with("polish-keyword-set-")
    );
    assert_eq!(
        first.speaker_profiles[0].samples[0].id,
        "speaker-sample-e9c8ca09"
    );

    service(&store).save_config(&input).unwrap();
    let second = store.state.lock().unwrap().clone().unwrap().library;
    assert_eq!(second, first, "repair must be deterministic across calls");
}

#[test]
fn save_preserves_zero_timestamp_when_clock_fails() {
    let store = MemoryAppConfigStore::default();

    AppConfigRepositoryService::new(&store, &FailingClock)
        .save_config(&json!({"theme": "dark"}))
        .unwrap();

    assert_eq!(store.state.lock().unwrap().as_ref().unwrap().updated_at, 0);
}

#[test]
fn id_repair_hashes_utf16_units_and_suffixes_generated_collisions() {
    let store = MemoryAppConfigStore::default();
    service(&store)
        .save_config(&json!({
            "summaryCustomTemplates": [
                {"name":"😀"},
                {"id":"summary-template-25805e9f"},
                {"name":"collision"}
            ],
            "polishCustomPresets": [
                {"id":"polish-preset-25805e9f"},
                {"id":"polish-preset-25805e9f-2"},
                {"name":"collision"}
            ]
        }))
        .unwrap();

    let stored = store.state.lock().unwrap();
    let library = &stored.as_ref().unwrap().library;
    assert_eq!(library.summary_templates[0].id, "summary-template-61c06abe");
    assert_eq!(
        library.summary_templates[2].id,
        "summary-template-25805e9f-2"
    );
    assert_eq!(library.polish_presets[2].id, "polish-preset-25805e9f-3");
}

#[test]
fn every_library_id_scope_repairs_missing_blank_and_duplicate_ids_to_exact_hashes() {
    let repair_rows = json!([{}, {"id":"   "}, {"id":"dup"}, {"id":"dup"}]);
    let store = MemoryAppConfigStore::default();
    service(&store)
        .save_config(&json!({
            "summaryCustomTemplates": repair_rows,
            "polishCustomPresets": [{}, {"id":"   "}, {"id":"dup"}, {"id":"dup"}],
            "textReplacementSets": [
                {}, {"id":"   "}, {"id":"dup"}, {"id":"dup"},
                {"id":"nested-text","rules":[{}, {"id":"   "}, {"id":"dup"}, {"id":"dup"}]}
            ],
            "hotwordSets": [
                {}, {"id":"   "}, {"id":"dup"}, {"id":"dup"},
                {"id":"nested-hotword","rules":[{}, {"id":"   "}, {"id":"dup"}, {"id":"dup"}]}
            ],
            "polishKeywordSets": [{}, {"id":"   "}, {"id":"dup"}, {"id":"dup"}],
            "speakerProfiles": [
                {}, {"id":"   "}, {"id":"dup"}, {"id":"dup"},
                {"id":"nested-speaker","samples":[{}, {"id":"   "}, {"id":"dup"}, {"id":"dup"}]}
            ]
        }))
        .unwrap();

    let library = store
        .state
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .library
        .clone();
    assert_eq!(
        library
            .summary_templates
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "summary-template-7c7d7c1e",
            "summary-template-08c0f868",
            "dup",
            "summary-template-3219f40b",
        ]
    );
    assert_eq!(
        library
            .polish_presets
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "polish-preset-7c7d7c1e",
            "polish-preset-08c0f868",
            "dup",
            "polish-preset-3219f40b",
        ]
    );
    assert_eq!(
        library
            .text_replacement_sets
            .iter()
            .take(4)
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "text-replacement-set-7c7d7c1e",
            "text-replacement-set-08c0f868",
            "dup",
            "text-replacement-set-3219f40b",
        ]
    );
    assert_eq!(
        library.text_replacement_sets[4]
            .rules
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "text-replacement-rule-7c7d7c1e",
            "text-replacement-rule-08c0f868",
            "dup",
            "text-replacement-rule-3219f40b",
        ]
    );
    assert_eq!(
        library
            .hotword_sets
            .iter()
            .take(4)
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "hotword-set-7c7d7c1e",
            "hotword-set-08c0f868",
            "dup",
            "hotword-set-3219f40b",
        ]
    );
    assert_eq!(
        library.hotword_sets[4]
            .rules
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "hotword-rule-7c7d7c1e",
            "hotword-rule-08c0f868",
            "dup",
            "hotword-rule-3219f40b",
        ]
    );
    assert_eq!(
        library
            .polish_keyword_sets
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "polish-keyword-set-7c7d7c1e",
            "polish-keyword-set-08c0f868",
            "dup",
            "polish-keyword-set-3219f40b",
        ]
    );
    assert_eq!(
        library
            .speaker_profiles
            .iter()
            .take(4)
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "speaker-profile-7c7d7c1e",
            "speaker-profile-08c0f868",
            "dup",
            "speaker-profile-3219f40b",
        ]
    );
    assert_eq!(
        library.speaker_profiles[4]
            .samples
            .iter()
            .map(|record| record.id.as_str())
            .collect::<Vec<_>>(),
        [
            "speaker-sample-7c7d7c1e",
            "speaker-sample-08c0f868",
            "dup",
            "speaker-sample-3219f40b",
        ]
    );
}

#[test]
fn save_projects_version_timestamp_and_rounded_startup_primitive_fields() {
    let store = MemoryAppConfigStore::default();
    service(&store)
        .save_config(&json!({
            "configVersion": 6.6,
            "httpServerEnabled": true,
            "httpServerHost": "0.0.0.0",
            "httpServerPort": 15555.6,
            "httpServerApiKey": "secret",
            "httpServerMaxConcurrent": 3.5,
            "httpServerMaxQueueSize": 31.4,
            "httpServerMaxUploadSizeMB": 128.5,
            "httpServerJobTtlMinutes": 15.2,
            "httpServerMaxStreaming": 6.5,
            "httpServerIpWhitelist": "127.0.0.1/32",
            "gpuAcceleration": "cpu"
        }))
        .unwrap();

    let stored = store.state.lock().unwrap().clone().unwrap();
    assert_eq!(stored.config_version, 7);
    assert_eq!(stored.updated_at, 1_234_567);
    assert_eq!(
        stored.startup_projection,
        AppConfigStartupProjection {
            http_server_enabled: true,
            host: "0.0.0.0".into(),
            port: 15556,
            api_key: "secret".into(),
            max_concurrent: 4,
            max_queue_size: 31,
            max_upload_size_mb: 129,
            job_ttl_minutes: 15,
            max_streaming: 7,
            ip_whitelist: "127.0.0.1/32".into(),
            gpu_acceleration: "cpu".into(),
        }
    );
}

#[test]
fn save_uses_projection_and_version_defaults_for_wrong_types() {
    let store = MemoryAppConfigStore::default();
    service(&store)
        .save_config(&json!({
            "configVersion": "bad",
            "httpServerEnabled": 1,
            "httpServerHost": false,
            "httpServerPort": "bad",
            "httpServerApiKey": null,
            "httpServerMaxConcurrent": [],
            "httpServerMaxQueueSize": {},
            "httpServerMaxUploadSizeMB": false,
            "httpServerJobTtlMinutes": "bad",
            "httpServerMaxStreaming": null,
            "httpServerIpWhitelist": 1,
            "gpuAcceleration": false
        }))
        .unwrap();

    let stored = store.state.lock().unwrap().clone().unwrap();
    assert_eq!(stored.config_version, 7);
    assert_eq!(stored.startup_projection, empty_projection());
}

#[test]
fn numeric_compatibility_covers_rounding_integer_paths_saturation_and_duration_coercion() {
    let above_i64 = i64::MAX as u64 + 1;
    let input = json!({
        "configVersion": above_i64,
        "httpServerPort": -1.5,
        "httpServerMaxConcurrent": i64::MIN,
        "httpServerMaxQueueSize": i64::MAX,
        "httpServerMaxUploadSizeMB": above_i64,
        "httpServerJobTtlMinutes": -2.5,
        "httpServerMaxStreaming": 4_i64,
        "speakerProfiles": [{
            "id":"speaker", "name":"Speaker", "enabled":true,
            "samples":[{"id":"sample","filePath":"a.wav","sourceName":"A","durationSeconds":7}]
        }]
    });
    let store = MemoryAppConfigStore::default();

    service(&store).save_config(&input).unwrap();

    assert_eq!(
        store.state.lock().unwrap().clone().unwrap(),
        AppConfigStoredState {
            base_config_json: serde_json::to_string(&json!({
                "configVersion": above_i64,
                "httpServerPort": -1.5,
                "httpServerMaxConcurrent": i64::MIN,
                "httpServerMaxQueueSize": i64::MAX,
                "httpServerMaxUploadSizeMB": above_i64,
                "httpServerJobTtlMinutes": -2.5,
                "httpServerMaxStreaming": 4_i64
            }))
            .unwrap(),
            library: AppConfigLibrary {
                speaker_profiles: vec![SpeakerProfileRecord {
                    id: "speaker".into(),
                    name: "Speaker".into(),
                    enabled: true,
                    samples: vec![SpeakerProfileSampleRecord {
                        id: "sample".into(),
                        file_path: "a.wav".into(),
                        source_name: "A".into(),
                        duration_seconds: 7.0,
                    }],
                }],
                ..AppConfigLibrary::default()
            },
            config_version: i64::MAX,
            updated_at: 1_234_567,
            startup_projection: AppConfigStartupProjection {
                port: -2,
                max_concurrent: i64::MIN,
                max_queue_size: i64::MAX,
                max_upload_size_mb: i64::MAX,
                job_ttl_minutes: -3,
                max_streaming: 4,
                ..empty_projection()
            },
        }
    );
}

#[test]
fn startup_load_applies_checked_integer_fallbacks_and_complete_projection() {
    let projection = AppConfigStartupProjection {
        http_server_enabled: true,
        host: "0.0.0.0".into(),
        port: 70_000,
        api_key: "key".into(),
        max_concurrent: -1,
        max_queue_size: -2,
        max_upload_size_mb: -3,
        job_ttl_minutes: -9,
        max_streaming: -4,
        ip_whitelist: "10.0.0.0/8".into(),
        gpu_acceleration: "cuda".into(),
    };
    let mut stored = state(json!({}), AppConfigLibrary::default());
    stored.startup_projection = projection;
    let store = MemoryAppConfigStore::with_state(stored);

    let settings = service(&store)
        .load_serve_startup_settings()
        .unwrap()
        .unwrap();

    assert!(settings.enabled);
    assert_eq!(settings.config.host.as_deref(), Some("0.0.0.0"));
    assert_eq!(settings.config.port, Some(14200));
    assert_eq!(settings.config.api_key.as_deref(), Some("key"));
    assert_eq!(settings.config.models_dir, None);
    assert_eq!(settings.config.max_concurrent, Some(2));
    assert_eq!(settings.config.max_queue_size, Some(100));
    assert_eq!(settings.config.max_upload_size_mb, Some(50));
    assert_eq!(settings.config.job_ttl_minutes, Some(60));
    assert_eq!(settings.config.max_streaming, Some(2));
    assert_eq!(settings.config.ip_whitelist.as_deref(), Some("10.0.0.0/8"));
    assert_eq!(settings.config.gpu_acceleration.as_deref(), Some("cuda"));
    assert_eq!(settings.config.vad_model_id, None);
    assert_eq!(settings.config.punctuation_model_id, None);

    let mut negative_port = state(json!({}), AppConfigLibrary::default());
    negative_port.startup_projection.port = -1;
    let negative_port_store = MemoryAppConfigStore::with_state(negative_port);
    assert_eq!(
        service(&negative_port_store)
            .load_serve_startup_settings()
            .unwrap()
            .unwrap()
            .config
            .port,
        Some(14200)
    );
}

#[test]
fn setting_json_round_trips_malformed_storage_and_preserves_empty_keys() {
    let store = MemoryAppConfigStore::default();
    store
        .settings
        .lock()
        .unwrap()
        .insert("bad".into(), "{".into());
    let service = service(&store);

    assert_eq!(service.get_setting("").unwrap(), None);
    service
        .set_setting("", &json!({"version": 1, "status": "completed"}))
        .unwrap();
    assert_eq!(
        service.get_setting("").unwrap(),
        Some(json!({"version": 1, "status": "completed"}))
    );
    assert!(
        service
            .get_setting("bad")
            .unwrap_err()
            .starts_with("Serialization error: EOF while parsing an object at line 1 column 1")
    );
    assert_eq!(
        store.calls(),
        vec![
            StoreCall::LoadSetting("".into()),
            StoreCall::SetSetting("".into(), "{\"status\":\"completed\",\"version\":1}".into()),
            StoreCall::LoadSetting("".into()),
            StoreCall::LoadSetting("bad".into()),
        ]
    );
}

#[test]
fn inspection_contains_complete_config_metadata_and_exact_counts() {
    let library = AppConfigLibrary {
        summary_templates: vec![SummaryTemplateRecord {
            id: "s".into(),
            name: "S".into(),
            instructions: "I".into(),
        }],
        polish_presets: vec![PolishPresetRecord {
            id: "p".into(),
            name: "P".into(),
            context: "C".into(),
        }],
        text_replacement_sets: vec![TextReplacementSetRecord {
            id: "t".into(),
            name: "T".into(),
            enabled: true,
            ignore_case: false,
            rules: vec![],
        }],
        hotword_sets: vec![
            HotwordSetRecord {
                id: "h1".into(),
                name: "H1".into(),
                enabled: true,
                rules: vec![],
            },
            HotwordSetRecord {
                id: "h2".into(),
                name: "H2".into(),
                enabled: true,
                rules: vec![],
            },
        ],
        polish_keyword_sets: vec![PolishKeywordSetRecord {
            id: "k".into(),
            name: "K".into(),
            enabled: true,
            keywords: "x".into(),
        }],
        speaker_profiles: vec![SpeakerProfileRecord {
            id: "sp".into(),
            name: "SP".into(),
            enabled: true,
            samples: vec![],
        }],
    };
    let store = MemoryAppConfigStore::with_state(state(json!({"theme":"dark"}), library));

    let snapshot = service(&store).inspect_state().unwrap().unwrap();
    let expected = AppConfigRepositorySnapshot {
        config: json!({
            "theme": "dark",
            "summaryCustomTemplates": [{"id":"s","name":"S","instructions":"I"}],
            "polishCustomPresets": [{"id":"p","name":"P","context":"C"}],
            "textReplacementSets": [{
                "id":"t","name":"T","enabled":true,"ignoreCase":false,"rules":[]
            }],
            "hotwordSets": [
                {"id":"h1","name":"H1","enabled":true,"rules":[]},
                {"id":"h2","name":"H2","enabled":true,"rules":[]}
            ],
            "polishKeywordSets": [{"id":"k","name":"K","enabled":true,"keywords":"x"}],
            "speakerProfiles": [{"id":"sp","name":"SP","enabled":true,"samples":[]}]
        }),
        config_version: 7,
        updated_at: 101,
        summary_template_count: 1,
        polish_preset_count: 1,
        vocabulary_set_count: 4,
        speaker_profile_count: 1,
    };

    assert_eq!(snapshot, expected);
    assert_eq!(
        serde_json::to_value(&snapshot).unwrap(),
        json!({
            "config": expected.config,
            "configVersion": 7,
            "updatedAt": 101,
            "summaryTemplateCount": 1,
            "polishPresetCount": 1,
            "vocabularySetCount": 4,
            "speakerProfileCount": 1
        })
    );
    assert_eq!(store.calls(), vec![StoreCall::LoadState]);
}

#[test]
fn malformed_base_json_uses_serialization_prefix() {
    let malformed = AppConfigStoredState {
        base_config_json: "{".into(),
        library: AppConfigLibrary::default(),
        config_version: 7,
        updated_at: 0,
        startup_projection: empty_projection(),
    };
    let malformed_store = MemoryAppConfigStore::with_state(malformed);
    assert!(
        service(&malformed_store)
            .load_config()
            .unwrap_err()
            .starts_with("Serialization error: EOF while parsing an object at line 1 column 1")
    );
}
