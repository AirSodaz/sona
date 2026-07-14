use crate::{Database, DatabaseError};
use chrono::{DateTime, Local, NaiveDate};
use serde_json::Value;
use sona_core::llm::usage::{LlmUsageDashboardStats, LlmUsageStatsFile, UsageBucket, UsageRecord};
use std::{collections::BTreeMap, sync::Arc};

pub const MAX_BACKUP_ANALYTICS_ROWS: usize = 100_000;

#[derive(Clone, Debug)]
pub(crate) struct PreparedLlmUsageRows {
    rows: Vec<PreparedLlmUsageRow>,
}

#[derive(Clone, Debug)]
struct PreparedLlmUsageRow {
    occurred_at: Arc<str>,
    provider: Arc<str>,
    category: Arc<str>,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
}

pub(crate) fn parse_raw(content: &str) -> Result<PreparedLlmUsageRows, String> {
    let value: Value = serde_json::from_str(content)
        .map_err(|error| format!("Invalid LLM usage content: {error}"))?;
    let rows = match value {
        Value::Array(rows) => {
            ensure_analytics_row_count(rows.len())?;
            rows.into_iter()
                .enumerate()
                .map(|(index, row)| parse_usage_row(&row, index))
                .collect::<Result<Vec<_>, _>>()?
        }
        Value::Object(object) => legacy_stats_object_to_rows(object)?,
        _ => return Err("LLM usage content must be a JSON array or object.".to_string()),
    };

    Ok(PreparedLlmUsageRows { rows })
}

fn ensure_analytics_row_count(row_count: usize) -> Result<(), String> {
    if row_count <= MAX_BACKUP_ANALYTICS_ROWS {
        return Ok(());
    }
    Err(format!(
        "LLM usage content exceeds the {MAX_BACKUP_ANALYTICS_ROWS} row limit."
    ))
}

fn parse_usage_row(row: &Value, index: usize) -> Result<PreparedLlmUsageRow, String> {
    let row = row
        .as_object()
        .ok_or_else(|| format!("LLM usage row at index {index} must be an object."))?;
    let string_field = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .map(Arc::<str>::from)
            .ok_or_else(|| format!("LLM usage row at index {index} has invalid {key}."))
    };
    let token_field = |key: &str| {
        row.get(key)
            .and_then(Value::as_i64)
            .filter(|value| *value >= 0)
            .ok_or_else(|| format!("LLM usage row at index {index} has invalid {key}."))
    };
    Ok(PreparedLlmUsageRow {
        occurred_at: string_field("occurredAt")?,
        provider: string_field("provider")?,
        category: string_field("category")?,
        prompt_tokens: token_field("promptTokens")?,
        completion_tokens: token_field("completionTokens")?,
        total_tokens: token_field("totalTokens")?,
    })
}

fn read_raw_from_connection(conn: &rusqlite::Connection) -> Result<String, DatabaseError> {
    let mut stmt = conn.prepare_cached(
        "SELECT occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens
         FROM analytics.llm_usage
         ORDER BY occurred_at, id",
    )?;
    let rows = stmt.query_map([], |row| {
        let occurred_at: String = row.get(0)?;
        let provider: String = row.get(1)?;
        let category: String = row.get(2)?;
        let prompt_tokens: i64 = row.get(3)?;
        let completion_tokens: i64 = row.get(4)?;
        let total_tokens: i64 = row.get(5)?;
        Ok(serde_json::json!({
            "occurredAt": occurred_at,
            "provider": provider,
            "category": category,
            "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
        }))
    })?;
    let items = rows.collect::<Result<Vec<_>, _>>()?;
    Ok(serde_json::to_string(&items)?)
}

pub(crate) fn read_raw_in_transaction(
    tx: &rusqlite::Transaction<'_>,
) -> Result<String, DatabaseError> {
    read_raw_from_connection(tx)
}

