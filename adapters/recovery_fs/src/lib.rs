use serde::Serialize;
use sona_core::ports::fs::{FileMetadata, FileSystem, FileSystemError, FileSystemOperation};
use sona_core::ports::time::{ClockError, UnixMillisClock};
use sona_core::recovery::RecoveryError;
use sona_core::recovery::normalization::{SourcePathStatus, SourcePathStatusProvider, empty_snapshot};
use sona_core::recovery::repository::RecoverySnapshotStore;
use sona_core::recovery::service::RecoveryService;
use sona_core::recovery::types::{
    QUEUE_RECOVERY_FILE_NAME, RECOVERY_DIR_NAME, RecoveryItemInput, RecoverySnapshot,
    RecoverySnapshotInput,
};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default)]
struct RealFileSystem;

#[derive(Clone, Copy, Debug, Default)]
struct SystemClock;

#[derive(Clone, Copy, Debug, Default)]
struct FsSourcePathStatusProvider;

impl FileSystem for RealFileSystem {
    fn create_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::create_dir_all(path)
            .map_err(|error| file_system_error(FileSystemOperation::CreateDirectory, path, error))
    }

    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), FileSystemError> {
        if let Some(parent) = path.parent() {
            self.create_dir_all(parent)?;
        }
        fs::write(path, contents)
            .map_err(|error| file_system_error(FileSystemOperation::WriteFile, path, error))
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, FileSystemError> {
        fs::read(path).map_err(|error| file_system_error(FileSystemOperation::ReadFile, path, error))
    }

    fn read_to_string(&self, path: &Path) -> Result<String, FileSystemError> {
        fs::read_to_string(path)
            .map_err(|error| file_system_error(FileSystemOperation::ReadText, path, error))
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), FileSystemError> {
        fs::rename(from, to).map_err(|error| {
            FileSystemError::with_target(FileSystemOperation::Rename, from, to, error.to_string())
        })
    }

    fn remove_file(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_file(path)
            .map_err(|error| file_system_error(FileSystemOperation::RemoveFile, path, error))
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), FileSystemError> {
        fs::remove_dir_all(path)
            .map_err(|error| file_system_error(FileSystemOperation::RemoveDirectory, path, error))
    }

    fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>, FileSystemError> {
        match fs::metadata(path) {
            Ok(metadata) => Ok(Some(FileMetadata {
                is_file: metadata.is_file(),
                is_dir: metadata.is_dir(),
            })),
            Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
            Err(error) => Err(file_system_error(FileSystemOperation::Metadata, path, error)),
        }
    }
}

impl UnixMillisClock for SystemClock {
    fn now_ms(&self) -> Result<u64, ClockError> {
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| ClockError::BeforeUnixEpoch(error.to_string()))?;
        u64::try_from(duration.as_millis()).map_err(|error| ClockError::OutOfRange(error.to_string()))
    }
}

impl SourcePathStatusProvider for FsSourcePathStatusProvider {
    fn status_for_path(&self, path: &str) -> SourcePathStatus {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_file() => SourcePathStatus::File,
            Ok(metadata) if metadata.is_dir() => SourcePathStatus::Directory,
            Ok(_) => SourcePathStatus::Unknown,
            Err(error) if error.kind() == ErrorKind::NotFound => SourcePathStatus::Missing,
            Err(_) => SourcePathStatus::Unknown,
        }
    }
}

fn file_system_error(
    operation: FileSystemOperation,
    path: &Path,
    error: std::io::Error,
) -> FileSystemError {
    FileSystemError::new(operation, path, error.to_string())
}

fn ensure_directory_exists(path: &Path) -> Result<(), FileSystemError> {
    RealFileSystem.create_dir_all(path)
}

fn write_json_pretty_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), FileSystemError> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| {
        FileSystemError::new(
            FileSystemOperation::WriteFile,
            path,
            format!("Failed to serialize recovery JSON: {error}"),
        )
    })?;
    write_binary_atomic(path, &serialized)
}

fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), FileSystemError> {
    let fs = RealFileSystem;
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        unique_temp_token()
    ));

    fs.write_file(&temp_path, contents)?;
    replace_path_atomically(&temp_path, path)
}

