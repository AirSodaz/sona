use chrono::{Datelike, Duration, Local, LocalResult, TimeZone};
use std::cmp::Ordering;
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

use super::{
    HistoryItemKind, HistoryItemRecord, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSearchRange,
    HistoryWorkspaceSearchSnippet, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
};

const DEFAULT_SNIPPET_LENGTH: usize = 72;

#[cfg(test)]
pub(super) fn query_workspace_items(
    items: Vec<HistoryItemRecord>,
    request: HistoryWorkspaceQueryRequest,
) -> HistoryWorkspaceQueryResult {
    let item_counts = count_items_by_project(&items);
    query_workspace_items_impl(items, request, item_counts)
}

pub(super) fn query_workspace_items_with_counts(
    items: Vec<HistoryItemRecord>,
    request: HistoryWorkspaceQueryRequest,
    item_counts: HistoryWorkspaceItemCounts,
) -> HistoryWorkspaceQueryResult {
    query_workspace_items_impl(items, request, item_counts)
}

fn query_workspace_items_impl(
    items: Vec<HistoryItemRecord>,
    request: HistoryWorkspaceQueryRequest,
    item_counts: HistoryWorkspaceItemCounts,
) -> HistoryWorkspaceQueryResult {
    let scoped_items = items
        .into_iter()
        .filter(|item| matches_scope(item, &request.scope))
        .collect::<Vec<_>>();
    let summary = summarize_items(&scoped_items);
    let scoped_item_ids = scoped_items
        .iter()
        .map(|item| item.id.clone())
        .collect::<Vec<_>>();
    let scoped_items_for_result = scoped_items.clone();
    let normalized_query = normalize_workspace_search_text(&request.query);
    let has_query = !normalized_query.text.is_empty();

    let mut filtered_entries = scoped_items
        .into_iter()
        .filter_map(|item| {
            let search_match = if has_query {
                match_workspace_item(&item, &normalized_query.text)?
            } else {
                None
            };

            if !matches_filter_type(&item, request.filter_type) {
                return None;
            }

            if !matches_date_filter(&item, request.date_filter) {
                return None;
            }

            Some((item, search_match))
        })
        .collect::<Vec<_>>();

    filtered_entries.sort_by(|(a, _), (b, _)| compare_items(a, b, request.sort_order));

    let search_match_by_item_id = filtered_entries
        .iter()
        .map(|(item, search_match)| (item.id.clone(), search_match.clone()))
        .collect::<BTreeMap<_, _>>();
    let filtered_items = filtered_entries
        .into_iter()
        .map(|(item, _)| item)
        .collect::<Vec<_>>();

    HistoryWorkspaceQueryResult {
        filtered_items,
        scoped_items: scoped_items_for_result,
        scoped_item_ids,
        search_match_by_item_id,
        summary,
        item_counts,
    }
}

fn matches_scope(item: &HistoryItemRecord, scope: &HistoryWorkspaceScope) -> bool {
    match scope {
        HistoryWorkspaceScope::All => true,
        HistoryWorkspaceScope::Inbox => item.project_id.is_none(),
        HistoryWorkspaceScope::Project { project_id } => {
            item.project_id.as_deref() == Some(project_id)
        }
    }
}

fn matches_filter_type(item: &HistoryItemRecord, filter_type: HistoryWorkspaceFilterType) -> bool {
    match filter_type {
        HistoryWorkspaceFilterType::All => true,
        HistoryWorkspaceFilterType::Recording => item.kind == HistoryItemKind::Recording,
        HistoryWorkspaceFilterType::Batch => item.kind == HistoryItemKind::Batch,
    }
}

fn matches_date_filter(item: &HistoryItemRecord, date_filter: HistoryWorkspaceDateFilter) -> bool {
    if date_filter == HistoryWorkspaceDateFilter::All {
        return true;
    }

    let Some(item_date) = timestamp_millis_to_local(item.timestamp) else {
        return false;
    };
    let now = Local::now();
    let today = match Local.with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0) {
        LocalResult::Single(value) => value,
        LocalResult::Ambiguous(earliest, _) => earliest,
        LocalResult::None => return true,
    };

    let threshold = match date_filter {
        HistoryWorkspaceDateFilter::Today => today,
        HistoryWorkspaceDateFilter::Week => today - Duration::days(7),
        HistoryWorkspaceDateFilter::Month => {
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
                LocalResult::Single(value) => value,
                LocalResult::Ambiguous(earliest, _) => earliest,
                LocalResult::None => today - Duration::days(30),
            }
        }
        HistoryWorkspaceDateFilter::All => today,
    };

    item_date >= threshold
}

