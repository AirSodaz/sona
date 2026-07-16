mod app_config;
mod library;
mod settings;

use std::collections::BTreeSet;
use std::sync::Arc;

use rusqlite::Transaction;
use serde_json::Value;
use sona_core::config::{
    AppConfigRepositoryService, AppConfigRepositorySnapshot, AppConfigStartupProjection,
    AppConfigStore, AppConfigStoredState,
};
use sona_core::ports::time::UnixMillisClock;
use sona_core::runtime::serve::ServeStartupSettings;
use sona_core::sync::SyncEntityKind;

use crate::{
    DatabaseError,
    ports::Database as DatabasePort,
    sync_repository::{
        record_local_delete_in_transaction, record_local_field_change_in_transaction,
    },
};

const PORTABLE_CONFIG_FIELDS: &[&str] = &[
    "language",
    "enableTimeline",
    "enableITN",
    "batchVadEnabled",
    "vadBufferSize",
    "maxConcurrent",
    "llmSettings",
    "asr",
    "summaryEnabled",
    "summaryTemplateId",
    "translationLanguage",
    "polishKeywords",
    "polishPresetId",
    "polishContext",
    "polishScenario",
    "autoPolish",
    "autoPolishFrequency",
];

pub(crate) fn load_state_in_transaction(
    tx: &Transaction<'_>,
) -> Result<Option<AppConfigStoredState>, DatabaseError> {
    let Some(base) = app_config::load(tx)? else {
        return Ok(None);
    };
    let library = library::load(&tx)?;
    Ok(Some(AppConfigStoredState {
        base_config_json: base.base_config_json,
        library,
        config_version: base.config_version,
        updated_at: base.updated_at,
        startup_projection: base.startup_projection,
    }))
}

pub(crate) fn replace_state_in_transaction(
    tx: &Transaction<'_>,
    state: &AppConfigStoredState,
) -> Result<(), DatabaseError> {
    let previous_library = library::load(tx)?;
    library::replace(tx, &state.library, state.updated_at)?;
    app_config::replace(tx, state)?;
    record_portable_config_sync(tx, state)?;
    record_library_sync(tx, &previous_library, &state.library, state.updated_at)
}

fn record_portable_config_sync(
    tx: &Transaction<'_>,
    state: &AppConfigStoredState,
) -> Result<(), DatabaseError> {
    let config: Value = serde_json::from_str(&state.base_config_json)?;
    let object = config.as_object().ok_or_else(|| {
        DatabaseError::Internal("App config must be a JSON object for sync.".to_string())
    })?;
    let now_ms = u64::try_from(state.updated_at).unwrap_or(0);
    for field in PORTABLE_CONFIG_FIELDS {
        let Some(value) = object.get(*field) else {
            continue;
        };
        let value = match *field {
            "asr" => redact_sensitive_config(value, true),
            "llmSettings" => redact_sensitive_config(value, false),
            _ => value.clone(),
        };
        record_local_field_change_in_transaction(
            tx,
            SyncEntityKind::Setting,
            &format!("app-config::{field}"),
            "value",
            value,
            now_ms,
        )?;
    }
    Ok(())
}

fn redact_sensitive_config(value: &Value, strip_paths: bool) -> Value {
    match value {
        Value::Object(object) => Value::Object(
            object
                .iter()
                .filter(|(key, _)| !is_sensitive_config_key(key, strip_paths))
                .map(|(key, value)| (key.clone(), redact_sensitive_config(value, strip_paths)))
                .collect(),
        ),
        Value::Array(values) => Value::Array(
            values
                .iter()
                .map(|value| redact_sensitive_config(value, strip_paths))
                .collect(),
        ),
        _ => value.clone(),
    }
}

fn is_sensitive_config_key(key: &str, strip_paths: bool) -> bool {
    let normalized = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    normalized.contains("apikey")
        || normalized.contains("token")
        || normalized.contains("secret")
        || normalized.contains("password")
        || normalized.contains("credential")
        || (strip_paths && (normalized.ends_with("path") || normalized.contains("directory")))
}