pub(crate) fn replace_raw_in_transaction(
    tx: &rusqlite::Transaction<'_>,
    prepared: &PreparedLlmUsageRows,
) -> Result<(), DatabaseError> {
    tx.execute("DELETE FROM analytics.llm_usage", [])?;
    let mut stmt = tx.prepare_cached(
        "INSERT INTO analytics.llm_usage (
            occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;
    for row in &prepared.rows {
        stmt.execute(rusqlite::params![
            row.occurred_at.as_ref(),
            row.provider.as_ref(),
            row.category.as_ref(),
            row.prompt_tokens,
            row.completion_tokens,
            row.total_tokens,
        ])?;
    }
    Ok(())
}

pub fn record_usage(db: &Database, record: &UsageRecord) -> Result<(), DatabaseError> {
    let prompt_tokens = record
        .usage
        .as_ref()
        .map(|u| u.prompt_tokens as i64)
        .unwrap_or(0);
    let completion_tokens = record
        .usage
        .as_ref()
        .map(|u| u.completion_tokens as i64)
        .unwrap_or(0);
    let total_tokens = record
        .usage
        .as_ref()
        .map(|u| {
            if u.total_tokens > 0 {
                u.total_tokens as i64
            } else {
                u.prompt_tokens as i64 + u.completion_tokens as i64
            }
        })
        .unwrap_or(0);

    let provider = &record.provider;
    let category = serde_json::to_string(&record.category)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string();

    db.with_write_connection(|conn| {
        conn.execute(
            "INSERT INTO analytics.llm_usage (occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                record.occurred_at,
                provider,
                category,
                prompt_tokens,
                completion_tokens,
                total_tokens,
            ],
        )?;
        Ok(())
    })
}

pub fn read_stats(db: &Database) -> Result<LlmUsageStatsFile, DatabaseError> {
    let rows: Vec<(String, String, String, i64, i64, i64)> = db.with_connection(|conn| {
        let mut stmt = conn.prepare_cached(
            "SELECT occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens FROM analytics.llm_usage ORDER BY occurred_at"
        )?;
        let rows = stmt.query_map([], |row| {
            let occurred_at: String = row.get(0)?;
            let provider: String = row.get(1)?;
            let category: String = row.get(2)?;
            let prompt_tokens: i64 = row.get(3)?;
            let completion_tokens: i64 = row.get(4)?;
            let total_tokens: i64 = row.get(5)?;
            Ok((occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens))
        })?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    })?;

    if rows.is_empty() {
        return Ok(create_empty_stats());
    }

    let started_at = rows.first().map(|r| r.0.clone());
    let last_updated_at = rows.last().map(|r| r.0.clone());

    let mut totals = UsageBucket::default();
    let mut by_provider: BTreeMap<String, UsageBucket> = BTreeMap::new();
    let mut by_category: BTreeMap<String, UsageBucket> = BTreeMap::new();
    let mut daily: BTreeMap<String, UsageBucket> = BTreeMap::new();

    for (_occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens) in &rows
    {
        let has_usage = *prompt_tokens > 0 || *completion_tokens > 0 || *total_tokens > 0;

        let bucket = if has_usage {
            UsageBucket {
                call_count: 1,
                calls_with_usage: 1,
                prompt_tokens: *prompt_tokens as u64,
                completion_tokens: *completion_tokens as u64,
                total_tokens: *total_tokens as u64,
                ..Default::default()
            }
        } else {
            UsageBucket {
                call_count: 1,
                calls_without_usage: 1,
                ..Default::default()
            }
        };

        add_usage_bucket(&mut totals, &bucket);
        add_usage_bucket(by_provider.entry(provider.clone()).or_default(), &bucket);
        add_usage_bucket(by_category.entry(category.clone()).or_default(), &bucket);

        let date_key = local_date_key_for_occurred_at;
        let date = date_key(_occurred_at);
        add_usage_bucket(daily.entry(date).or_default(), &bucket);
    }

    Ok(LlmUsageStatsFile {
        schema_version: 1,
        started_at,
        last_updated_at,
        totals,
        by_provider,
        by_category,
        daily,
    })
}