fn replace_path_atomically(temp_path: &Path, final_path: &Path) -> Result<(), FileSystemError> {
    let fs = RealFileSystem;
    let backup_path = final_path.with_extension(format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        unique_temp_token()
    ));
    let had_existing = fs.metadata(final_path)?.is_some();

    if had_existing {
        fs.rename(final_path, &backup_path)?;
    }

    match fs.rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                let _ = fs.remove_file(&backup_path);
            }
            Ok(())
        }
        Err(error) => {
            if had_existing {
                let _ = fs.rename(&backup_path, final_path);
            }
            let _ = fs.remove_file(temp_path);
            Err(error)
        }
    }
}

fn unique_temp_token() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let counter = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{millis}-{counter}")
}

pub struct FsRecoveryAdapter {
    store: FsRecoverySnapshotStore,
    source_paths: FsSourcePathStatusProvider,
    clock: SystemClock,
}

impl FsRecoveryAdapter {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self {
            store: FsRecoverySnapshotStore::new(app_local_data_dir),
            source_paths: FsSourcePathStatusProvider,
            clock: SystemClock,
        }
    }

    pub fn load_snapshot(&self) -> Result<RecoverySnapshot, RecoveryError> {
        self.service().load_snapshot()
    }

    pub fn load_snapshot_at(&self, now_ms: u64) -> Result<RecoverySnapshot, RecoveryError> {
        self.service().load_snapshot_at(now_ms)
    }

    pub fn save_snapshot(
        &self,
        items: Vec<RecoveryItemInput>,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        self.service().save_snapshot(items)
    }

    pub fn save_snapshot_at(
        &self,
        items: Vec<RecoveryItemInput>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        self.service().save_snapshot_at(items, now_ms)
    }

    pub fn persist_queue_snapshot(
        &self,
        queue_items: Vec<RecoveryItemInput>,
        resolved_ids: Vec<String>,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        self.service()
            .persist_queue_snapshot(queue_items, resolved_ids)
    }

    pub fn persist_queue_snapshot_at(
        &self,
        queue_items: Vec<RecoveryItemInput>,
        resolved_ids: Vec<String>,
        now_ms: u64,
    ) -> Result<RecoverySnapshot, RecoveryError> {
        self.service()
            .persist_queue_snapshot_at(queue_items, resolved_ids, now_ms)
    }

    fn service(&self) -> RecoveryService<'_> {
        RecoveryService::new(&self.store, &self.source_paths, &self.clock)
    }
}

pub struct FsRecoverySnapshotStore {
    app_local_data_dir: PathBuf,
}

impl FsRecoverySnapshotStore {
    pub fn new(app_local_data_dir: PathBuf) -> Self {
        Self { app_local_data_dir }
    }

    pub fn recovery_dir(&self) -> PathBuf {
        self.app_local_data_dir.join(RECOVERY_DIR_NAME)
    }

    pub fn queue_recovery_path(&self) -> PathBuf {
        self.recovery_dir().join(QUEUE_RECOVERY_FILE_NAME)
    }
}

impl RecoverySnapshotStore for FsRecoverySnapshotStore {
    fn load_snapshot_input(&self) -> Result<RecoverySnapshotInput, RecoveryError> {
        let recovery_dir = self.recovery_dir();
        ensure_directory_exists(&recovery_dir)
            .map_err(|error| RecoveryError::Repository(error.to_string()))?;
        let recovery_path = self.queue_recovery_path();
        if RealFileSystem
            .metadata(&recovery_path)
            .map_err(|error| RecoveryError::Repository(error.to_string()))?
            .is_none()
        {
            write_json_pretty_atomic(&recovery_path, &empty_snapshot())
                .map_err(|error| RecoveryError::Repository(error.to_string()))?;
        }

        let content = RealFileSystem
            .read_to_string(&recovery_path)
            .map_err(|error| RecoveryError::Repository(error.to_string()))?;
        match serde_json::from_str::<RecoverySnapshotInput>(&content) {
            Ok(input) => Ok(input),
            Err(error) => {
                log::error!("[Recovery] Failed to parse recovery snapshot: {}", error);
                Ok(RecoverySnapshotInput::default())
            }
        }
    }

    fn save_snapshot(&self, snapshot: &RecoverySnapshot) -> Result<(), RecoveryError> {
        write_json_pretty_atomic(&self.queue_recovery_path(), snapshot)
            .map_err(|error| RecoveryError::Repository(error.to_string()))
    }
}