fn record_library_sync(
    tx: &Transaction<'_>,
    previous: &sona_core::config::AppConfigLibrary,
    current: &sona_core::config::AppConfigLibrary,
    updated_at: i64,
) -> Result<(), DatabaseError> {
    let now_ms = u64::try_from(updated_at).unwrap_or(0);

    record_removed_ids(
        tx,
        SyncEntityKind::SummaryTemplate,
        previous
            .summary_templates
            .iter()
            .map(|record| record.id.as_str()),
        current
            .summary_templates
            .iter()
            .map(|record| record.id.as_str()),
        now_ms,
    )?;
    for (sort_order, record) in current.summary_templates.iter().enumerate() {
        record_fields(
            tx,
            SyncEntityKind::SummaryTemplate,
            &record.id,
            [
                ("name", serde_json::json!(record.name)),
                ("instructions", serde_json::json!(record.instructions)),
                ("sortOrder", serde_json::json!(sort_order)),
                ("updatedAt", serde_json::json!(now_ms)),
            ],
            now_ms,
        )?;
    }

    record_removed_ids(
        tx,
        SyncEntityKind::PolishPreset,
        previous
            .polish_presets
            .iter()
            .map(|record| record.id.as_str()),
        current
            .polish_presets
            .iter()
            .map(|record| record.id.as_str()),
        now_ms,
    )?;
    for (sort_order, record) in current.polish_presets.iter().enumerate() {
        record_fields(
            tx,
            SyncEntityKind::PolishPreset,
            &record.id,
            [
                ("name", serde_json::json!(record.name)),
                ("context", serde_json::json!(record.context)),
                ("sortOrder", serde_json::json!(sort_order)),
                ("updatedAt", serde_json::json!(now_ms)),
            ],
            now_ms,
        )?;
    }

    let previous_set_ids = vocabulary_set_ids(previous);
    let current_set_ids = vocabulary_set_ids(current);
    record_removed_ids(
        tx,
        SyncEntityKind::VocabularySet,
        previous_set_ids.iter().map(String::as_str),
        current_set_ids.iter().map(String::as_str),
        now_ms,
    )?;
    let previous_rule_ids = vocabulary_rule_ids(previous);
    let current_rule_ids = vocabulary_rule_ids(current);
    record_removed_ids(
        tx,
        SyncEntityKind::VocabularyRule,
        previous_rule_ids.iter().map(String::as_str),
        current_rule_ids.iter().map(String::as_str),
        now_ms,
    )?;

    for (sort_order, set) in current.text_replacement_sets.iter().enumerate() {
        let entity_id = format!("text_replacement::{}", set.id);
        record_fields(
            tx,
            SyncEntityKind::VocabularySet,
            &entity_id,
            [
                ("name", serde_json::json!(set.name)),
                ("enabled", serde_json::json!(set.enabled)),
                ("ignoreCase", serde_json::json!(set.ignore_case)),
                ("sortOrder", serde_json::json!(sort_order)),
                ("updatedAt", serde_json::json!(now_ms)),
            ],
            now_ms,
        )?;
        for (rule_order, rule) in set.rules.iter().enumerate() {
            let rule_id = format!("text_replacement::{}::{}", set.id, rule.id);
            record_fields(
                tx,
                SyncEntityKind::VocabularyRule,
                &rule_id,
                [
                    ("from", serde_json::json!(rule.from)),
                    ("to", serde_json::json!(rule.to)),
                    ("sortOrder", serde_json::json!(rule_order)),
                ],
                now_ms,
            )?;
        }
    }
    for (sort_order, set) in current.hotword_sets.iter().enumerate() {
        let entity_id = format!("hotword::{}", set.id);
        record_fields(
            tx,
            SyncEntityKind::VocabularySet,
            &entity_id,
            [
                ("name", serde_json::json!(set.name)),
                ("enabled", serde_json::json!(set.enabled)),
                ("sortOrder", serde_json::json!(sort_order)),
                ("updatedAt", serde_json::json!(now_ms)),
            ],
            now_ms,
        )?;
        for (rule_order, rule) in set.rules.iter().enumerate() {
            let rule_id = format!("hotword::{}::{}", set.id, rule.id);
            record_fields(
                tx,
                SyncEntityKind::VocabularyRule,
                &rule_id,
                [
                    ("text", serde_json::json!(rule.text)),
                    ("sortOrder", serde_json::json!(rule_order)),
                ],
                now_ms,
            )?;
        }
    }
    for (sort_order, set) in current.polish_keyword_sets.iter().enumerate() {
        let entity_id = format!("polish_keyword::{}", set.id);
        record_fields(
            tx,
            SyncEntityKind::VocabularySet,
            &entity_id,
            [
                ("name", serde_json::json!(set.name)),
                ("enabled", serde_json::json!(set.enabled)),
                ("keywords", serde_json::json!(set.keywords)),
                ("sortOrder", serde_json::json!(sort_order)),
                ("updatedAt", serde_json::json!(now_ms)),
            ],
            now_ms,
        )?;
    }

    record_removed_ids(
        tx,
        SyncEntityKind::SpeakerProfile,
        previous
            .speaker_profiles
            .iter()
            .map(|record| record.id.as_str()),
        current
            .speaker_profiles
            .iter()
            .map(|record| record.id.as_str()),
        now_ms,
    )?;
    for (sort_order, record) in current.speaker_profiles.iter().enumerate() {
        record_fields(
            tx,
            SyncEntityKind::SpeakerProfile,
            &record.id,
            [
                ("name", serde_json::json!(record.name)),
                ("enabled", serde_json::json!(record.enabled)),
                ("sortOrder", serde_json::json!(sort_order)),
                ("updatedAt", serde_json::json!(now_ms)),
            ],
            now_ms,
        )?;
    }
    Ok(())
}