fn timestamp_millis_to_local(timestamp: u64) -> Option<chrono::DateTime<Local>> {
    match Local.timestamp_millis_opt(timestamp as i64) {
        LocalResult::Single(value) => Some(value),
        LocalResult::Ambiguous(earliest, _) => Some(earliest),
        LocalResult::None => None,
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let next_month = if month == 12 { 1 } else { month + 1 };
    let next_year = if month == 12 { year + 1 } else { year };
    let Some(first_next_month) = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1) else {
        return 28;
    };
    (first_next_month - Duration::days(1)).day()
}

fn compare_items(
    a: &HistoryItemRecord,
    b: &HistoryItemRecord,
    sort_order: HistoryWorkspaceSortOrder,
) -> Ordering {
    match sort_order {
        HistoryWorkspaceSortOrder::Oldest => a.timestamp.cmp(&b.timestamp),
        HistoryWorkspaceSortOrder::DurationDesc => {
            compare_f64_desc(b.duration, a.duration).then_with(|| b.timestamp.cmp(&a.timestamp))
        }
        HistoryWorkspaceSortOrder::DurationAsc => {
            compare_f64_desc(a.duration, b.duration).then_with(|| b.timestamp.cmp(&a.timestamp))
        }
        HistoryWorkspaceSortOrder::TitleAsc => a
            .title
            .to_lowercase()
            .cmp(&b.title.to_lowercase())
            .then_with(|| b.timestamp.cmp(&a.timestamp)),
        HistoryWorkspaceSortOrder::Newest => b.timestamp.cmp(&a.timestamp),
    }
}

fn compare_f64_desc(left: f64, right: f64) -> Ordering {
    left.partial_cmp(&right).unwrap_or(Ordering::Equal)
}

fn summarize_items(items: &[HistoryItemRecord]) -> HistoryWorkspaceSummary {
    let mut total_duration = 0.0;
    let mut latest_timestamp = None;
    let mut recording_count = 0;
    let mut batch_count = 0;

    for item in items {
        total_duration += item.duration;
        latest_timestamp = Some(
            latest_timestamp.map_or(item.timestamp, |current: u64| current.max(item.timestamp)),
        );
        match item.kind {
            HistoryItemKind::Batch => batch_count += 1,
            HistoryItemKind::Recording => recording_count += 1,
        }
    }

    HistoryWorkspaceSummary {
        total_items: items.len(),
        total_duration,
        latest_timestamp,
        recording_count,
        batch_count,
    }
}

#[cfg(test)]
fn count_items_by_project(items: &[HistoryItemRecord]) -> HistoryWorkspaceItemCounts {
    let mut inbox = 0;
    let mut by_project_id = BTreeMap::new();

    for item in items {
        match &item.project_id {
            Some(project_id) => {
                *by_project_id.entry(project_id.clone()).or_insert(0) += 1;
            }
            None => inbox += 1,
        }
    }

    HistoryWorkspaceItemCounts {
        inbox,
        by_project_id,
    }
}

#[derive(Clone)]
pub(super) struct NormalizedSearchText {
    pub(super) text: String,
    pub(super) raw_segments: Vec<NormalizedRawSegment>,
}

#[derive(Clone)]
pub(super) struct NormalizedRawSegment {
    pub(super) byte_start: usize,
    pub(super) byte_end: usize,
    pub(super) utf16_start: usize,
    pub(super) utf16_end: usize,
}

#[derive(Clone)]
struct WorkspaceMatchRange {
    byte_start: usize,
    byte_end: usize,
    display_range: HistoryWorkspaceSearchRange,
}

