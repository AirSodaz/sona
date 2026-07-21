use chrono::{Datelike, Duration, Local, LocalResult, TimeZone};
use rusqlite::types::ToSql;
use sona_core::history::workspace_query::HistoryWorkspaceDateFilterThresholds;
use sona_core::history::{
    HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
};
use sona_core::ports::time::ClockError;

pub(super) fn current_workspace_date_filter_thresholds(
    now_millis: u64,
) -> Result<HistoryWorkspaceDateFilterThresholds, ClockError> {
    let now_millis = i64::try_from(now_millis).map_err(|_| {
        ClockError::OutOfRange("History workspace timestamp exceeds chrono range".to_string())
    })?;
    let now = Local
        .timestamp_millis_opt(now_millis)
        .single()
        .ok_or_else(|| {
            ClockError::OutOfRange("History workspace timestamp exceeds chrono range".to_string())
        })?;
    let today = local_day_start(now).unwrap_or(now);
    let month = month_threshold(today).unwrap_or_else(|| today - Duration::days(30));

    Ok(HistoryWorkspaceDateFilterThresholds {
        today_start_millis: millis_u64(today),
        week_start_millis: millis_u64(today - Duration::days(7)),
        month_start_millis: millis_u64(month),
    })
}

pub(super) fn local_day_start(value: chrono::DateTime<Local>) -> Option<chrono::DateTime<Local>> {
    match Local.with_ymd_and_hms(value.year(), value.month(), value.day(), 0, 0, 0) {
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

pub(super) fn month_threshold(today: chrono::DateTime<Local>) -> Option<chrono::DateTime<Local>> {
    let (year, month) = if today.month() == 1 {
        (today.year() - 1, 12)
    } else {
        (today.year(), today.month() - 1)
    };
    match Local.with_ymd_and_hms(
        year,
        month,
        today.day().min(days_in_month(year, month)),
        0,
        0,
        0,
    ) {
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

pub(super) fn millis_u64(value: chrono::DateTime<Local>) -> u64 {
    value.timestamp_millis().max(0) as u64
}

pub(super) fn threshold_for_date_filter(
    date_filter: HistoryWorkspaceDateFilter,
    thresholds: HistoryWorkspaceDateFilterThresholds,
) -> Option<i64> {
    let threshold = match date_filter {
        HistoryWorkspaceDateFilter::All => return None,
        HistoryWorkspaceDateFilter::Today => thresholds.today_start_millis,
        HistoryWorkspaceDateFilter::Week => thresholds.week_start_millis,
        HistoryWorkspaceDateFilter::Month => thresholds.month_start_millis,
    };
    Some(threshold.min(i64::MAX as u64) as i64)
}

pub(super) fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    let Some(first_next_month) = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1) else {
        return 28;
    };
    (first_next_month - Duration::days(1)).day()
}

pub(super) fn add_workspace_query_conditions(
    request: &HistoryWorkspaceQueryRequest,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn ToSql>>,
) {
    match &request.scope {
        HistoryWorkspaceScope::All => clauses.push("h.deleted_at IS NULL".to_string()),
        HistoryWorkspaceScope::Untagged => clauses.push(
            "h.deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM history_item_tags hit WHERE hit.history_id = h.id)".to_string(),
        ),
        HistoryWorkspaceScope::Tag { tag_id } => {
            clauses.push("h.deleted_at IS NULL AND EXISTS (SELECT 1 FROM history_item_tags hit WHERE hit.history_id = h.id AND hit.tag_id = ?)".to_string());
            params.push(Box::new(tag_id.clone()));
        }
        HistoryWorkspaceScope::Trash => clauses.push("h.deleted_at IS NOT NULL".to_string()),
    }
    match request.filter_type {
        HistoryWorkspaceFilterType::All => {}
        HistoryWorkspaceFilterType::Recording => {
            clauses.push("h.kind = 'recording'".to_string());
        }
        HistoryWorkspaceFilterType::Batch => {
            clauses.push("h.kind = 'batch'".to_string());
        }
    }
    if let Some(threshold) = threshold_for_date_filter(request.date_filter, date_filter_thresholds)
    {
        clauses.push("h.timestamp >= ?".to_string());
        params.push(Box::new(threshold));
    }
}

pub(super) fn add_workspace_scope_condition(
    scope: &HistoryWorkspaceScope,
    clauses: &mut Vec<String>,
    params: &mut Vec<Box<dyn ToSql>>,
) {
    match scope {
        HistoryWorkspaceScope::All => clauses.push("h.deleted_at IS NULL".to_string()),
        HistoryWorkspaceScope::Untagged => clauses.push(
            "h.deleted_at IS NULL AND NOT EXISTS (SELECT 1 FROM history_item_tags hit WHERE hit.history_id = h.id)".to_string(),
        ),
        HistoryWorkspaceScope::Tag { tag_id } => {
            clauses.push("h.deleted_at IS NULL AND EXISTS (SELECT 1 FROM history_item_tags hit WHERE hit.history_id = h.id AND hit.tag_id = ?)".to_string());
            params.push(Box::new(tag_id.clone()));
        }
        HistoryWorkspaceScope::Trash => clauses.push("h.deleted_at IS NOT NULL".to_string()),
    }
}

pub(super) fn workspace_order_by(sort_order: HistoryWorkspaceSortOrder) -> &'static str {
    match sort_order {
        HistoryWorkspaceSortOrder::Newest => "h.timestamp DESC, h.id ASC",
        HistoryWorkspaceSortOrder::Oldest => "h.timestamp ASC, h.id ASC",
        HistoryWorkspaceSortOrder::DurationDesc => "h.duration DESC, h.timestamp DESC, h.id ASC",
        HistoryWorkspaceSortOrder::DurationAsc => "h.duration ASC, h.timestamp DESC, h.id ASC",
        HistoryWorkspaceSortOrder::TitleAsc => {
            "sona_workspace_title_key(h.title) ASC, h.timestamp DESC, h.id ASC"
        }
    }
}

pub(super) fn workspace_match_query_parts(
    request: &HistoryWorkspaceQueryRequest,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
    normalized_query: &str,
) -> (String, Vec<Box<dyn ToSql>>) {
    let mut conditions = Vec::new();
    let mut params: Vec<Box<dyn ToSql>> = Vec::new();

    add_workspace_query_conditions(
        request,
        date_filter_thresholds,
        &mut conditions,
        &mut params,
    );
    if !normalized_query.is_empty() {
        conditions.push(
            "sona_workspace_matches(h.title, h.preview_text, h.search_content, ?) = 1".to_string(),
        );
        params.push(Box::new(normalized_query.to_string()));
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };
    (where_clause, params)
}
