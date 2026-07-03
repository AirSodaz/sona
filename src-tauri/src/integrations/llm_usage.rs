use crate::integrations::llm::{LlmUsageCategory, TokenUsage};
use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub(crate) const RECENT_DAILY_WINDOW: i64 = 30;

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
pub struct DashboardUsageBucket {
    pub(crate) call_count: u64,
    pub(crate) call_count_display: String,
    pub(crate) calls_with_usage: u64,
    pub(crate) calls_with_usage_display: String,
    pub(crate) calls_without_usage: u64,
    pub(crate) calls_without_usage_display: String,
    pub(crate) prompt_tokens: u64,
    pub(crate) prompt_tokens_display: String,
    pub(crate) completion_tokens: u64,
    pub(crate) completion_tokens_display: String,
    pub(crate) total_tokens: u64,
    pub(crate) total_tokens_display: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageBreakdown {
    pub(crate) key: String,
    pub(crate) label: String,
    pub(crate) stats: DashboardUsageBucket,
    pub(crate) value: u64,
    pub(crate) value_display: String,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UsageTrendPoint {
    pub(crate) date: String,
    pub(crate) date_label: String,
    #[serde(flatten)]
    pub(crate) stats: DashboardUsageBucket,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LlmUsageDashboardStats {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) tracking_since_display: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_updated_display: Option<String>,
    pub(crate) totals: DashboardUsageBucket,
    pub(crate) by_provider: Vec<UsageBreakdown>,
    pub(crate) by_provider_top_rows: Vec<UsageBreakdown>,
    pub(crate) by_provider_max_value: u64,
    pub(crate) by_category: Vec<UsageBreakdown>,
    pub(crate) by_category_top_rows: Vec<UsageBreakdown>,
    pub(crate) by_category_max_value: u64,
    pub(crate) recent_daily: Vec<UsageTrendPoint>,
}

#[derive(Clone, Debug)]
pub(crate) struct UsageRecord {
    pub(crate) occurred_at: String,
    pub(crate) provider: String,
    pub(crate) category: LlmUsageCategory,
    pub(crate) usage: Option<TokenUsage>,
}

pub(crate) fn to_dashboard_stats(stats: &LlmUsageStatsFile) -> LlmUsageDashboardStats {
    let by_provider = to_sorted_breakdown(&stats.by_provider);
    let by_category = to_sorted_breakdown(&stats.by_category);

    LlmUsageDashboardStats {
        started_at: stats.started_at.clone(),
        last_updated_at: stats.last_updated_at.clone(),
        tracking_since_display: stats.started_at.as_deref().map(format_date_time_label),
        last_updated_display: stats.last_updated_at.as_deref().map(format_date_time_label),
        totals: to_dashboard_bucket(&stats.totals),
        by_provider_max_value: max_breakdown_value(&by_provider),
        by_provider_top_rows: by_provider.iter().take(6).cloned().collect(),
        by_provider,
        by_category_max_value: max_breakdown_value(&by_category),
        by_category_top_rows: by_category.iter().take(6).cloned().collect(),
        by_category,
        recent_daily: build_recent_daily_trend(&stats.daily),
    }
}

fn to_sorted_breakdown(collection: &BTreeMap<String, UsageBucket>) -> Vec<UsageBreakdown> {
    let mut breakdowns = collection
        .iter()
        .map(|(key, stats)| UsageBreakdown {
            key: key.clone(),
            label: key.clone(),
            stats: to_dashboard_bucket(stats),
            value: stats.total_tokens.max(stats.call_count),
            value_display: format_number(stats.total_tokens.max(stats.call_count)),
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
                stats: to_dashboard_bucket(&daily.get(&key).cloned().unwrap_or_default()),
                date_label: format_date_key(&key),
                date: key,
            }
        })
        .collect()
}

fn to_dashboard_bucket(bucket: &UsageBucket) -> DashboardUsageBucket {
    DashboardUsageBucket {
        call_count: bucket.call_count,
        call_count_display: format_number(bucket.call_count),
        calls_with_usage: bucket.calls_with_usage,
        calls_with_usage_display: format_number(bucket.calls_with_usage),
        calls_without_usage: bucket.calls_without_usage,
        calls_without_usage_display: format_number(bucket.calls_without_usage),
        prompt_tokens: bucket.prompt_tokens,
        prompt_tokens_display: format_number(bucket.prompt_tokens),
        completion_tokens: bucket.completion_tokens,
        completion_tokens_display: format_number(bucket.completion_tokens),
        total_tokens: bucket.total_tokens,
        total_tokens_display: format_number(bucket.total_tokens),
    }
}

fn max_breakdown_value(breakdown: &[UsageBreakdown]) -> u64 {
    breakdown.iter().map(|item| item.value).max().unwrap_or(0)
}

fn format_number(value: u64) -> String {
    let raw = value.to_string();
    let mut formatted = String::new();
    for (index, character) in raw.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            formatted.push(',');
        }
        formatted.push(character);
    }
    formatted.chars().rev().collect()
}

fn format_date_key(date_key: &str) -> String {
    chrono::NaiveDate::parse_from_str(date_key, "%Y-%m-%d")
        .map(|date| date.format("%m-%d").to_string())
        .unwrap_or_default()
}

fn format_date_time_label(value: &str) -> String {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.date_naive().format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn dashboard_stats_sort_breakdowns_and_include_recent_window() {
        let mut by_provider = BTreeMap::new();
        by_provider.insert(
            "ollama".to_string(),
            UsageBucket {
                call_count: 1,
                total_tokens: 5,
                ..UsageBucket::default()
            },
        );
        by_provider.insert(
            "open_ai".to_string(),
            UsageBucket {
                call_count: 1,
                total_tokens: 15,
                ..UsageBucket::default()
            },
        );

        let mut by_category = BTreeMap::new();
        by_category.insert(
            "summary".to_string(),
            UsageBucket {
                call_count: 2,
                total_tokens: 20,
                ..UsageBucket::default()
            },
        );

        let stats = to_dashboard_stats(&LlmUsageStatsFile {
            schema_version: 1,
            started_at: Some("2026-05-01T00:00:00Z".to_string()),
            last_updated_at: Some("2026-05-03T00:00:00Z".to_string()),
            totals: UsageBucket {
                call_count: 2,
                total_tokens: 20,
                ..UsageBucket::default()
            },
            by_provider,
            by_category,
            daily: BTreeMap::new(),
        });

        assert_eq!(stats.started_at.as_deref(), Some("2026-05-01T00:00:00Z"));
        assert_eq!(stats.by_provider[0].key, "open_ai");
        assert_eq!(stats.by_provider[1].key, "ollama");
        assert_eq!(stats.by_category[0].key, "summary");
        assert_eq!(stats.recent_daily.len(), RECENT_DAILY_WINDOW as usize);
    }
}
