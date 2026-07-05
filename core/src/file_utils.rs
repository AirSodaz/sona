use serde::Serialize;
use std::fs::{self, File};
use std::io::{BufWriter, ErrorKind, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub fn write_json_pretty_atomic<T: Serialize + ?Sized>(
    path: &Path,
    value: &T,
) -> Result<(), String> {
    let serialized = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    write_binary_atomic(path, &serialized)
}

fn write_binary_atomic(path: &Path, contents: &[u8]) -> Result<(), String> {
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

fn replace_path_atomically(temp_path: &Path, final_path: &Path) -> Result<(), String> {
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

pub fn remove_path_if_exists(path: &Path) -> Result<(), String> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            fs::remove_dir_all(path).map_err(|error| error.to_string())
        }
        Ok(_) => fs::remove_file(path).map_err(|error| error.to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.to_string()),
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
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ensure_json_array_value() {
        let arr = json!([1, 2, 3]);
        let res = ensure_json_array_value(arr.clone(), "Test list");
        assert_eq!(res, Ok(arr));

        let obj = json!({"key": "value"});
        let res = ensure_json_array_value(obj, "Test list");
        assert_eq!(res, Err("Test list must be an array.".to_string()));

        let null_val = json!(null);
        let res = ensure_json_array_value(null_val, "Test list");
        assert_eq!(res, Err("Test list must be an array.".to_string()));
    }
}