pub(super) fn normalize_workspace_search_text(value: &str) -> NormalizedSearchText {
    let mut normalized = String::new();
    let mut raw_segments: Vec<NormalizedRawSegment> = Vec::new();
    let mut raw_utf16_offset = 0;

    for (raw_start, char) in value.char_indices() {
        let raw_end = raw_start + char.len_utf8();
        let raw_utf16_start = raw_utf16_offset;
        raw_utf16_offset += char.len_utf16();
        let raw_utf16_end = raw_utf16_offset;
        let nfkc = char.to_string().nfkc().collect::<String>();
        for normalized_char in nfkc
            .chars()
            .map(map_compatibility_char)
            .flat_map(|mapped| mapped.to_lowercase())
        {
            if normalized_char.is_whitespace() {
                if normalized.is_empty() {
                    continue;
                }
                if normalized.ends_with(' ') {
                    if let Some(last) = raw_segments.last_mut() {
                        last.byte_end = raw_end;
                        last.utf16_end = raw_utf16_end;
                    }
                    continue;
                }
                normalized.push(' ');
                raw_segments.push(NormalizedRawSegment {
                    byte_start: raw_start,
                    byte_end: raw_end,
                    utf16_start: raw_utf16_start,
                    utf16_end: raw_utf16_end,
                });
                continue;
            }

            normalized.push(normalized_char);
            raw_segments.push(NormalizedRawSegment {
                byte_start: raw_start,
                byte_end: raw_end,
                utf16_start: raw_utf16_start,
                utf16_end: raw_utf16_end,
            });
        }
    }

    if normalized.ends_with(' ') {
        normalized.pop();
        raw_segments.pop();
    }

    NormalizedSearchText {
        text: normalized,
        raw_segments,
    }
}

fn map_compatibility_char(value: char) -> char {
    map_punctuation(value)
        .or_else(|| {
            let codepoint = value as u32;
            if (0xff01..=0xff5e).contains(&codepoint) {
                char::from_u32(codepoint - 0xfee0)
            } else {
                None
            }
        })
        .unwrap_or(value)
}

fn map_punctuation(value: char) -> Option<char> {
    Some(match value {
        '，' | '、' => ',',
        '。' | '．' => '.',
        '：' => ':',
        '；' => ';',
        '！' => '!',
        '？' => '?',
        '（' => '(',
        '）' => ')',
        '【' | '［' => '[',
        '】' | '］' => ']',
        '｛' => '{',
        '｝' => '}',
        '《' | '〈' => '<',
        '》' | '〉' => '>',
        '“' | '”' | '「' | '」' | '『' | '』' => '"',
        '‘' | '’' => '\'',
        '—' | '–' | '－' => '-',
        '〜' | '～' => '~',
        '／' => '/',
        '＼' => '\\',
        '｜' => '|',
        '·' | '・' => '.',
        _ => return None,
    })
}

fn find_match_range(value: &str, normalized_query: &str) -> Option<WorkspaceMatchRange> {
    if normalized_query.is_empty() {
        return None;
    }

    let normalized = normalize_workspace_search_text(value);
    let match_start_byte = normalized.text.find(normalized_query)?;
    let match_start = normalized.text[..match_start_byte].chars().count();
    let match_end = match_start + normalized_query.chars().count() - 1;
    let start_segment = normalized.raw_segments.get(match_start)?;
    let end_segment = normalized.raw_segments.get(match_end)?;
    Some(WorkspaceMatchRange {
        byte_start: start_segment.byte_start,
        byte_end: end_segment.byte_end,
        display_range: HistoryWorkspaceSearchRange {
            start: start_segment.utf16_start,
            end: end_segment.utf16_end,
        },
    })
}

fn build_snippet(
    value: &str,
    range: &WorkspaceMatchRange,
    max_length: usize,
) -> HistoryWorkspaceSearchSnippet {
    let safe_start = range.byte_start.min(value.len());
    let safe_end = range.byte_end.min(value.len()).max(safe_start + 1);
    let match_length = safe_end - safe_start;
    let snippet_length = max_length.max(match_length);
    let context_length = (snippet_length - match_length) / 2;

    let mut slice_start = safe_start.saturating_sub(context_length);
    let mut slice_end = (safe_end + context_length).min(value.len());

    if slice_end - slice_start < snippet_length {
        if slice_start == 0 {
            slice_end = value.len().min(snippet_length);
        } else if slice_end == value.len() {
            slice_start = value.len().saturating_sub(snippet_length);
        }
    }

    while slice_start < slice_end && !value.is_char_boundary(slice_start) {
        slice_start += 1;
    }
    while slice_end > slice_start && !value.is_char_boundary(slice_end) {
        slice_end -= 1;
    }

    let snippet_slice = &value[slice_start..slice_end];
    let leading_whitespace = snippet_slice
        .char_indices()
        .take_while(|(_, char)| char.is_whitespace())
        .last()
        .map_or(0, |(index, char)| index + char.len_utf8());
    let trailing_whitespace = snippet_slice
        .char_indices()
        .rev()
        .take_while(|(_, char)| char.is_whitespace())
        .last()
        .map_or(0, |(index, _)| snippet_slice.len() - index);

    slice_start += leading_whitespace;
    slice_end = slice_end.saturating_sub(trailing_whitespace);
    let snippet_text = &value[slice_start..slice_end];

    let mut highlight_start = snippet_text[..safe_start.saturating_sub(slice_start)]
        .encode_utf16()
        .count();
    let mut highlight_end = snippet_text[..safe_end.saturating_sub(slice_start)]
        .encode_utf16()
        .count();
    let prefix = if slice_start > 0 {
        highlight_start += 3;
        highlight_end += 3;
        "..."
    } else {
        ""
    };
    let suffix = if slice_end < value.len() { "..." } else { "" };

    HistoryWorkspaceSearchSnippet {
        text: format!("{prefix}{snippet_text}{suffix}"),
        highlight_start,
        highlight_end,
    }
}

