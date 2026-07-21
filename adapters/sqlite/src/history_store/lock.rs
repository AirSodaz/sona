use fs3::FileExt;
use sona_core::ports::fs::FileSystemOperation;
use std::fs;
use std::fs::{File, OpenOptions};
use std::path::Path;

use crate::DatabaseError;

use super::util::{HISTORY_FILE_LOCK_NAME, db_file_system_error};

#[must_use]
pub(crate) struct HistoryFileLockGuard {
    _file: File,
}

pub(crate) fn acquire_history_file_lock(
    app_local_data_dir: &Path,
) -> Result<HistoryFileLockGuard, DatabaseError> {
    fs::create_dir_all(app_local_data_dir).map_err(|error| {
        db_file_system_error(
            FileSystemOperation::CreateDirectory,
            app_local_data_dir,
            error,
        )
    })?;
    let lock_path = app_local_data_dir.join(HISTORY_FILE_LOCK_NAME);
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| db_file_system_error(FileSystemOperation::WriteFile, &lock_path, error))?;
    file.lock_exclusive()
        .map_err(|error| db_file_system_error(FileSystemOperation::WriteFile, &lock_path, error))?;
    Ok(HistoryFileLockGuard { _file: file })
}
