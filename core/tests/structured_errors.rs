use std::path::PathBuf;

use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use sona_core::ports::path::{PathKind, PathPortError};

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
fn path_error_keeps_machine_readable_context() {
    let path_error = PathPortError::new(PathKind::AppLocalData, "scope denied");

    assert_eq!(path_error.kind, PathKind::AppLocalData);
    assert!(path_error.to_string().contains("AppLocalData"));
}
