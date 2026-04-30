use bzip2::read::BzDecoder;
use bzip2::write::BzEncoder;
use serde::Serialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, ErrorKind, Write};
use std::path::{Path, PathBuf};
use uuid::Uuid;
use walkdir::WalkDir;

pub(super) fn ensure_safe_file_name(value: &str, label: &str) -> Result<String, String> {
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

pub(super) fn optional_history_child_path(root: &Path, file_name: &str) -> Option<PathBuf> {
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

pub(super) fn ensure_json_array_value(value: Value, label: &str) -> Result<Value, String> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(format!("{label} must be an array."))
    }
}

pub(super) fn ensure_json_object_value(value: Value, label: &str) -> Result<Value, String> {
    if value.is_object() {
        Ok(value)
    } else {
        Err(format!("{label} must be an object."))
    }
}

pub(super) fn read_json_value(path: &Path) -> Result<Value, String> {
    let content = fs::read_to_string(path).map_err(|error| error.to_string())?;
    serde_json::from_str(&content).map_err(|error| error.to_string())
}

pub(super) fn write_json_pretty_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_binary_atomic(path, &serialized)
}

pub(super) fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        Uuid::new_v4()
    ));
    {
        let mut writer =
            BufWriter::new(File::create(&temp_path).map_err(|error| error.to_string())?);
        writer
            .write_all(contents)
            .map_err(|error| error.to_string())?;
        writer.flush().map_err(|error| error.to_string())?;
    }

    replace_path_atomically(&temp_path, path)
}

pub(super) fn replace_path_atomically(temp_path: &Path, final_path: &Path) -> Result<(), String> {
    let backup_path = final_path.with_extension(format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4()
    ));
    let had_existing = final_path.exists();

    if had_existing {
        fs::rename(final_path, &backup_path).map_err(|error| error.to_string())?;
    }

    match fs::rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                remove_path_if_exists(&backup_path)?;
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && !final_path.exists() {
                let _ = fs::rename(&backup_path, final_path);
            }
            let _ = remove_path_if_exists(temp_path);
            Err(error.to_string())
        }
    }
}

pub(super) fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
    }
}

pub(super) fn copy_directory_recursive(source: &Path, target: &Path) -> Result<(), String> {
    if !source.is_dir() {
        return Err(format!(
            "Source directory does not exist: {}",
            source.to_string_lossy()
        ));
    }

    fs::create_dir_all(target).map_err(|error| error.to_string())?;
    for entry in WalkDir::new(source) {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let relative = path
            .strip_prefix(source)
            .map_err(|error| error.to_string())?;
        if relative.as_os_str().is_empty() {
            continue;
        }

        let destination = target.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir_all(&destination).map_err(|error| error.to_string())?;
            continue;
        }

        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::copy(path, &destination).map_err(|error| error.to_string())?;
    }

    Ok(())
}

pub(super) fn create_temp_directory(prefix: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("sona-{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

pub(super) fn create_tar_bz2_archive(source_dir: &Path, archive_path: &Path) -> Result<(), String> {
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

pub(super) fn extract_tar_bz2_archive(
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
