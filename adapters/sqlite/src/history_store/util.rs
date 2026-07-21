use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use std::path::Path;

use crate::DatabaseError;

pub(super) const STAGED_AUDIO_MARKER: &str = ".sona-staging-";
pub(super) const MILLIS_PER_DAY: u64 = 86_400_000;
pub(super) const HISTORY_DIR_NAME: &str = "history";
pub(super) const HISTORY_FILE_LOCK_NAME: &str = ".sona-history.lock";
pub(super) const TRANSCRIPT_SNAPSHOT_RETENTION_LIMIT: usize = 20;

pub(super) fn db_file_system_error(
    operation: FileSystemOperation,
    path: &Path,
    error: std::io::Error,
) -> DatabaseError {
    DatabaseError::FileSystem(FileSystemError::new(operation, path, error.to_string()))
}
