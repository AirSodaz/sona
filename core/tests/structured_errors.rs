use std::path::PathBuf;

use sona_core::automation::AutomationError;
use sona_core::config::ConfigError;
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use sona_core::ports::path::{PathKind, PathProviderError};
use sona_core::ports::time::ClockError;
use sona_core::recovery::RecoveryError;
use sona_core::tag::TagError;
use sona_core::task_ledger::TaskLedgerError;

#[test]
fn filesystem_error_preserves_operation_and_both_paths() {
    let error = FileSystemError::with_target(
        FileSystemOperation::Rename,
        PathBuf::from("source.tmp"),
        PathBuf::from("target.json"),
        "access denied",
    );

    assert_eq!(error.operation, FileSystemOperation::Rename);
    assert_eq!(error.path, PathBuf::from("source.tmp"));
    assert_eq!(error.target, Some(PathBuf::from("target.json")));
    assert!(error.to_string().contains("source.tmp"));
    assert!(error.to_string().contains("target.json"));
}

#[test]
fn path_and_clock_errors_keep_machine_readable_context() {
    let path_error = PathProviderError::new(PathKind::AppLocalData, "scope denied");
    let clock_error = ClockError::Unavailable("clock offline".to_string());

    assert_eq!(path_error.kind, PathKind::AppLocalData);
    assert!(path_error.to_string().contains("AppLocalData"));
    assert!(matches!(clock_error, ClockError::Unavailable(_)));
}

#[test]
fn domain_errors_distinguish_repository_serialization_and_clock_failures() {
    let config = ConfigError::Repository("config store".to_string());
    let tag = TagError::Repository("tag store".to_string());
    let automation = AutomationError::Repository("automation store".to_string());
    let recovery = RecoveryError::Path(PathProviderError::new(
        PathKind::AppLocalData,
        "recovery path",
    ));
    let ledger_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
    let ledger = TaskLedgerError::Serialization(ledger_error);

    assert!(matches!(config, ConfigError::Repository(_)));
    assert!(matches!(tag, TagError::Repository(_)));
    assert!(matches!(automation, AutomationError::Repository(_)));
    assert!(matches!(recovery, RecoveryError::Path(_)));
    assert!(matches!(ledger, TaskLedgerError::Serialization(_)));
}
