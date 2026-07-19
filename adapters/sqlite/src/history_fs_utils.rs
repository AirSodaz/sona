use serde::Serialize;
use serde_json::Value;
use sona_core::ports::fs::{FileSystemError, FileSystemOperation};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum HistoryFsError {
    #[error(transparent)]
    FileSystem(#[from] FileSystemError),
    #[error("Serialization error for {path}: {reason}")]
    Serialization { path: PathBuf, reason: String },
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum HistoryDataValidationError {
    #[error("{label} contains an invalid file name.")]
    InvalidFileName { label: String },
    #[error("{label} must be an array.")]
    ExpectedArray { label: String },
    #[error("{label} must be an object.")]
    ExpectedObject { label: String },
}

pub fn write_json_pretty_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), HistoryFsError> {
    let serialized =
        serde_json::to_vec_pretty(value).map_err(|error| HistoryFsError::Serialization {
            path: path.to_path_buf(),
            reason: error.to_string(),
        })?;
    write_binary_atomic(path, &serialized)
}

fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), HistoryFsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            FileSystemError::new(
                FileSystemOperation::CreateDirectory,
                parent,
                error.to_string(),
            )
        })?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        Uuid::new_v4()
    ));

    fs::write(&temp_path, contents).map_err(|error| {
        FileSystemError::new(
            FileSystemOperation::WriteFile,
            &temp_path,
            error.to_string(),
        )
    })?;
    replace_path_atomically(&temp_path, path).map_err(Into::into)
}

fn replace_path_atomically(temp_path: &Path, final_path: &Path) -> Result<(), FileSystemError> {
    let backup_name = format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4()
    );
    let backup_path = final_path.with_extension(&backup_name);
    let had_existing = match fs::metadata(final_path) {
        Ok(_) => true,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            return Err(FileSystemError::new(
                FileSystemOperation::Metadata,
                final_path,
                error.to_string(),
            ));
        }
    };

    if had_existing {
        fs::rename(final_path, &backup_path).map_err(|error| {
            FileSystemError::with_target(
                FileSystemOperation::Rename,
                final_path,
                &backup_path,
                error.to_string(),
            )
        })?;
    }

    match fs::rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                let _ = remove_path_if_exists(&backup_path);
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && !final_path.exists() {
                let _ = fs::rename(&backup_path, final_path);
            }
            let _ = remove_path_if_exists(temp_path);
            Err(FileSystemError::with_target(
                FileSystemOperation::Rename,
                temp_path,
                final_path,
                error.to_string(),
            ))
        }
    }
}

pub fn remove_path_if_exists(path: &Path) -> Result<(), FileSystemError> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => fs::remove_dir_all(path).map_err(|error| {
            FileSystemError::new(
                FileSystemOperation::RemoveDirectory,
                path,
                error.to_string(),
            )
        }),
        Ok(_) => fs::remove_file(path).map_err(|error| {
            FileSystemError::new(FileSystemOperation::RemoveFile, path, error.to_string())
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(FileSystemError::new(
            FileSystemOperation::Metadata,
            path,
            error.to_string(),
        )),
    }
}

pub fn ensure_safe_file_name(
    value: &str,
    label: &str,
) -> Result<String, HistoryDataValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(HistoryDataValidationError::InvalidFileName {
            label: label.to_string(),
        });
    }
    Ok(trimmed.to_string())
}

pub fn optional_history_child_path(root: &Path, file_name: &str) -> Option<PathBuf> {
    let trimmed = file_name.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return None;
    }
    Some(root.join(trimmed))
}

pub fn ensure_json_array_value(
    value: Value,
    label: &str,
) -> Result<Value, HistoryDataValidationError> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(HistoryDataValidationError::ExpectedArray {
            label: label.to_string(),
        })
    }
}

pub fn ensure_json_object_value(
    value: Value,
    label: &str,
) -> Result<Value, HistoryDataValidationError> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(HistoryDataValidationError::ExpectedObject {
            label: label.to_string(),
        })
    }
}

pub fn read_json_value(path: &Path) -> Result<Value, HistoryFsError> {
    let content = fs::read_to_string(path).map_err(|error| {
        FileSystemError::new(FileSystemOperation::ReadText, path, error.to_string())
    })?;
    serde_json::from_str(&content).map_err(|error| HistoryFsError::Serialization {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })
}
