use std::cmp::Ordering;
use std::collections::BTreeMap;
use unicode_normalization::UnicodeNormalization;

use super::{
    HistoryItemKind, HistoryItemRecord, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceItemCounts, HistoryWorkspaceItemSearchMatch, HistoryWorkspaceQueryRequest,
    HistoryWorkspaceQueryResult, HistoryWorkspaceScope, HistoryWorkspaceSearchRange,
    HistoryWorkspaceSearchSnippet, HistoryWorkspaceSortOrder, HistoryWorkspaceSummary,
    MAX_WORKSPACE_QUERY_LIMIT,
};
use crate::history::query_repository::HistoryQueryError;

const DEFAULT_SNIPPET_LENGTH: usize = 72;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryWorkspaceDateFilterThresholds {
    pub today_start_millis: u64,
    pub week_start_millis: u64,
    pub month_start_millis: u64,
}

impl HistoryWorkspaceDateFilterThresholds {
    fn threshold_millis(self, date_filter: HistoryWorkspaceDateFilter) -> Option<u64> {
        match date_filter {
            HistoryWorkspaceDateFilter::All => None,
            HistoryWorkspaceDateFilter::Today => Some(self.today_start_millis),
            HistoryWorkspaceDateFilter::Week => Some(self.week_start_millis),
            HistoryWorkspaceDateFilter::Month => Some(self.month_start_millis),
        }
    }
}

pub fn query_workspace_items_at(
    items: Vec<HistoryItemRecord>,
    request: HistoryWorkspaceQueryRequest,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
) -> Result<HistoryWorkspaceQueryResult, HistoryQueryError> {
    validate_workspace_query_request(&request)?;
    let item_counts = count_items_by_tag(&items);
    Ok(query_workspace_items_impl(
        items,
        request,
        item_counts,
        date_filter_thresholds,
    ))
}

pub fn query_workspace_items_with_counts_at(
    items: Vec<HistoryItemRecord>,
    request: HistoryWorkspaceQueryRequest,
    item_counts: HistoryWorkspaceItemCounts,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
) -> Result<HistoryWorkspaceQueryResult, HistoryQueryError> {
    validate_workspace_query_request(&request)?;
    Ok(query_workspace_items_impl(
        items,
        request,
        item_counts,
        date_filter_thresholds,
    ))
}

pub fn validate_workspace_query_request(
    request: &HistoryWorkspaceQueryRequest,
) -> Result<(), HistoryQueryError> {
    if request.limit == 0 || request.limit > MAX_WORKSPACE_QUERY_LIMIT {
        return Err(HistoryQueryError::InvalidRequest(format!(
            "limit must be between 1 and {MAX_WORKSPACE_QUERY_LIMIT}"
        )));
    }
    Ok(())
}

fn query_workspace_items_impl(
    items: Vec<HistoryItemRecord>,
    request: HistoryWorkspaceQueryRequest,
    item_counts: HistoryWorkspaceItemCounts,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
) -> HistoryWorkspaceQueryResult {
    let scoped_items = items
        .into_iter()
        .filter(|item| matches_scope(item, &request.scope))
        .collect::<Vec<_>>();
    let summary = summarize_items(&scoped_items);
    let normalized_query = normalize_workspace_search_text(&request.query);
    let has_query = !normalized_query.text.is_empty();

    let mut filtered_entries = scoped_items
        .into_iter()
        .filter_map(|item| {
            let search_match = if has_query {
                Some(workspace_item_search_match(&item, &normalized_query.text)?)
            } else {
                None
            };

            if !matches_filter_type(&item, request.filter_type) {
                return None;
            }

            if !matches_date_filter(&item, request.date_filter, date_filter_thresholds) {
                return None;
            }

            Some((item, search_match))
        })
        .collect::<Vec<_>>();

    filtered_entries.sort_by(|(a, _), (b, _)| compare_items(a, b, request.sort_order));
    let filtered_item_count = filtered_entries.len();
    let filtered_entries = filtered_entries
        .into_iter()
        .skip(request.offset)
        .take(request.limit)
        .collect::<Vec<_>>();

    let search_match_by_item_id = filtered_entries
        .iter()
        .map(|(item, search_match)| (item.id.clone(), search_match.clone()))
        .collect::<BTreeMap<_, _>>();
    let filtered_items = filtered_entries
        .into_iter()
        .map(|(item, _)| item)
        .collect::<Vec<_>>();
    let has_more = request.offset.saturating_add(filtered_items.len()) < filtered_item_count;

    HistoryWorkspaceQueryResult {
        filtered_items,
        search_match_by_item_id,
        filtered_item_count,
        has_more,
        summary,
        item_counts,
    }
}

