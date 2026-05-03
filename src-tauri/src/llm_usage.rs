use crate::llm::{LlmProvider, LlmUsageCategory, TokenUsage};
use chrono::{DateTime, Duration, Local};
use log::warn;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};
use tauri::{Manager, Runtime};

const ANALYTICS_DIR: &str = "analytics";
const LLM_USAGE_FILE_NAME: &str = "llm-usage.json";
const LLM_USAGE_SCHEMA_VERSION: u64 = 1;
pub(crate) const RECENT_DAILY_WINDOW: i64 = 30;

static LLM_USAGE_STORAGE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageBucket {
    pub(crate) call_count: u64,
    pub(crate) calls_with_usage: u64,
    pub(crate) calls_without_usage: u64,
    pub(crate) prompt_tokens: u64,
    pub(crate) completion_tokens: u64,
    pub(crate) total_tokens: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlmUsageStatsFile {
    pub(crate) schema_version: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_updated_at: Option<String>,
    pub(crate) totals: UsageBucket,
    pub(crate) by_provider: BTreeMap<String, UsageBucket>,
    pub(crate) by_category: BTreeMap<String, UsageBucket>,
    pub(crate) daily: BTreeMap<String, UsageBucket>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageBreakdown {
    pub(crate) key: String,
    pub(crate) stats: UsageBucket,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageTrendPoint {
    pub(crate) date: String,
    #[serde(flatten)]
    pub(crate) stats: UsageBucket,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LlmUsageDashboardStats {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_updated_at: Option<String>,
    pub(crate) totals: UsageBucket,
    pub(crate) by_provider: Vec<UsageBreakdown>,
    pub(crate) by_category: Vec<UsageBreakdown>,
    pub(crate) recent_daily: Vec<UsageTrendPoint>,
}

#[derive(Clone, Debug)]
pub(crate) struct UsageRecord {
    pub(crate) occurred_at: String,
    pub(crate) provider: LlmProvider,
    pub(crate) category: LlmUsageCategory,
    pub(crate) usage: Option<TokenUsage>,
}

#[tauri::command]
pub async fn llm_usage_ensure_storage<R: Runtime>(app: tauri::AppHandle<R>) -> Result<(), String> {
    let app_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || ensure_storage_at_dir(&app_dir))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn llm_usage_read_raw<R: Runtime>(app: tauri::AppHandle<R>) -> Result<String, String> {
    let app_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || read_raw_from_dir(&app_dir))
        .await
        .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn llm_usage_replace_raw<R: Runtime>(
    app: tauri::AppHandle<R>,
    content: String,
) -> Result<(), String> {
    let app_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;

    tauri::async_runtime::spawn_blocking(move || replace_raw_at_dir(&app_dir, &content))
        .await
        .map_err(|error| error.to_string())?
}

pub(crate) fn record_usage<R: Runtime>(
    app: &tauri::AppHandle<R>,
    record: UsageRecord,
) -> Result<(), String> {
    let app_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|error| error.to_string())?;

    record_usage_at_dir(&app_dir, record)
}

pub(crate) fn read_dashboard_stats(app_dir: &Path) -> LlmUsageDashboardStats {
    to_dashboard_stats(&read_stats_from_dir(app_dir))
}

pub(crate) fn read_stats_from_dir(app_dir: &Path) -> LlmUsageStatsFile {
    with_storage_lock(|| Ok(read_stats_unlocked(app_dir))).unwrap_or_else(|error| {
        warn!("[LLM Usage] failed to read stats: {error}");
        create_empty_stats_file()
    })
}

fn ensure_storage_at_dir(app_dir: &Path) -> Result<(), String> {
    with_storage_lock(|| ensure_storage_unlocked(app_dir))
}

fn read_raw_from_dir(app_dir: &Path) -> Result<String, String> {
    with_storage_lock(|| {
        if let Err(error) = ensure_storage_unlocked(app_dir) {
            warn!("[LLM Usage] failed to ensure storage before raw read: {error}");
            return serialize_stats_file(&create_empty_stats_file());
        }

        let path = usage_file_path(app_dir);
        let stats = fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<Value>(&content).ok())
            .filter(Value::is_object)
            .map(|value| normalize_stats_file(&value))
            .unwrap_or_else(create_empty_stats_file);

        serialize_stats_file(&stats)
    })
}

fn replace_raw_at_dir(app_dir: &Path, content: &str) -> Result<(), String> {
    let value: Value = serde_json::from_str(content)
        .map_err(|error| format!("LLM usage content must be a JSON object: {error}"))?;

    if !value.is_object() {
        return Err("LLM usage content must be a JSON object.".to_string());
    }

    let stats = normalize_stats_file(&value);

    with_storage_lock(|| {
        ensure_usage_dir_unlocked(app_dir)?;
        write_stats_unlocked(app_dir, &stats)
    })
}

fn record_usage_at_dir(app_dir: &Path, record: UsageRecord) -> Result<(), String> {
    with_storage_lock(|| {
        ensure_usage_dir_unlocked(app_dir)?;
        let mut stats = read_stats_unlocked(app_dir);
        let bucket = usage_bucket_for_record(record.usage.as_ref());
        let provider_key = serde_key(record.provider);
        let category_key = serde_key(record.category);
        let date_key = local_date_key(&record.occurred_at);

        if stats.started_at.is_none() {
            stats.started_at = Some(record.occurred_at.clone());
        }
        stats.last_updated_at = Some(record.occurred_at);

        add_usage_bucket(&mut stats.totals, &bucket);
        add_usage_bucket(
            upsert_bucket(&mut stats.by_provider, &provider_key),
            &bucket,
        );
        add_usage_bucket(
            upsert_bucket(&mut stats.by_category, &category_key),
            &bucket,
        );
        add_usage_bucket(upsert_bucket(&mut stats.daily, &date_key), &bucket);

        write_stats_unlocked(app_dir, &stats)
    })
}

fn storage_lock() -> &'static Mutex<()> {
    LLM_USAGE_STORAGE_LOCK.get_or_init(|| Mutex::new(()))
}

fn with_storage_lock<T>(operation: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
    let _guard = storage_lock()
        .lock()
        .map_err(|_| "LLM usage storage lock was poisoned".to_string())?;
    operation()
}

fn analytics_dir_path(app_dir: &Path) -> PathBuf {
    app_dir.join(ANALYTICS_DIR)
}

fn usage_file_path(app_dir: &Path) -> PathBuf {
    analytics_dir_path(app_dir).join(LLM_USAGE_FILE_NAME)
}

fn ensure_usage_dir_unlocked(app_dir: &Path) -> Result<(), String> {
    fs::create_dir_all(analytics_dir_path(app_dir)).map_err(|error| error.to_string())
}

fn ensure_storage_unlocked(app_dir: &Path) -> Result<(), String> {
    ensure_usage_dir_unlocked(app_dir)?;
    let path = usage_file_path(app_dir);

    if !path.exists() {
        write_stats_unlocked(app_dir, &create_empty_stats_file())?;
    }

    Ok(())
}

fn read_stats_unlocked(app_dir: &Path) -> LlmUsageStatsFile {
    let path = usage_file_path(app_dir);
    fs::read_to_string(&path)
        .ok()
        .and_then(|content| serde_json::from_str::<Value>(&content).ok())
        .filter(Value::is_object)
        .map(|value| normalize_stats_file(&value))
        .unwrap_or_else(create_empty_stats_file)
}

fn write_stats_unlocked(app_dir: &Path, stats: &LlmUsageStatsFile) -> Result<(), String> {
    fs::write(usage_file_path(app_dir), serialize_stats_file(stats)?)
        .map_err(|error| error.to_string())
}

fn serialize_stats_file(stats: &LlmUsageStatsFile) -> Result<String, String> {
    serde_json::to_string_pretty(stats).map_err(|error| error.to_string())
}

fn create_empty_stats_file() -> LlmUsageStatsFile {
    LlmUsageStatsFile {
        schema_version: LLM_USAGE_SCHEMA_VERSION,
        started_at: None,
        last_updated_at: None,
        totals: UsageBucket::default(),
        by_provider: BTreeMap::new(),
        by_category: BTreeMap::new(),
        daily: BTreeMap::new(),
    }
}

fn normalize_stats_file(input: &Value) -> LlmUsageStatsFile {
    let Some(source) = input.as_object() else {
        return create_empty_stats_file();
    };

    LlmUsageStatsFile {
        schema_version: normalize_count(source.get("schemaVersion")).max(LLM_USAGE_SCHEMA_VERSION),
        started_at: normalize_optional_string(source.get("startedAt")),
        last_updated_at: normalize_optional_string(source.get("lastUpdatedAt")),
        totals: normalize_usage_bucket(source.get("totals")),
        by_provider: normalize_bucket_map(source.get("byProvider")),
        by_category: normalize_bucket_map(source.get("byCategory")),
        daily: normalize_bucket_map(source.get("daily")),
    }
}

fn normalize_bucket_map(input: Option<&Value>) -> BTreeMap<String, UsageBucket> {
    input
        .and_then(Value::as_object)
        .map(|items| {
            items
                .iter()
                .map(|(key, value)| (key.clone(), normalize_usage_bucket(Some(value))))
                .collect()
        })
        .unwrap_or_default()
}

fn normalize_usage_bucket(input: Option<&Value>) -> UsageBucket {
    let source = input.and_then(Value::as_object);
    UsageBucket {
        call_count: normalize_count(source.and_then(|object| object.get("callCount"))),
        calls_with_usage: normalize_count(source.and_then(|object| object.get("callsWithUsage"))),
        calls_without_usage: normalize_count(
            source.and_then(|object| object.get("callsWithoutUsage")),
        ),
        prompt_tokens: normalize_count(source.and_then(|object| object.get("promptTokens"))),
        completion_tokens: normalize_count(
            source.and_then(|object| object.get("completionTokens")),
        ),
        total_tokens: normalize_count(source.and_then(|object| object.get("totalTokens"))),
    }
}

fn normalize_count(input: Option<&Value>) -> u64 {
    match input.and_then(Value::as_f64) {
        Some(value) if value.is_finite() && value > 0.0 => value.round() as u64,
        _ => 0,
    }
}

fn normalize_optional_string(input: Option<&Value>) -> Option<String> {
    let value = input?.as_str()?.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn usage_bucket_for_record(usage: Option<&TokenUsage>) -> UsageBucket {
    let prompt_tokens = usage.map(|item| item.prompt_tokens as u64).unwrap_or(0);
    let completion_tokens = usage.map(|item| item.completion_tokens as u64).unwrap_or(0);
    let total_tokens = usage
        .map(|item| item.total_tokens as u64)
        .filter(|value| *value > 0)
        .unwrap_or_else(|| prompt_tokens.saturating_add(completion_tokens));

    if prompt_tokens == 0 && completion_tokens == 0 && total_tokens == 0 {
        return UsageBucket {
            call_count: 1,
            calls_without_usage: 1,
            ..UsageBucket::default()
        };
    }

    UsageBucket {
        call_count: 1,
        calls_with_usage: 1,
        prompt_tokens,
        completion_tokens,
        total_tokens,
        ..UsageBucket::default()
    }
}

fn add_usage_bucket(target: &mut UsageBucket, usage: &UsageBucket) {
    target.call_count = target.call_count.saturating_add(usage.call_count);
    target.calls_with_usage = target
        .calls_with_usage
        .saturating_add(usage.calls_with_usage);
    target.calls_without_usage = target
        .calls_without_usage
        .saturating_add(usage.calls_without_usage);
    target.prompt_tokens = target.prompt_tokens.saturating_add(usage.prompt_tokens);
    target.completion_tokens = target
        .completion_tokens
        .saturating_add(usage.completion_tokens);
    target.total_tokens = target.total_tokens.saturating_add(usage.total_tokens);
}

fn upsert_bucket<'a>(
    collection: &'a mut BTreeMap<String, UsageBucket>,
    key: &str,
) -> &'a mut UsageBucket {
    collection.entry(key.to_string()).or_default()
}

