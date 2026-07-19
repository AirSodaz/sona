use std::fmt;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use std::time::Instant;

mod backup;

pub use backup::{
    FsBackupAdapter, FsBackupArchiveRepository, MAX_BACKUP_ENTRIES, MAX_BACKUP_EXPANDED_BYTES,
    MAX_BACKUP_FILE_BYTES,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArchiveOperation {
    InspectSource,
    OpenArchive,
    CreateTargetDirectory,
    ReadEntries,
    ReadEntry,
    ReadEntryPath,
    ExtractEntry,
    CreateArchiveParent,
    CreateArchive,
    ReadSourceDirectory,
    ReadSourceEntry,
    InspectSourceEntry,
    ResolveSourceEntry,
    AppendDirectory,
    AppendFile,
    FinishArchive,
}

impl fmt::Display for ArchiveOperation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            Self::InspectSource => "inspect source",
            Self::OpenArchive => "open archive",
            Self::CreateTargetDirectory => "create target directory",
            Self::ReadEntries => "read archive entries",
            Self::ReadEntry => "read archive entry",
            Self::ReadEntryPath => "read archive entry path",
            Self::ExtractEntry => "extract archive entry",
            Self::CreateArchiveParent => "create archive parent directory",
            Self::CreateArchive => "create archive",
            Self::ReadSourceDirectory => "read source directory",
            Self::ReadSourceEntry => "read source entry",
            Self::InspectSourceEntry => "inspect source entry",
            Self::ResolveSourceEntry => "resolve source entry",
            Self::AppendDirectory => "append directory",
            Self::AppendFile => "append file",
            Self::FinishArchive => "finish archive",
        };
        formatter.write_str(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArchiveError {
    pub operation: ArchiveOperation,
    pub source: PathBuf,
    pub target: Option<PathBuf>,
    pub reason: String,
}

impl ArchiveError {
    fn with_target(
        operation: ArchiveOperation,
        source: impl Into<PathBuf>,
        target: impl Into<PathBuf>,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            operation,
            source: source.into(),
            target: Some(target.into()),
            reason: reason.into(),
        }
    }
}

impl fmt::Display for ArchiveError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "Archive {} failed for {}",
            self.operation,
            self.source.display()
        )?;
        if let Some(target) = &self.target {
            write!(formatter, " -> {}", target.display())?;
        }
        write!(formatter, ": {}", self.reason)
    }
}

impl std::error::Error for ArchiveError {}

pub fn extract_tar_bz2<F>(
    archive_path: &str,
    target_dir: &str,
    mut on_progress: F,
) -> Result<(), ArchiveError>
where
    F: FnMut(&str),
{
    let archive_path = PathBuf::from(archive_path);
    let target_path = PathBuf::from(target_dir);
    let archive_error = |operation, reason| {
        ArchiveError::with_target(operation, &archive_path, &target_path, reason)
    };

    let file = File::open(&archive_path)
        .map_err(|error| archive_error(ArchiveOperation::OpenArchive, error.to_string()))?;
    let buffered = BufReader::new(file);
    let tar = bzip2::read::BzDecoder::new(buffered);
    let mut archive = tar::Archive::new(tar);
    fs::create_dir_all(&target_path).map_err(|error| {
        archive_error(ArchiveOperation::CreateTargetDirectory, error.to_string())
    })?;

    let mut last_emit = Instant::now();

    for entry in archive
        .entries()
        .map_err(|error| archive_error(ArchiveOperation::ReadEntries, error.to_string()))?
    {
        let mut entry =
            entry.map_err(|error| archive_error(ArchiveOperation::ReadEntry, error.to_string()))?;

        if last_emit.elapsed().as_millis() > 100 {
            let path = entry.path().map_err(|error| {
                archive_error(ArchiveOperation::ReadEntryPath, error.to_string())
            })?;
            on_progress(&path.to_string_lossy());
            last_emit = Instant::now();
        }

        entry
            .unpack_in(&target_path)
            .map_err(|error| archive_error(ArchiveOperation::ExtractEntry, error.to_string()))?;
    }

    Ok(())
}

pub fn create_tar_bz2(source_dir: &str, archive_path: &str) -> Result<(), ArchiveError> {
    fn append_directory_contents(
        builder: &mut tar::Builder<bzip2::write::BzEncoder<BufWriter<File>>>,
        root: &Path,
        current: &Path,
        archive_path: &Path,
    ) -> Result<(), ArchiveError> {
        for entry in fs::read_dir(current).map_err(|error| {
            ArchiveError::with_target(
                ArchiveOperation::ReadSourceDirectory,
                current,
                archive_path,
                error.to_string(),
            )
        })? {
            let entry = entry.map_err(|error| {
                ArchiveError::with_target(
                    ArchiveOperation::ReadSourceEntry,
                    current,
                    archive_path,
                    error.to_string(),
                )
            })?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|error| {
                ArchiveError::with_target(
                    ArchiveOperation::ResolveSourceEntry,
                    &path,
                    archive_path,
                    error.to_string(),
                )
            })?;

            if entry
                .file_type()
                .map_err(|error| {
                    ArchiveError::with_target(
                        ArchiveOperation::InspectSourceEntry,
                        &path,
                        archive_path,
                        error.to_string(),
                    )
                })?
                .is_dir()
            {
                builder.append_dir(relative, &path).map_err(|error| {
                    ArchiveError::with_target(
                        ArchiveOperation::AppendDirectory,
                        &path,
                        archive_path,
                        error.to_string(),
                    )
                })?;
                append_directory_contents(builder, root, &path, archive_path)?;
                continue;
            }

            builder
                .append_path_with_name(&path, relative)
                .map_err(|error| {
                    ArchiveError::with_target(
                        ArchiveOperation::AppendFile,
                        &path,
                        archive_path,
                        error.to_string(),
                    )
                })?;
        }

        Ok(())
    }

    let source_path = PathBuf::from(source_dir);
    let archive_path = PathBuf::from(archive_path);
    if !source_path.is_dir() {
        return Err(ArchiveError::with_target(
            ArchiveOperation::InspectSource,
            &source_path,
            &archive_path,
            format!("Source directory does not exist: {source_dir}"),
        ));
    }

    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            ArchiveError::with_target(
                ArchiveOperation::CreateArchiveParent,
                &source_path,
                &archive_path,
                error.to_string(),
            )
        })?;
    }

    let file = File::create(&archive_path).map_err(|error| {
        ArchiveError::with_target(
            ArchiveOperation::CreateArchive,
            &source_path,
            &archive_path,
            error.to_string(),
        )
    })?;
    let writer = BufWriter::new(file);
    let encoder = bzip2::write::BzEncoder::new(writer, bzip2::Compression::best());
    let mut builder = tar::Builder::new(encoder);

    append_directory_contents(&mut builder, &source_path, &source_path, &archive_path)?;

    let encoder = builder.into_inner().map_err(|error| {
        ArchiveError::with_target(
            ArchiveOperation::FinishArchive,
            &source_path,
            &archive_path,
            error.to_string(),
        )
    })?;
    encoder.finish().map_err(|error| {
        ArchiveError::with_target(
            ArchiveOperation::FinishArchive,
            &source_path,
            &archive_path,
            error.to_string(),
        )
    })?;

    Ok(())
}