fn matches_scope(item: &HistoryItemRecord, scope: &HistoryWorkspaceScope) -> bool {
    match scope {
        HistoryWorkspaceScope::All => item.deleted_at.is_none(),
        HistoryWorkspaceScope::Untagged => item.deleted_at.is_none() && item.tag_ids.is_empty(),
        HistoryWorkspaceScope::Tag { tag_id } => {
            item.deleted_at.is_none() && item.tag_ids.iter().any(|candidate| candidate == tag_id)
        }
        HistoryWorkspaceScope::Trash => item.deleted_at.is_some(),
    }
}

fn matches_filter_type(item: &HistoryItemRecord, filter_type: HistoryWorkspaceFilterType) -> bool {
    match filter_type {
        HistoryWorkspaceFilterType::All => true,
        HistoryWorkspaceFilterType::Recording => item.kind == HistoryItemKind::Recording,
        HistoryWorkspaceFilterType::Batch => item.kind == HistoryItemKind::Batch,
    }
}

fn matches_date_filter(
    item: &HistoryItemRecord,
    date_filter: HistoryWorkspaceDateFilter,
    date_filter_thresholds: HistoryWorkspaceDateFilterThresholds,
) -> bool {
    match date_filter_thresholds.threshold_millis(date_filter) {
        Some(threshold) => item.timestamp >= threshold,
        None => true,
    }
}

fn compare_items(
    a: &HistoryItemRecord,
    b: &HistoryItemRecord,
    sort_order: HistoryWorkspaceSortOrder,
) -> Ordering {
    let ordering = match sort_order {
        HistoryWorkspaceSortOrder::Oldest => a.timestamp.cmp(&b.timestamp),
        HistoryWorkspaceSortOrder::DurationDesc => {
            compare_f64_desc(b.duration, a.duration).then_with(|| b.timestamp.cmp(&a.timestamp))
        }
        HistoryWorkspaceSortOrder::DurationAsc => {
            compare_f64_desc(a.duration, b.duration).then_with(|| b.timestamp.cmp(&a.timestamp))
        }
        HistoryWorkspaceSortOrder::TitleAsc => workspace_title_sort_key(&a.title)
            .cmp(&workspace_title_sort_key(&b.title))
            .then_with(|| b.timestamp.cmp(&a.timestamp)),
        HistoryWorkspaceSortOrder::Newest => b.timestamp.cmp(&a.timestamp),
    };
    ordering.then_with(|| a.id.cmp(&b.id))
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

fn count_items_by_tag(items: &[HistoryItemRecord]) -> HistoryWorkspaceItemCounts {
    let mut untagged = 0;
    let mut trash = 0;
    let mut by_tag_id = BTreeMap::new();

    for item in items {
        if item.deleted_at.is_some() {
            trash += 1;
            continue;
        }
        if item.tag_ids.is_empty() {
            untagged += 1;
        }
        for tag_id in &item.tag_ids {
            *by_tag_id.entry(tag_id.clone()).or_insert(0) += 1;
        }
    }

    HistoryWorkspaceItemCounts {
        untagged,
        trash,
        by_tag_id,
    }
}

#[derive(Clone)]
pub struct NormalizedSearchText {
    pub text: String,
    pub raw_segments: Vec<NormalizedRawSegment>,
}

#[derive(Clone)]
pub struct NormalizedRawSegment {
    pub byte_start: usize,
    pub byte_end: usize,
    pub utf16_start: usize,
    pub utf16_end: usize,
}

#[derive(Clone)]
struct WorkspaceMatchRange {
    byte_start: usize,
    byte_end: usize,
    display_range: HistoryWorkspaceSearchRange,
}

pub fn normalize_workspace_search_text(value: &str) -> NormalizedSearchText {
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

pub fn workspace_search_fields_match(
    title: &str,
    preview_text: &str,
    search_content: &str,
    normalized_query: &str,
) -> bool {
    find_match_range(title, normalized_query).is_some()
        || find_match_range(preview_text, normalized_query).is_some()
        || find_match_range(search_content, normalized_query).is_some()
}

pub fn workspace_title_sort_key(value: &str) -> String {
    value.to_lowercase()
}

pub fn workspace_item_search_match(
    item: &HistoryItemRecord,
    normalized_query: &str,
) -> Option<HistoryWorkspaceItemSearchMatch> {
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

    Some(HistoryWorkspaceItemSearchMatch {
        matched_field: matched_field.to_string(),
        title_match: title_match.map(|range| range.display_range),
        display_snippet: build_snippet(source_text, &display_range, DEFAULT_SNIPPET_LENGTH),
    })
}

#[cfg(test)]
#[path = "workspace_query_tests.rs"]
mod tests;
