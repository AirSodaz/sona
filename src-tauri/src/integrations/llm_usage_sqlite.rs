use crate::core::database::Database;
use crate::integrations::llm::llm_usage::{
    LlmUsageDashboardStats, LlmUsageStatsFile, UsageBucket, UsageRecord,
};
use chrono::{DateTime, Local};
use serde_json::Value;
use std::collections::BTreeMap;

pub(crate) fn record_usage(db: &Database, record: &UsageRecord) -> Result<(), String> {
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

    db.with_connection(|conn| {
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

pub(crate) fn read_stats(db: &Database) -> Result<LlmUsageStatsFile, String> {
    let rows: Vec<(String, String, String, i64, i64, i64)> = db.with_connection(|conn| {
        let mut stmt = conn.prepare(
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

pub fn read_raw(db: &Database) -> Result<String, String> {
    let rows: Vec<Value> = db.with_connection(|conn| {
        let mut stmt = conn.prepare(
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

    serde_json::to_string(&rows).map_err(|e| e.to_string())
}

pub fn replace_raw(db: &Database, content: &str) -> Result<(), String> {
    let rows: Vec<Value> = serde_json::from_str(content)
        .map_err(|e| format!("LLM usage content must be a JSON array: {e}"))?;

    db.with_transaction(|tx| {
        tx.execute("DELETE FROM analytics.llm_usage", [])?;
        let mut stmt = tx.prepare(
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

pub fn read_dashboard_stats(db: &Database) -> Result<LlmUsageDashboardStats, String> {
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
    fn test_llm_usage_empty_stats() {
        let db = Database::open_in_memory().unwrap();
        let stats = read_stats(&db).unwrap();
        assert_eq!(stats.totals.call_count, 0);
        assert!(stats.started_at.is_none());
    }
}