fn match_workspace_item(
    item: &HistoryItemRecord,
    normalized_query: &str,
) -> Option<Option<HistoryWorkspaceItemSearchMatch>> {
    let title_match = find_match_range(&item.title, normalized_query);
    let preview_match = find_match_range(&item.preview_text, normalized_query);
    let search_content_match = find_match_range(&item.search_content, normalized_query);

    if title_match.is_none() && preview_match.is_none() && search_content_match.is_none() {
        return None;
    }

    let matched_field = if title_match.is_some() {
        "title"
    } else if preview_match.is_some() {
        "previewText"
    } else {
        "searchContent"
    };

    let (source_text, display_range) = if let Some(range) = preview_match.clone() {
        (item.preview_text.as_str(), range)
    } else if let Some(range) = search_content_match.clone() {
        (item.search_content.as_str(), range)
    } else {
        (item.title.as_str(), title_match.clone()?)
    };

    Some(Some(HistoryWorkspaceItemSearchMatch {
        matched_field: matched_field.to_string(),
        title_match: title_match.map(|range| range.display_range),
        display_snippet: build_snippet(source_text, &display_range, DEFAULT_SNIPPET_LENGTH),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::history::test_support::sample_history_item;
    use crate::repositories::history::{
        HistoryItemStatus, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
        HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
    };

    fn base_request(query: &str) -> HistoryWorkspaceQueryRequest {
        HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: query.to_string(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
        }
    }

    #[test]
    fn workspace_search_preserves_legacy_frontend_match_semantics() {
        let mut punctuation = sample_history_item("punctuation", HistoryItemStatus::Complete);
        punctuation.title = "Chinese punctuation".to_string();
        punctuation.preview_text = "你好，世界".to_string();
        punctuation.search_content = punctuation.preview_text.clone();

        let punctuation_result =
            query_workspace_items(vec![punctuation.clone()], base_request("你好,世界"));
        assert_eq!(punctuation_result.filtered_items, vec![punctuation.clone()]);
        let punctuation_match = punctuation_result
            .search_match_by_item_id
            .get("punctuation")
            .and_then(|entry| entry.as_ref())
            .unwrap();
        assert_eq!(punctuation_match.matched_field, "previewText");
        assert_eq!(punctuation_match.display_snippet.text, "你好，世界");

        let mut whitespace = sample_history_item("whitespace", HistoryItemStatus::Complete);
        whitespace.title = "Whitespace".to_string();
        whitespace.preview_text = "hello world".to_string();
        whitespace.search_content = whitespace.preview_text.clone();

        let whitespace_result =
            query_workspace_items(vec![whitespace.clone()], base_request("helloworld"));
        assert!(whitespace_result.filtered_items.is_empty());

        let mut body_priority = sample_history_item("body-priority", HistoryItemStatus::Complete);
        body_priority.title = "Roadmap Review".to_string();
        body_priority.preview_text =
            "Quarterly roadmap discussion with design and product.".to_string();
        body_priority.search_content = body_priority.preview_text.clone();

        let body_result =
            query_workspace_items(vec![body_priority.clone()], base_request("roadmap"));
        let body_match = body_result
            .search_match_by_item_id
            .get("body-priority")
            .and_then(|entry| entry.as_ref())
            .unwrap();
        assert_eq!(body_match.matched_field, "title");
        assert_eq!(
            body_match.title_match,
            Some(HistoryWorkspaceSearchRange { start: 0, end: 7 })
        );
        assert!(
            body_match
                .display_snippet
                .text
                .contains("Quarterly roadmap discussion")
        );
    }
}
