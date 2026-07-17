use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileMetadata {
    pub is_file: bool,
    pub is_dir: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSystemOperation {
    CreateDirectory,
    WriteFile,
    ReadFile,
    ReadText,
    Rename,
    RemoveFile,
    RemoveDirectory,
    Metadata,
}

impl fmt::Display for FileSystemOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::CreateDirectory => "create directory",
            Self::WriteFile => "write file",
            Self::ReadFile => "read file",
            Self::ReadText => "read text",
            Self::Rename => "rename",
            Self::RemoveFile => "remove file",
            Self::RemoveDirectory => "remove directory",
            Self::Metadata => "read metadata",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSystemError {
    pub operation: FileSystemOperation,
    pub path: PathBuf,
    pub target: Option<PathBuf>,
    pub reason: String,
}

impl FileSystemError {
    pub fn new(
        operation: FileSystemOperation,
        path: impl Into<PathBuf>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            path: path.into(),
            target: None,
            reason: reason.into(),
        }
    }

    pub fn with_target(
        operation: FileSystemOperation,
        path: impl Into<PathBuf>,
        target: impl Into<PathBuf>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            path: path.into(),
            target: Some(target.into()),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for FileSystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Filesystem {} failed for {}",
            self.operation,
            self.path.display()
        )?;
        if let Some(target) = &self.target {
            write!(formatter, " -> {}", target.display())?;
        }
        write!(formatter, ": {}", self.reason)
    }
}

impl std::error::Error for FileSystemError {}

pub trait FileSystem: Send + Sync {
    fn create_dir_all(&self, path: &Path) -> Result<(), FileSystemError>;
    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError>;
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError>;
    fn read_to_string(&self, path: &Path) -> Result<String, FileSystemError>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError>;
    fn remove_file(&self, path: &Path) -> Result<(), FileSystemError>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError>;
    fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>, FileSystemError>;
}

#[cfg(test)]
pub struct MockFileSystem {
    pub files: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

#[cfg(test)]
impl MockFileSystem {
    pub fn new() -> Self {
        Self {
            files: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }
}

#[cfg(test)]
impl Default for MockFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl FileSystem for MockFileSystem {
    fn create_dir_all(&self, _path: &Path) -> Result<(), FileSystemError> {
        Ok(())
    }

    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError> {
        let key = path.to_string_lossy().to_string();
        let mut files = self.files.lock().unwrap();
        // Simulate parent directory creation like the runtime filesystem adapter does.
        if let Some(parent) = path.parent() {
            let parent_key = parent.to_string_lossy().to_string();
            files.entry(parent_key).or_insert_with(Vec::new);
        }
        files.insert(key, contents.to_vec());
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        let key = path.to_string_lossy().to_string();
        let files = self.files.lock().unwrap();
        files.get(&key).cloned().ok_or_else(|| {
            FileSystemError::new(
                FileSystemOperation::ReadFile,
                path,
                format!("file not found: {key}"),
            )
        })
    }

    fn read_to_string(&self, path: &Path) -> Result<String, FileSystemError> {
        let bytes = self.read_file(path)?;
        String::from_utf8(bytes).map_err(|error| {
            FileSystemError::new(FileSystemOperation::ReadText, path, error.to_string())
        })
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        let from_key = from.to_string_lossy().to_string();
        let to_key = to.to_string_lossy().to_string();
        let mut files = self.files.lock().unwrap();
        if let Some(data) = files.remove(&from_key) {
            files.insert(to_key, data);
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), FileSystemError> {
        let key = path.to_string_lossy().to_string();
        let mut files = self.files.lock().unwrap();
        files.remove(&key);
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        let prefix = path.to_string_lossy().to_string();
        let mut files = self.files.lock().unwrap();
        files.retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }

    fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>, FileSystemError> {
        let key = path.to_string_lossy().to_string();
        let files = self.files.lock().unwrap();
        Ok(if files.contains_key(&key) {
            Some(FileMetadata {
                is_file: true,
                is_dir: false,
            })
        } else {
            None
        })
    }
}