pub fn read_raw(db: &Database) -> Result<String, DatabaseError> {
    db.with_connection(read_raw_from_connection)
}

pub fn replace_raw(db: &Database, content: &str) -> Result<(), DatabaseError> {
    let prepared = parse_raw(content).map_err(DatabaseError::Internal)?;
    db.with_transaction(|tx| replace_raw_in_transaction(tx, &prepared))
}

fn legacy_stats_object_to_rows(
    object: serde_json::Map<String, Value>,
) -> Result<Vec<PreparedLlmUsageRow>, String> {
    let stats: LlmUsageStatsFile = serde_json::from_value(Value::Object(object))
        .map_err(|error| format!("Invalid legacy LLM usage stats: {error}"))?;
    if stats.schema_version != 1 {
        return Err(format!(
            "Unsupported legacy LLM usage schema version: {}",
            stats.schema_version
        ));
    }
    validate_legacy_timestamp("startedAt", stats.started_at.as_deref())?;
    validate_legacy_timestamp("lastUpdatedAt", stats.last_updated_at.as_deref())?;
    validate_bucket("totals", &stats.totals)?;
    let provider_rows = validate_bucket_map("byProvider", &stats.by_provider, &stats.totals)?;
    let category_rows = validate_bucket_map("byCategory", &stats.by_category, &stats.totals)?;
    let daily_rows = validate_daily_buckets(&stats.daily, &stats.totals)?;
    let fallback_occurred_at: Arc<str> = stats
        .last_updated_at
        .as_deref()
        .or(stats.started_at.as_deref())
        .unwrap_or("1970-01-01T00:00:00Z")
        .into();
    let migrated: Arc<str> = "migrated".into();

    if provider_rows > 0 {
        return rows_from_bucket_map(
            &stats.by_provider,
            provider_rows,
            |key| Arc::<str>::from(key),
            |_| Arc::clone(&migrated),
            |_| Arc::clone(&fallback_occurred_at),
        );
    }
    if category_rows > 0 {
        return rows_from_bucket_map(
            &stats.by_category,
            category_rows,
            |_| Arc::clone(&migrated),
            |key| Arc::<str>::from(key),
            |_| Arc::clone(&fallback_occurred_at),
        );
    }
    if daily_rows > 0 {
        return rows_from_bucket_map(
            &stats.daily,
            daily_rows,
            |_| Arc::clone(&migrated),
            |_| Arc::clone(&migrated),
            |key| format!("{key}T00:00:00Z").into(),
        );
    }

    let total_rows = bucket_row_count("totals", &stats.totals)?;
    let mut rows = Vec::with_capacity(total_rows);
    let total: Arc<str> = "total".into();
    append_bucket_rows(
        &mut rows,
        &fallback_occurred_at,
        &total,
        &migrated,
        &stats.totals,
    )?;
    Ok(rows)
}

fn validate_legacy_timestamp(label: &str, value: Option<&str>) -> Result<(), String> {
    let Some(value) = value else {
        return Ok(());
    };
    DateTime::parse_from_rfc3339(value)
        .map(|_| ())
        .map_err(|error| format!("Legacy LLM usage {label} is invalid: {error}"))
}