pub(crate) fn seed_sync_config_baseline_in_transaction(
    tx: &Transaction<'_>,
) -> Result<(), DatabaseError> {
    let library = library::load(tx)?;
    if let Some(state) = load_state_in_transaction(tx)? {
        record_portable_config_sync(tx, &state)?;
        record_library_sync(
            tx,
            &sona_core::config::AppConfigLibrary::default(),
            &library,
            state.updated_at,
        )?;
    } else {
        record_library_sync(
            tx,
            &sona_core::config::AppConfigLibrary::default(),
            &library,
            0,
        )?;
    }
    Ok(())
}

fn record_fields<const N: usize>(
    tx: &Transaction<'_>,
    kind: SyncEntityKind,
    entity_id: &str,
    fields: [(&str, Value); N],
    now_ms: u64,
) -> Result<(), DatabaseError> {
    for (field, value) in fields {
        record_local_field_change_in_transaction(tx, kind, entity_id, field, value, now_ms)?;
    }
    Ok(())
}

fn record_removed_ids<'a>(
    tx: &Transaction<'_>,
    kind: SyncEntityKind,
    previous: impl Iterator<Item = &'a str>,
    current: impl Iterator<Item = &'a str>,
    now_ms: u64,
) -> Result<(), DatabaseError> {
    let current = current.collect::<BTreeSet<_>>();
    for entity_id in previous.filter(|entity_id| !current.contains(entity_id)) {
        record_local_delete_in_transaction(tx, kind, entity_id, now_ms)?;
    }
    Ok(())
}

fn vocabulary_set_ids(library: &sona_core::config::AppConfigLibrary) -> Vec<String> {
    library
        .text_replacement_sets
        .iter()
        .map(|set| format!("text_replacement::{}", set.id))
        .chain(
            library
                .hotword_sets
                .iter()
                .map(|set| format!("hotword::{}", set.id)),
        )
        .chain(
            library
                .polish_keyword_sets
                .iter()
                .map(|set| format!("polish_keyword::{}", set.id)),
        )
        .collect()
}

