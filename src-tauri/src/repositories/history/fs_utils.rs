use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::{Path, PathBuf};
use uuid::Uuid;

pub(crate) use crate::repositories::storage::remove_path_if_exists;
pub(crate) use crate::repositories::storage::write_json_pretty_atomic;

pub(crate) fn ensure_safe_file_name(value: &str, label: &str) -> Result<String, String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.contains("..")
        || trimmed.contains('/')
        || trimmed.contains('\\')
    {
        return Err(format!("{label} contains an invalid file name."));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn optional_history_child_path(root: &Path, file_name: &str) -> Option<PathBuf> {
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

pub(crate) fn ensure_json_array_value(value: Value, label: &str) -> Result<Value, String> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(format!("{label} must be an array."))
    }
}

pub(crate) fn ensure_json_object_value(value: Value, label: &str) -> Result<Value, String> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(format!("{label} must be an object."))
    }
}

pub(crate) fn read_json_value(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

pub(crate) fn create_temp_directory(prefix: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("sona-{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

pub(crate) fn create_tar_bz2_archive(source_dir: &Path, archive_path: &Path) -> Result<(), String> {
    fn append_directory_contents(
        builder: &mut tar::Builder<BzEncoder<BufWriter<File>>>,
        root: &Path,
        current: &Path,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let path = entry.path();
            let relative = path.strip_prefix(root).map_err(|error| error.to_string())?;

            if entry
                .file_type()
                .map_err(|error| error.to_string())?
                .is_dir()
            {
                builder
                    .append_dir(relative, &path)
                    .map_err(|error| error.to_string())?;
                append_directory_contents(builder, root, &path)?;
                continue;
            }

            builder
                .append_path_with_name(&path, relative)
                .map_err(|error| error.to_string())?;
        }

        Ok(())
    }

    if !source_dir.is_dir() {
        return Err(format!(
            "Source directory does not exist: {}",
            source_dir.to_string_lossy()
        ));
    }
    if let Some(parent) = archive_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let file = File::create(archive_path).map_err(|error| error.to_string())?;
    let writer = BufWriter::new(file);
    let encoder = BzEncoder::new(writer, bzip2::Compression::best());
    let mut builder = tar::Builder::new(encoder);
    append_directory_contents(&mut builder, source_dir, source_dir)?;
    let encoder = builder.into_inner().map_err(|error| error.to_string())?;
    encoder.finish().map_err(|error| error.to_string())?;
    Ok(())
}

pub(crate) fn extract_tar_bz2_archive(
    archive_path: &Path,
    target_dir: &Path,
) -> Result<(), String> {
    let file = File::open(archive_path).map_err(|error| error.to_string())?;
    let buffered = BufReader::new(file);
    let tar = BzDecoder::new(buffered);
    let mut archive = tar::Archive::new(tar);
    fs::create_dir_all(target_dir).map_err(|error| error.to_string())?;
    for entry in archive.entries().map_err(|error| error.to_string())? {
        let mut entry = entry.map_err(|error| error.to_string())?;
        entry
            .unpack_in(target_dir)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