fn validate_bucket(label: &str, bucket: &UsageBucket) -> Result<(), String> {
    let accounted_calls = bucket
        .calls_with_usage
        .checked_add(bucket.calls_without_usage)
        .ok_or_else(|| format!("Legacy LLM usage {label} call counters overflow."))?;
    if accounted_calls != bucket.call_count {
        return Err(format!(
            "Legacy LLM usage {label} call counters are inconsistent."
        ));
    }
    let component_total = bucket
        .prompt_tokens
        .checked_add(bucket.completion_tokens)
        .ok_or_else(|| format!("Legacy LLM usage {label} token counters overflow."))?;
    if bucket.total_tokens < component_total {
        return Err(format!(
            "Legacy LLM usage {label} token counters are inconsistent."
        ));
    }
    let has_tokens =
        bucket.prompt_tokens > 0 || bucket.completion_tokens > 0 || bucket.total_tokens > 0;
    if has_tokens != (bucket.calls_with_usage > 0) {
        return Err(format!(
            "Legacy LLM usage {label} usage counters are inconsistent."
        ));
    }
    bucket_row_count(label, bucket).map(|_| ())
}

fn bucket_row_count(label: &str, bucket: &UsageBucket) -> Result<usize, String> {
    let row_count = usize::try_from(bucket.call_count).map_err(|_| {
        format!("Legacy LLM usage {label} call count is not representable as a row count.")
    })?;
    ensure_analytics_row_count(row_count)?;
    Ok(row_count)
}

fn validate_bucket_map(
    label: &str,
    buckets: &BTreeMap<String, UsageBucket>,
    totals: &UsageBucket,
) -> Result<usize, String> {
    let mut row_count = 0usize;
    let mut aggregate = UsageBucket::default();
    for (key, bucket) in buckets {
        validate_bucket(&format!("{label}.{key}"), bucket)?;
        checked_add_bucket(&mut aggregate, bucket, label)?;
        row_count = row_count
            .checked_add(bucket_row_count(&format!("{label}.{key}"), bucket)?)
            .ok_or_else(|| format!("Legacy LLM usage {label} row count overflows."))?;
        ensure_analytics_row_count(row_count)?;
    }
    if !buckets.is_empty() && aggregate != *totals {
        return Err(format!(
            "Legacy LLM usage {label} aggregate disagrees with totals."
        ));
    }
    Ok(row_count)
}

fn checked_add_bucket(
    aggregate: &mut UsageBucket,
    bucket: &UsageBucket,
    label: &str,
) -> Result<(), String> {
    let checked_add = |left: u64, right: u64, field: &str| {
        left.checked_add(right)
            .ok_or_else(|| format!("Legacy LLM usage {label} aggregate {field} overflows."))
    };
    aggregate.call_count = checked_add(aggregate.call_count, bucket.call_count, "callCount")?;
    aggregate.calls_with_usage = checked_add(
        aggregate.calls_with_usage,
        bucket.calls_with_usage,
        "callsWithUsage",
    )?;
    aggregate.calls_without_usage = checked_add(
        aggregate.calls_without_usage,
        bucket.calls_without_usage,
        "callsWithoutUsage",
    )?;
    aggregate.prompt_tokens = checked_add(
        aggregate.prompt_tokens,
        bucket.prompt_tokens,
        "promptTokens",
    )?;
    aggregate.completion_tokens = checked_add(
        aggregate.completion_tokens,
        bucket.completion_tokens,
        "completionTokens",
    )?;
    aggregate.total_tokens =
        checked_add(aggregate.total_tokens, bucket.total_tokens, "totalTokens")?;
    Ok(())
}

fn validate_daily_buckets(
    buckets: &BTreeMap<String, UsageBucket>,
    totals: &UsageBucket,
) -> Result<usize, String> {
    for key in buckets.keys() {
        NaiveDate::parse_from_str(key, "%Y-%m-%d")
            .map_err(|error| format!("Legacy LLM usage daily key is invalid: {error}"))?;
    }
    validate_bucket_map("daily", buckets, totals)
}