fn vocabulary_rule_ids(library: &sona_core::config::AppConfigLibrary) -> Vec<String> {
    library
        .text_replacement_sets
        .iter()
        .flat_map(|set| {
            set.rules
                .iter()
                .map(|rule| format!("text_replacement::{}::{}", set.id, rule.id))
        })
        .chain(library.hotword_sets.iter().flat_map(|set| {
            set.rules
                .iter()
                .map(|rule| format!("hotword::{}::{}", set.id, rule.id))
        }))
        .collect()
}

pub(crate) fn clear_setting_in_transaction(
    tx: &Transaction<'_>,
    key: &str,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM app_settings WHERE key = ?1", [key])?;
    Ok(())
}

#[derive(Clone)]
pub struct SqliteConfigStore<D = crate::Database>
where
    D: DatabasePort,
{
    db: Arc<D>,
}

crate::impl_db_repository!(SqliteConfigStore);

pub struct SqliteAppConfigAdapter<D = crate::Database>
where
    D: DatabasePort,
{
    store: SqliteConfigStore<D>,
    clock: Arc<dyn UnixMillisClock>,
}

impl<D> SqliteAppConfigAdapter<D>
where
    D: DatabasePort,
{
    pub fn new(db: Arc<D>, clock: Arc<dyn UnixMillisClock>) -> Self {
        Self {
            store: SqliteConfigStore::new(db),
            clock,
        }
    }

    pub fn load_config(&self) -> Result<Option<Value>, String> {
        self.service().load_config()
    }

    pub fn inspect_state(&self) -> Result<Option<AppConfigRepositorySnapshot>, String> {
        self.service().inspect_state()
    }

    pub fn save_config(&self, config: &Value) -> Result<(), String> {
        self.service().save_config(config)
    }

    pub fn get_setting(&self, key: &str) -> Result<Option<Value>, String> {
        self.service().get_setting(key)
    }

    pub fn set_setting(&self, key: &str, value: &Value) -> Result<(), String> {
        self.service().set_setting(key, value)
    }

    pub fn load_app_config_payload(&self) -> Result<Option<Value>, String> {
        self.service().load_app_config_payload()
    }

    pub fn load_serve_startup_settings(&self) -> Result<Option<ServeStartupSettings>, String> {
        self.service().load_serve_startup_settings()
    }

    fn service(&self) -> AppConfigRepositoryService<'_> {
        AppConfigRepositoryService::new(&self.store, self.clock.as_ref())
    }
}

impl<D> AppConfigStore for SqliteConfigStore<D>
where
    D: DatabasePort,
{
    fn load_state(&self) -> Result<Option<AppConfigStoredState>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(|conn| {
                let tx = conn.unchecked_transaction()?;
                let state = load_state_in_transaction(&tx)?;
                tx.commit()?;
                Ok(state)
            })
            .map_err(|error| error.to_string())
    }

    fn load_base_config_json(&self) -> Result<Option<String>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(app_config::load_base_config_json)
            .map_err(|error| error.to_string())
    }

    fn load_startup_projection(&self) -> Result<Option<AppConfigStartupProjection>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(app_config::load_startup_projection)
            .map_err(|error| error.to_string())
    }

    fn replace_state(&self, state: AppConfigStoredState) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_rw_transaction(|tx| replace_state_in_transaction(tx, &state))
            .map_err(|error| error.to_string())
    }

    fn load_setting_json(&self, key: &str) -> Result<Option<String>, String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_connection(|conn| settings::load(conn, key))
            .map_err(|error| error.to_string())
    }

    fn set_setting_json(&self, key: &str, value_json: String) -> Result<(), String> {
        self.get_db()
            .map_err(|error| error.to_string())?
            .with_rw_transaction(|tx| settings::set(tx, key, &value_json))
            .map_err(|error| error.to_string())
    }
}
