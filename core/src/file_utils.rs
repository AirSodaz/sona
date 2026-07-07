use crate::ports::fs::{FileSystem, RealFileSystem};
use serde::Serialize;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub fn write_json_pretty_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), String> {
    write_json_pretty_atomic_with(&RealFileSystem, path, value)
}

pub fn write_json_pretty_atomic_with<T: Serialize + ?Sized>(
    fs: &dyn FileSystem,
    path: &Path,
    value: &T,
) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_binary_atomic(fs, path, &serialized)
}

fn write_binary_atomic(fs: &dyn FileSystem, path: &Path, contents: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs.create_dir_all(parent)?;
    }

    let temp_path = path.with_extension(format!(
        "{}.tmp-{}",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json"),
        Uuid::new_v4()
    ));

    fs.write_file(&temp_path, contents)?;

    replace_path_atomically(fs, &temp_path, path)
}

fn replace_path_atomically(
    fs: &dyn FileSystem,
    temp_path: &Path,
    final_path: &Path,
) -> Result<(), String> {
    let backup_name = format!(
        "{}.bak-{}",
        final_path
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("tmp"),
        Uuid::new_v4()
    );
    let backup_path = final_path.with_extension(&backup_name);
    let had_existing = fs.metadata(final_path)?.is_some();

    if had_existing {
        fs.rename(final_path, &backup_path)?;
    }

    match fs.rename(temp_path, final_path) {
        Ok(()) => {
            if had_existing {
                // Best-effort: stray .bak-* is harmless, never fail a successful write.
                let _ = remove_path_if_exists_with(fs, &backup_path);
            }
            Ok(())
        }
        Err(error) => {
            if had_existing && fs.metadata(final_path).ok().flatten().is_none() {
                let _ = fs.rename(&backup_path, final_path);
            }
            let _ = remove_path_if_exists_with(fs, temp_path);
            Err(error)
        }
    }
}

pub fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    remove_path_if_exists_with(&RealFileSystem, path)
}

pub fn remove_path_if_exists_with(fs: &dyn FileSystem, path: &Path) -> Result<(), String> {
    match fs.metadata(path)? {
        Some(meta) if meta.is_dir => fs.remove_dir_all(path),
        Some(_) => fs.remove_file(path),
        None => Ok(()),
    }
}

pub fn ensure_json_array_value(
    value: serde_json::Value,
    label: &str,
) -> Result<serde_json::Value, String> {
    if value.is_array() {
        Ok(value)
    } else {
        Err(format!("{label} must be an array."))
    }
}

pub fn current_time_millis() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use crate::ports::fs::{FileSystem, MockFileSystem};
    use serde_json::json;

    #[test]
    fn test_ensure_json_array_value() {
        let arr = json!([1, 2, 3]);
        let res = super::ensure_json_array_value(arr.clone(), "Test list");
        assert_eq!(res, Ok(arr));

        let obj = json!({"key": "value"});
        let res = super::ensure_json_array_value(obj, "Test list");
        assert_eq!(res, Err("Test list must be an array.".to_string()));

        let null_val = json!(null);
        let res = super::ensure_json_array_value(null_val, "Test list");
        assert_eq!(res, Err("Test list must be an array.".to_string()));
    }

    #[test]
    fn write_json_pretty_atomic_with_uses_mock_fs() {
        let fs = MockFileSystem::new();
        let path = std::path::Path::new("/tmp/test.json");
        let value = json!({"key": "value"});

        super::write_json_pretty_atomic_with(&fs, path, &value).unwrap();

        let files = fs.files.lock().unwrap();
        assert!(files.contains_key("/tmp/test.json"));
        let content: serde_json::Value =
            serde_json::from_slice(files.get("/tmp/test.json").unwrap()).unwrap();
        assert_eq!(content["key"], "value");
    }

    #[test]
    fn remove_path_if_exists_with_mock_fs() {
        let fs = MockFileSystem::new();
        let path = std::path::Path::new("/tmp/data.bin");

        fs.write_file(path, b"data").unwrap();
        assert!(fs.metadata(path).unwrap().is_some());

        super::remove_path_if_exists_with(&fs, path).unwrap();
        assert!(fs.metadata(path).unwrap().is_none());

        // removing non-existing path should not error
        super::remove_path_if_exists_with(&fs, path).unwrap();
    }
}
