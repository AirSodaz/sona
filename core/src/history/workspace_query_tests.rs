use super::*;
use crate::history::{
    HistoryAudioStatus, HistoryDraftSource, HistoryItemStatus, HistoryWorkspaceDateFilter,
    HistoryWorkspaceFilterType, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
};

fn sample_history_item(id: &str, status: HistoryItemStatus) -> HistoryItemRecord {
    HistoryItemRecord {
        id: id.to_string(),
        timestamp: 1,
        duration: 2.0,
        audio_path: format!("{id}.wav"),
        audio_status: HistoryAudioStatus::Available,
        transcript_path: format!("{id}.json"),
        title: format!("Item {id}"),
        preview_text: String::new(),
        icon: None,
        kind: HistoryItemKind::Recording,
        search_content: String::new(),
        tag_ids: vec![id.to_string()],
        deleted_at: None,
        status,
        draft_source: if status == HistoryItemStatus::Draft {
            Some(HistoryDraftSource::LiveRecord)
        } else {
            None
        },
    }
}

fn base_request(query: &str) -> HistoryWorkspaceQueryRequest {
    HistoryWorkspaceQueryRequest {
        scope: HistoryWorkspaceScope::All,
        query: query.to_string(),
        filter_type: HistoryWorkspaceFilterType::All,
        date_filter: HistoryWorkspaceDateFilter::All,
        sort_order: HistoryWorkspaceSortOrder::Newest,
        limit: 100,
        offset: 0,
    }
}

#[test]
fn workspace_query_paginates_after_filtering_with_stable_order_and_totals() {
    let item_c = sample_history_item("c", HistoryItemStatus::Complete);
    let item_a = sample_history_item("a", HistoryItemStatus::Complete);
    let item_b = sample_history_item("b", HistoryItemStatus::Complete);
    let mut request = base_request("item");
    request.limit = 1;
    request.offset = 1;

    let result = query_workspace_items_at(
        vec![item_c, item_a, item_b.clone()],
        request,
        test_thresholds(),
    )
    .unwrap();

    assert_eq!(result.filtered_items, vec![item_b]);
    assert_eq!(result.filtered_item_count, 3);
    assert!(result.has_more);
    assert_eq!(result.summary.total_items, 3);
    assert_eq!(
        result
            .search_match_by_item_id
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        vec!["b".to_string()]
    );
}

#[test]
fn workspace_query_rejects_limits_outside_the_shared_contract() {
    for limit in [0, MAX_WORKSPACE_QUERY_LIMIT + 1] {
        let mut request = base_request("");
        request.limit = limit;

        let result = query_workspace_items_at(Vec::new(), request, test_thresholds());

        assert!(matches!(result, Err(HistoryQueryError::InvalidRequest(_))));
    }
}

#[test]
fn workspace_search_preserves_legacy_frontend_match_semantics() {
    let mut punctuation = sample_history_item("punctuation", HistoryItemStatus::Complete);
    punctuation.title = "Chinese punctuation".to_string();
    punctuation.preview_text = "你好，世界".to_string();
    punctuation.search_content = punctuation.preview_text.clone();

    let punctuation_result = query_workspace_items_at(
        vec![punctuation.clone()],
        base_request("你好,世界"),
        test_thresholds(),
    )
    .unwrap();
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

    let whitespace_result = query_workspace_items_at(
        vec![whitespace.clone()],
        base_request("helloworld"),
        test_thresholds(),
    )
    .unwrap();
    assert!(whitespace_result.filtered_items.is_empty());

    let mut body_priority = sample_history_item("body-priority", HistoryItemStatus::Complete);
    body_priority.title = "Roadmap Review".to_string();
    body_priority.preview_text =
        "Quarterly roadmap discussion with design and product.".to_string();
    body_priority.search_content = body_priority.preview_text.clone();

    let body_result = query_workspace_items_at(
        vec![body_priority.clone()],
        base_request("roadmap"),
        test_thresholds(),
    )
    .unwrap();
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

#[test]
fn workspace_date_filter_uses_supplied_thresholds() {
    let mut before_today = sample_history_item("before-today", HistoryItemStatus::Complete);
    before_today.timestamp = 999;
    let mut today = sample_history_item("today", HistoryItemStatus::Complete);
    today.timestamp = 1_000;

    let mut request = base_request("");
    request.date_filter = HistoryWorkspaceDateFilter::Today;

    let result = query_workspace_items_at(
        vec![before_today, today.clone()],
        request,
        HistoryWorkspaceDateFilterThresholds {
            today_start_millis: 1_000,
            week_start_millis: 500,
            month_start_millis: 100,
        },
    )
    .unwrap();

    assert_eq!(result.filtered_items, vec![today]);
}

fn test_thresholds() -> HistoryWorkspaceDateFilterThresholds {
    HistoryWorkspaceDateFilterThresholds {
        today_start_millis: 1_000,
        week_start_millis: 500,
        month_start_millis: 100,
    }
}
