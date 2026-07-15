use std::sync::Arc;

use sona_core::history::query_repository::{HistoryQueryError, HistoryQueryRepository};
use sona_core::history::query_service::HistoryQueryService;
use sona_core::history::{
    HistoryListOptions, HistoryWorkspaceDateFilter, HistoryWorkspaceFilterType,
    HistoryWorkspaceQueryRequest, HistoryWorkspaceScope, HistoryWorkspaceSortOrder,
};
use sona_sqlite::LazySqliteHistoryQueryRepository;

fn service(app_data_dir: &std::path::Path) -> HistoryQueryService {
    HistoryQueryService::new(Arc::new(LazySqliteHistoryQueryRepository::new(
        app_data_dir.to_path_buf(),
    )))
}

#[test]
fn lazy_repository_implements_the_core_history_query_port() {
    fn assert_query_port<T: HistoryQueryRepository>() {}

    assert_query_port::<LazySqliteHistoryQueryRepository>();
}

#[test]
fn core_id_validation_precedes_lazy_database_open() {
    let dir = tempfile::tempdir().unwrap();
    let error = service(dir.path()).load_transcript("").unwrap_err();

    assert!(matches!(error, HistoryQueryError::InvalidRequest(_)));
    assert_eq!(
        error.to_string(),
        "Invalid history query: history ID must not be empty"
    );
    assert!(!dir.path().join("sona.db").exists());
}

#[test]
fn workspace_validation_precedes_lazy_database_open() {
    let dir = tempfile::tempdir().unwrap();
    let error = service(dir.path())
        .query_workspace(HistoryWorkspaceQueryRequest {
            scope: HistoryWorkspaceScope::All,
            query: String::new(),
            filter_type: HistoryWorkspaceFilterType::All,
            date_filter: HistoryWorkspaceDateFilter::All,
            sort_order: HistoryWorkspaceSortOrder::Newest,
            limit: 0,
            offset: 0,
        })
        .unwrap_err();

    assert!(matches!(error, HistoryQueryError::InvalidRequest(_)));
    assert_eq!(
        error.to_string(),
        "Invalid history query: limit must be between 1 and 200"
    );
    assert!(!dir.path().join("sona.db").exists());
}

#[test]
fn valid_query_opens_sqlite_and_returns_the_page() {
    let dir = tempfile::tempdir().unwrap();
    let result = service(dir.path())
        .list_items(HistoryListOptions {
            limit: Some(10),
            offset: Some(0),
        })
        .unwrap();

    assert!(result.is_empty());
    assert!(dir.path().join("sona.db").is_file());
}