fn rows_from_bucket_map(
    buckets: &BTreeMap<String, UsageBucket>,
    row_count: usize,
    provider_for_key: impl Fn(&str) -> Arc<str>,
    category_for_key: impl Fn(&str) -> Arc<str>,
    occurred_at_for_key: impl Fn(&str) -> Arc<str>,
) -> Result<Vec<PreparedLlmUsageRow>, String> {
    let mut rows = Vec::with_capacity(row_count);
    for (key, bucket) in buckets {
        let occurred_at = occurred_at_for_key(key);
        let provider = provider_for_key(key);
        let category = category_for_key(key);
        append_bucket_rows(&mut rows, &occurred_at, &provider, &category, bucket)?;
    }
    Ok(rows)
}

fn append_bucket_rows(
    rows: &mut Vec<PreparedLlmUsageRow>,
    occurred_at: &Arc<str>,
    provider: &Arc<str>,
    category: &Arc<str>,
    bucket: &UsageBucket,
) -> Result<(), String> {
    for index in 0..bucket.calls_with_usage {
        rows.push(PreparedLlmUsageRow {
            occurred_at: Arc::clone(occurred_at),
            provider: Arc::clone(provider),
            category: Arc::clone(category),
            prompt_tokens: checked_split_to_i64(
                bucket.prompt_tokens,
                bucket.calls_with_usage,
                index,
                "promptTokens",
            )?,
            completion_tokens: checked_split_to_i64(
                bucket.completion_tokens,
                bucket.calls_with_usage,
                index,
                "completionTokens",
            )?,
            total_tokens: checked_split_to_i64(
                bucket.total_tokens,
                bucket.calls_with_usage,
                index,
                "totalTokens",
            )?,
        });
    }
    for _ in 0..bucket.calls_without_usage {
        rows.push(PreparedLlmUsageRow {
            occurred_at: Arc::clone(occurred_at),
            provider: Arc::clone(provider),
            category: Arc::clone(category),
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        });
    }
    Ok(())
}

fn checked_split_to_i64(total: u64, parts: u64, index: u64, label: &str) -> Result<i64, String> {
    i64::try_from(split_u64(total, parts, index))
        .map_err(|_| format!("Legacy LLM usage {label} cannot be represented in SQLite storage."))
}

fn split_u64(total: u64, parts: u64, index: u64) -> u64 {
    if parts == 0 {
        return 0;
    }
    let base = total / parts;
    if index < total % parts {
        base + 1
    } else {
        base
    }
}

pub fn read_dashboard_stats(db: &Database) -> Result<LlmUsageDashboardStats, DatabaseError> {
    let stats = read_stats(db)?;
    Ok(sona_core::llm::usage::to_dashboard_stats_at(
        &stats,
        Local::now().date_naive(),
    ))
}

fn create_empty_stats() -> LlmUsageStatsFile {
    LlmUsageStatsFile {
        schema_version: 1,
        started_at: None,
        last_updated_at: None,
        totals: UsageBucket::default(),
        by_provider: BTreeMap::new(),
        by_category: BTreeMap::new(),
        daily: BTreeMap::new(),
    }
}