fn serde_key<T: Serialize>(value: T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|value| value.as_str().map(ToString::to_string))
        .unwrap_or_else(|| "unknown".to_string())
}

fn local_date_key(occurred_at: &str) -> String {
    DateTime::parse_from_rfc3339(occurred_at)
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| Local::now().format("%Y-%m-%d").to_string())
}

fn to_dashboard_stats(stats: &LlmUsageStatsFile) -> LlmUsageDashboardStats {
    LlmUsageDashboardStats {
        started_at: stats.started_at.clone(),
        last_updated_at: stats.last_updated_at.clone(),
        totals: stats.totals.clone(),
        by_provider: to_sorted_breakdown(&stats.by_provider),
        by_category: to_sorted_breakdown(&stats.by_category),
        recent_daily: build_recent_daily_trend(&stats.daily),
    }
}

fn to_sorted_breakdown(collection: &BTreeMap<String, UsageBucket>) -> Vec<UsageBreakdown> {
    let mut breakdowns = collection
        .iter()
        .map(|(key, stats)| UsageBreakdown {
            key: key.clone(),
            stats: stats.clone(),
        })
        .filter(|breakdown| breakdown.stats.call_count > 0)
        .collect::<Vec<_>>();

    breakdowns.sort_by(|left, right| {
        right
            .stats
            .total_tokens
            .cmp(&left.stats.total_tokens)
            .then_with(|| right.stats.call_count.cmp(&left.stats.call_count))
            .then_with(|| left.key.cmp(&right.key))
    });
    breakdowns
}

