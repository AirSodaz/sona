use crate::core::database::{Database, DatabaseError};
use crate::integrations::llm::llm_usage::{
    LlmUsageDashboardStats, LlmUsageStatsFile, UsageBucket, UsageRecord,
};
use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn record_usage(db: &Database, record: &UsageRecord) -> Result<(), DatabaseError> {
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

pub(crate) fn read_stats(db: &Database) -> Result<LlmUsageStatsFile, DatabaseError> {
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
    let rows: Vec<Value> = db.with_connection(|conn| {
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
            Ok(serde_json::json!({
                "occurredAt": occurred_at,
                "provider": provider,
                "category": category,
                "promptTokens": prompt_tokens,
                "completionTokens": completion_tokens,
                "totalTokens": total_tokens,
            }))
        })?;
        let mut items = Vec::new();
        for row in rows {
            items.push(row?);
        }
        Ok(items)
    })?;

    Ok(serde_json::to_string(&rows)?)
}

pub fn replace_raw(db: &Database, content: &str) -> Result<(), DatabaseError> {
    let value: Value = serde_json::from_str(content)
        .map_err(|e| DatabaseError::Internal(format!("Invalid LLM usage content: {e}")))?;
    let rows = match value {
        Value::Array(rows) => rows,
        Value::Object(object) => legacy_stats_object_to_rows(&object),
        _ => {
            return Err(DatabaseError::Internal(
                "LLM usage content must be a JSON array or object.".to_string(),
            ));
        }
    };

    db.with_transaction(|tx| {
        tx.execute("DELETE FROM analytics.llm_usage", [])?;
        let mut stmt = tx.prepare_cached(
            "INSERT INTO analytics.llm_usage (occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        )?;
        for row in &rows {
            let occurred_at = row.get("occurredAt").and_then(Value::as_str).unwrap_or("");
            let provider = row.get("provider").and_then(Value::as_str).unwrap_or("");
            let category = row.get("category").and_then(Value::as_str).unwrap_or("");
            let prompt_tokens = row.get("promptTokens").and_then(Value::as_i64).unwrap_or(0);
            let completion_tokens = row.get("completionTokens").and_then(Value::as_i64).unwrap_or(0);
            let total_tokens = row.get("totalTokens").and_then(Value::as_i64).unwrap_or(0);
            stmt.execute(rusqlite::params![
                occurred_at, provider, category, prompt_tokens, completion_tokens, total_tokens,
            ])?;
        }
        Ok(())
    })
}

fn legacy_stats_object_to_rows(object: &serde_json::Map<String, Value>) -> Vec<Value> {
    let fallback_occurred_at = object
        .get("lastUpdatedAt")
        .or_else(|| object.get("startedAt"))
        .and_then(Value::as_str)
        .unwrap_or("1970-01-01T00:00:00Z");

    if let Some(rows) = rows_from_bucket_map(
        object.get("byProvider"),
        fallback_occurred_at,
        |key| key.to_string(),
        |_| "migrated".to_string(),
    ) {
        return rows;
    }

    if let Some(rows) = rows_from_bucket_map(
        object.get("byCategory"),
        fallback_occurred_at,
        |_| "migrated".to_string(),
        |key| key.to_string(),
    ) {
        return rows;
    }

    if let Some(rows) = rows_from_bucket_map(
        object.get("daily"),
        fallback_occurred_at,
        |_| "migrated".to_string(),
        |_| "migrated".to_string(),
    ) {
        return rows;
    }

    object
        .get("totals")
        .and_then(Value::as_object)
        .map(|bucket| rows_from_bucket(fallback_occurred_at, "total", "migrated", bucket))
        .unwrap_or_default()
}

fn rows_from_bucket_map(
    value: Option<&Value>,
    fallback_occurred_at: &str,
    provider_for_key: impl Fn(&str) -> String,
    category_for_key: impl Fn(&str) -> String,
) -> Option<Vec<Value>> {
    let buckets = value.and_then(Value::as_object)?;
    let mut rows = Vec::new();
    for (key, bucket) in buckets {
        let Some(bucket) = bucket.as_object() else {
            continue;
        };
        let occurred_at = if is_date_key(key) {
            format!("{key}T00:00:00Z")
        } else {
            fallback_occurred_at.to_string()
        };
        rows.extend(rows_from_bucket(
            &occurred_at,
            &provider_for_key(key),
            &category_for_key(key),
            bucket,
        ));
    }
    if rows.is_empty() { None } else { Some(rows) }
}

fn rows_from_bucket(
    occurred_at: &str,
    provider: &str,
    category: &str,
    bucket: &serde_json::Map<String, Value>,
) -> Vec<Value> {
    let prompt_tokens = u64_field(bucket, "promptTokens");
    let completion_tokens = u64_field(bucket, "completionTokens");
    let total_tokens = u64_field(bucket, "totalTokens").max(prompt_tokens + completion_tokens);
    let calls_with_usage = u64_field(bucket, "callsWithUsage");
    let calls_without_usage = u64_field(bucket, "callsWithoutUsage");
    let explicit_call_count = u64_field(bucket, "callCount");
    let inferred_call_count = calls_with_usage
        .saturating_add(calls_without_usage)
        .max(if total_tokens > 0 { 1 } else { 0 });
    let call_count = explicit_call_count.max(inferred_call_count);

    if call_count == 0 {
        return Vec::new();
    }

    let usage_row_count = if calls_with_usage > 0 {
        calls_with_usage.min(call_count)
    } else if total_tokens > 0 {
        call_count
    } else {
        0
    };
    let empty_row_count = call_count.saturating_sub(usage_row_count);
    let mut rows = Vec::new();

    for index in 0..usage_row_count {
        rows.push(serde_json::json!({
            "occurredAt": occurred_at,
            "provider": provider,
            "category": category,
            "promptTokens": split_u64(prompt_tokens, usage_row_count, index),
            "completionTokens": split_u64(completion_tokens, usage_row_count, index),
            "totalTokens": split_u64(total_tokens, usage_row_count, index),
        }));
    }

    for _ in 0..empty_row_count {
        rows.push(serde_json::json!({
            "occurredAt": occurred_at,
            "provider": provider,
            "category": category,
            "promptTokens": 0,
            "completionTokens": 0,
            "totalTokens": 0,
        }));
    }

    rows
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

fn u64_field(source: &serde_json::Map<String, Value>, key: &str) -> u64 {
    source.get(key).and_then(Value::as_u64).unwrap_or(0)
}

fn is_date_key(value: &str) -> bool {
    value.len() == 10
        && value.as_bytes()[4] == b'-'
        && value.as_bytes()[7] == b'-'
        && value
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub fn read_dashboard_stats(db: &Database) -> Result<LlmUsageDashboardStats, DatabaseError> {
    let stats = read_stats(db)?;
    Ok(crate::integrations::llm::llm_usage::to_dashboard_stats(
        &stats,
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
    use crate::core::database::Database;
    use crate::integrations::llm::llm_usage::UsageRecord;
    use crate::integrations::llm::{LlmUsageCategory, TokenUsage};
    use serde_json::json;

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

        replace_raw(
            &db,
            r#"{"schemaVersion":1,"totals":{"callCount":2,"totalTokens":42},"byProvider":{"open_ai":{"callCount":2,"totalTokens":42}}}"#,
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
    fn test_llm_usage_empty_stats() {
        let db = Database::open_in_memory().unwrap();
        let stats = read_stats(&db).unwrap();
        assert_eq!(stats.totals.call_count, 0);
        assert!(stats.started_at.is_none());
    }
}