fn local_date_key_for_occurred_at(occurred_at: &str) -> String {
    DateTime::parse_from_rfc3339(occurred_at)
        .map(|date| date.with_timezone(&Local).format("%Y-%m-%d").to_string())
        .unwrap_or_else(|_| Local::now().format("%Y-%m-%d").to_string())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;
    use serde_json::json;
    use sona_core::llm::usage::{LlmUsageCategory, TokenUsage, UsageRecord};

    fn legacy_bucket(
        call_count: Value,
        calls_with_usage: Value,
        calls_without_usage: Value,
        prompt_tokens: Value,
        completion_tokens: Value,
        total_tokens: Value,
    ) -> Value {
        json!({
            "callCount": call_count,
            "callsWithUsage": calls_with_usage,
            "callsWithoutUsage": calls_without_usage,
            "promptTokens": prompt_tokens,
            "completionTokens": completion_tokens,
            "totalTokens": total_tokens,
        })
    }

    fn valid_legacy_bucket() -> Value {
        legacy_bucket(json!(2), json!(1), json!(1), json!(10), json!(5), json!(15))
    }

    fn legacy_stats(totals: Value, by_provider: Value, by_category: Value, daily: Value) -> Value {
        json!({
            "schemaVersion": 1,
            "startedAt": "2026-05-01T00:00:00Z",
            "lastUpdatedAt": "2026-05-02T00:00:00Z",
            "totals": totals,
            "byProvider": by_provider,
            "byCategory": by_category,
            "daily": daily,
        })
    }

    #[test]
    fn test_llm_usage_record_and_read_stats() {
        let db = Database::open_in_memory().unwrap();

        record_usage(
            &db,
            &UsageRecord {
                occurred_at: "2026-05-03T08:30:00Z".to_string(),
                provider: "open_ai".to_string(),
                category: LlmUsageCategory::Summary,
                usage: Some(TokenUsage {
                    prompt_tokens: 10,
                    completion_tokens: 5,
                    total_tokens: 0,
                }),
            },
        )
        .unwrap();

        record_usage(
            &db,
            &UsageRecord {
                occurred_at: "2026-05-03T08:31:00Z".to_string(),
                provider: "google_translate_free".to_string(),
                category: LlmUsageCategory::Translation,
                usage: None,
            },
        )
        .unwrap();

        let stats = read_stats(&db).unwrap();
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
    }

    #[test]
    fn test_llm_usage_read_raw() {
        let db = Database::open_in_memory().unwrap();

        record_usage(
            &db,
            &UsageRecord {
                occurred_at: "2026-05-03T08:30:00Z".to_string(),
                provider: "test_provider".to_string(),
                category: LlmUsageCategory::Generic,
                usage: Some(TokenUsage {
                    prompt_tokens: 100,
                    completion_tokens: 50,
                    total_tokens: 150,
                }),
            },
        )
        .unwrap();

        let raw = read_raw(&db).unwrap();
        let parsed: Vec<Value> = serde_json::from_str(&raw).unwrap();
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0]["provider"], "test_provider");
        assert_eq!(parsed[0]["totalTokens"], 150);
    }

    #[test]
    fn test_llm_usage_replace_raw() {
        let db = Database::open_in_memory().unwrap();

        let data = json!([
            {"occurredAt": "2026-05-01T00:00:00Z", "provider": "p1", "category": "summary", "promptTokens": 10, "completionTokens": 5, "totalTokens": 15},
            {"occurredAt": "2026-05-02T00:00:00Z", "provider": "p2", "category": "translation", "promptTokens": 0, "completionTokens": 0, "totalTokens": 0}
        ]);

        replace_raw(&db, &data.to_string()).unwrap();

        let stats = read_stats(&db).unwrap();
        assert_eq!(stats.totals.call_count, 2);
        assert_eq!(stats.totals.calls_with_usage, 1);
        assert_eq!(stats.totals.calls_without_usage, 1);
    }

    #[test]
    fn test_llm_usage_replace_raw_accepts_legacy_stats_object() {
        let db = Database::open_in_memory().unwrap();

        let bucket = legacy_bucket(
            json!(2),
            json!(1),
            json!(1),
            json!(20),
            json!(22),
            json!(42),
        );
        replace_raw(
            &db,
            &legacy_stats(
                bucket.clone(),
                json!({"open_ai": bucket}),
                json!({}),
                json!({}),
            )
            .to_string(),
        )
        .unwrap();

        let stats = read_stats(&db).unwrap();
        assert_eq!(stats.totals.call_count, 2);
        assert_eq!(stats.totals.total_tokens, 42);
        assert_eq!(
            stats
                .by_provider
                .get("open_ai")
                .map(|bucket| bucket.total_tokens),
            Some(42)
        );
    }

    #[test]
    fn legacy_empty_stats_object_is_valid() {
        let empty = legacy_bucket(json!(0), json!(0), json!(0), json!(0), json!(0), json!(0));
        let prepared =
            parse_raw(&legacy_stats(empty, json!({}), json!({}), json!({})).to_string()).unwrap();

        assert!(prepared.rows.is_empty());
    }

    #[test]
    fn legacy_stats_reject_malformed_shapes_counters_and_timestamps() {
        let valid = valid_legacy_bucket();
        let malformed = [
            json!({}),
            legacy_stats(valid.clone(), json!([]), json!({}), json!({})),
            legacy_stats(
                valid.clone(),
                json!({"open_ai": null}),
                json!({}),
                json!({}),
            ),
            legacy_stats(
                valid.clone(),
                json!({"open_ai": {"callCount": 1}}),
                json!({}),
                json!({}),
            ),
            legacy_stats(
                legacy_bucket(json!(1), json!(-1), json!(2), json!(0), json!(0), json!(0)),
                json!({}),
                json!({}),
                json!({}),
            ),
            legacy_stats(
                legacy_bucket(json!(1.5), json!(1), json!(0), json!(1), json!(0), json!(1)),
                json!({}),
                json!({}),
                json!({}),
            ),
            legacy_stats(
                legacy_bucket(json!(1), json!(1), json!(1), json!(1), json!(0), json!(1)),
                json!({}),
                json!({}),
                json!({}),
            ),
            legacy_stats(
                legacy_bucket(json!(1), json!(1), json!(0), json!(10), json!(5), json!(14)),
                json!({}),
                json!({}),
                json!({}),
            ),
            {
                let mut value = legacy_stats(valid.clone(), json!({}), json!({}), json!({}));
                value["startedAt"] = json!(42);
                value
            },
            {
                let mut value = legacy_stats(valid, json!({}), json!({}), json!({}));
                value["lastUpdatedAt"] = json!("not-a-timestamp");
                value
            },
        ];

        for value in malformed {
            assert!(parse_raw(&value.to_string()).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn legacy_stats_reject_u64_and_token_sum_overflow() {
        let u64_overflow = r#"{"schemaVersion":1,"totals":{"callCount":18446744073709551616,"callsWithUsage":0,"callsWithoutUsage":0,"promptTokens":0,"completionTokens":0,"totalTokens":0},"byProvider":{},"byCategory":{},"daily":{}}"#;
        assert!(parse_raw(u64_overflow).is_err());

        let sum_overflow = legacy_stats(
            legacy_bucket(
                json!(1),
                json!(1),
                json!(0),
                json!(u64::MAX),
                json!(1),
                json!(u64::MAX),
            ),
            json!({}),
            json!({}),
            json!({}),
        );
        assert!(parse_raw(&sum_overflow.to_string()).is_err());

        let sqlite_row_overflow = legacy_stats(
            legacy_bucket(
                json!(1),
                json!(1),
                json!(0),
                json!(0),
                json!(0),
                json!(i64::MAX as u64 + 1),
            ),
            json!({}),
            json!({}),
            json!({}),
        );
        assert!(parse_raw(&sqlite_row_overflow.to_string()).is_err());
    }

    #[test]
    fn legacy_stats_enforce_per_bucket_and_cumulative_row_caps() {
        let oversized = legacy_stats(
            legacy_bucket(
                json!(100_001),
                json!(0),
                json!(100_001),
                json!(0),
                json!(0),
                json!(0),
            ),
            json!({}),
            json!({}),
            json!({}),
        );
        assert!(parse_raw(&oversized.to_string()).is_err());

        let first = legacy_bucket(
            json!(50_001),
            json!(0),
            json!(50_001),
            json!(0),
            json!(0),
            json!(0),
        );
        let second = legacy_bucket(
            json!(50_000),
            json!(0),
            json!(50_000),
            json!(0),
            json!(0),
            json!(0),
        );
        let cumulative = legacy_stats(
            legacy_bucket(json!(0), json!(0), json!(0), json!(0), json!(0), json!(0)),
            json!({"first": first, "second": second}),
            json!({}),
            json!({}),
        );
        assert!(parse_raw(&cumulative.to_string()).is_err());
    }

    #[test]
    fn current_rows_enforce_the_same_row_cap_before_row_parsing() {
        let content = format!("[{}]", vec!["null"; 100_001].join(","));
        let error = parse_raw(&content).unwrap_err();

        assert!(error.contains("100000"));
    }

    #[test]
    fn valid_legacy_provider_category_daily_and_totals_sources_convert() {
        for source in ["provider", "category", "daily", "totals"] {
            let bucket = valid_legacy_bucket();
            let value = match source {
                "provider" => legacy_stats(
                    bucket.clone(),
                    json!({"open_ai": bucket}),
                    json!({}),
                    json!({}),
                ),
                "category" => legacy_stats(
                    bucket.clone(),
                    json!({}),
                    json!({"summary": bucket}),
                    json!({}),
                ),
                "daily" => legacy_stats(
                    bucket.clone(),
                    json!({}),
                    json!({}),
                    json!({"2026-05-01": bucket}),
                ),
                "totals" => legacy_stats(bucket, json!({}), json!({}), json!({})),
                _ => unreachable!(),
            };

            let prepared = parse_raw(&value.to_string()).unwrap();
            assert_eq!(prepared.rows.len(), 2, "source {source}");
            assert_eq!(prepared.rows[0].total_tokens, 15, "source {source}");
            assert_eq!(prepared.rows[1].total_tokens, 0, "source {source}");
        }
    }

    #[test]
    fn legacy_stats_reject_each_aggregate_dimension_that_disagrees_with_totals() {
        let totals = valid_legacy_bucket();
        let mismatched = legacy_bucket(json!(1), json!(1), json!(0), json!(3), json!(4), json!(7));
        let cases = [
            legacy_stats(
                totals.clone(),
                json!({"open_ai": mismatched.clone()}),
                json!({}),
                json!({}),
            ),
            legacy_stats(
                totals.clone(),
                json!({}),
                json!({"summary": mismatched.clone()}),
                json!({}),
            ),
            legacy_stats(
                totals,
                json!({}),
                json!({}),
                json!({"2026-05-01": mismatched}),
            ),
        ];

        for value in cases {
            assert!(parse_raw(&value.to_string()).is_err(), "accepted {value}");
        }
    }

    #[test]
    fn valid_full_legacy_stats_with_matching_dimensions_convert() {
        let bucket = valid_legacy_bucket();
        let value = legacy_stats(
            bucket.clone(),
            json!({"open_ai": bucket.clone()}),
            json!({"summary": bucket.clone()}),
            json!({"2026-05-01": bucket}),
        );

        assert_eq!(parse_raw(&value.to_string()).unwrap().rows.len(), 2);
    }

    #[test]
    fn legacy_expansion_shares_repeated_text_fields_between_rows() {
        let bucket = valid_legacy_bucket();
        let provider = "provider".repeat(4_096);
        let value = legacy_stats(
            bucket.clone(),
            json!({provider: bucket}),
            json!({}),
            json!({}),
        );

        let prepared = parse_raw(&value.to_string()).unwrap();
        assert_eq!(prepared.rows.len(), 2);
        assert_eq!(
            prepared.rows[0].occurred_at.as_ptr(),
            prepared.rows[1].occurred_at.as_ptr()
        );
        assert_eq!(
            prepared.rows[0].provider.as_ptr(),
            prepared.rows[1].provider.as_ptr()
        );
        assert_eq!(
            prepared.rows[0].category.as_ptr(),
            prepared.rows[1].category.as_ptr()
        );
    }

    #[test]
    fn test_llm_usage_empty_stats() {
        let db = Database::open_in_memory().unwrap();
        let stats = read_stats(&db).unwrap();
        assert_eq!(stats.totals.call_count, 0);
        assert!(stats.started_at.is_none());
    }
}