fn build_recent_daily_trend(daily: &BTreeMap<String, UsageBucket>) -> Vec<UsageTrendPoint> {
    let today = Local::now().date_naive();
    (0..RECENT_DAILY_WINDOW)
        .rev()
        .map(|offset| {
            let key = (today - Duration::days(offset))
                .format("%Y-%m-%d")
                .to_string();
            UsageTrendPoint {
                stats: daily.get(&key).cloned().unwrap_or_default(),
                date: key,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::fs;

    #[test]
    fn read_raw_returns_valid_empty_stats_when_storage_is_missing() {
        let temp_dir = tempfile::tempdir().expect("tempdir");

        let content = read_raw_from_dir(temp_dir.path()).expect("raw content");
        let parsed: Value = serde_json::from_str(&content).expect("valid json");

        assert_eq!(parsed["schemaVersion"], 1);
        assert_eq!(parsed["totals"]["callCount"], 0);
        assert!(parsed["byProvider"]
            .as_object()
            .expect("providers")
            .is_empty());
        assert!(temp_dir.path().join("analytics/llm-usage.json").exists());
    }

    #[test]
    fn replace_raw_rejects_json_arrays_before_writing() {
        let temp_dir = tempfile::tempdir().expect("tempdir");

        let error = replace_raw_at_dir(temp_dir.path(), "[]").expect_err("array should fail");

        assert!(error.contains("JSON object"));
        assert!(!temp_dir.path().join("analytics/llm-usage.json").exists());
    }

    #[test]
    fn replace_raw_normalizes_legacy_object_shape_for_import() {
        let temp_dir = tempfile::tempdir().expect("tempdir");

        replace_raw_at_dir(
            temp_dir.path(),
            r#"{"schemaVersion":1,"totals":{"callCount":2.4},"byProvider":{"open_ai":{"callCount":1,"totalTokens":9}}}"#,
        )
        .expect("replace raw");

        let content =
            fs::read_to_string(temp_dir.path().join("analytics/llm-usage.json")).expect("file");
        let parsed: Value = serde_json::from_str(&content).expect("valid json");

        assert_eq!(parsed["totals"]["callCount"], 2);
        assert_eq!(parsed["byProvider"]["open_ai"]["totalTokens"], 9);
        assert_eq!(
            parsed["byCategory"].as_object().expect("categories").len(),
            0
        );
        assert_eq!(parsed["daily"].as_object().expect("daily").len(), 0);
    }

    #[test]
    fn record_usage_updates_totals_breakdowns_and_daily_buckets() {
        let temp_dir = tempfile::tempdir().expect("tempdir");

        record_usage_at_dir(
            temp_dir.path(),
            UsageRecord {
                occurred_at: "2026-05-03T08:30:00Z".to_string(),
                provider: LlmProvider::OpenAi,
                category: LlmUsageCategory::Summary,
                usage: Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 0,
                }),
            },
        )
        .expect("record with usage");
        record_usage_at_dir(
            temp_dir.path(),
            UsageRecord {
                occurred_at: "2026-05-03T08:31:00Z".to_string(),
                provider: LlmProvider::GoogleTranslateFree,
                category: LlmUsageCategory::Translation,
                usage: None,
            },
        )
        .expect("record without usage");

        let stats = read_stats_from_dir(temp_dir.path());

        assert_eq!(stats.started_at.as_deref(), Some("2026-05-03T08:30:00Z"));
        assert_eq!(
            stats.last_updated_at.as_deref(),
            Some("2026-05-03T08:31:00Z")
        );
        assert_eq!(stats.totals.call_count, 2);
        assert_eq!(stats.totals.calls_with_usage, 1);
        assert_eq!(stats.totals.calls_without_usage, 1);
        assert_eq!(stats.totals.prompt_tokens, 10);
        assert_eq!(stats.totals.completion_tokens, 5);
        assert_eq!(stats.totals.total_tokens, 15);
        assert_eq!(stats.by_provider["open_ai"].total_tokens, 15);
        assert_eq!(
            stats.by_provider["google_translate_free"].calls_without_usage,
            1
        );
        assert_eq!(stats.by_category["summary"].calls_with_usage, 1);
        assert_eq!(stats.by_category["translation"].calls_without_usage, 1);
        assert_eq!(stats.daily["2026-05-03"].call_count, 2);
    }

    #[test]
    fn dashboard_stats_sort_breakdowns_and_include_recent_window() {
        let temp_dir = tempfile::tempdir().expect("tempdir");

        replace_raw_at_dir(
            temp_dir.path(),
            r#"{
                "startedAt":"2026-05-01T00:00:00Z",
                "lastUpdatedAt":"2026-05-03T00:00:00Z",
                "totals":{"callCount":2,"totalTokens":20},
                "byProvider":{
                    "ollama":{"callCount":1,"totalTokens":5},
                    "open_ai":{"callCount":1,"totalTokens":15}
                },
                "byCategory":{"summary":{"callCount":2,"totalTokens":20}},
                "daily":{}
            }"#,
        )
        .expect("replace raw");

        let stats = read_dashboard_stats(temp_dir.path());

        assert_eq!(stats.started_at.as_deref(), Some("2026-05-01T00:00:00Z"));
        assert_eq!(stats.by_provider[0].key, "open_ai");
        assert_eq!(stats.by_provider[1].key, "ollama");
        assert_eq!(stats.by_category[0].key, "summary");
        assert_eq!(stats.recent_daily.len(), RECENT_DAILY_WINDOW as usize);
    }
}
