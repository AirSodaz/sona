use std::path::Path;

#[derive(Debug, Clone)]
pub struct FileMetadata {
    pub is_file: bool,
    pub is_dir: bool,
}

pub trait FileSystem: Send + Sync {
    fn create_dir_all(&self, path: &Path) -> Result<(), String>;
    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), String>;
    fn read_file(&self, path: &Path) -> Result<Vec<u8>, String>;
    fn read_to_string(&self, path: &Path) -> Result<String, String>;
    fn rename(&self, from: &Path, to: &Path) -> Result<(), String>;
    fn remove_file(&self, path: &Path) -> Result<(), String>;
    fn remove_dir_all(&self, path: &Path) -> Result<(), String>;
    fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>, String>;
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
    fn create_dir_all(&self, _path: &Path) -> Result<(), String> {
        Ok(())
    }

    fn write_file(&self, path: &Path, contents: &[u8]) -> Result<(), String> {
        let key = path.to_string_lossy().to_string();
        let mut files = self.files.lock().map_err(|e| e.to_string())?;
        // Simulate parent directory creation like the runtime filesystem adapter does.
        if let Some(parent) = path.parent() {
            let parent_key = parent.to_string_lossy().to_string();
            files.entry(parent_key).or_insert_with(Vec::new);
        }
        files.insert(key, contents.to_vec());
        Ok(())
    }

    fn read_file(&self, path: &Path) -> Result<Vec<u8>, String> {
        let key = path.to_string_lossy().to_string();
        let files = self.files.lock().map_err(|e| e.to_string())?;
        files
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("file not found: {key}"))
    }

    fn read_to_string(&self, path: &Path) -> Result<String, String> {
        let bytes = self.read_file(path)?;
        String::from_utf8(bytes).map_err(|e| e.to_string())
    }

    fn rename(&self, from: &Path, to: &Path) -> Result<(), String> {
        let from_key = from.to_string_lossy().to_string();
        let to_key = to.to_string_lossy().to_string();
        let mut files = self.files.lock().map_err(|e| e.to_string())?;
        if let Some(data) = files.remove(&from_key) {
            files.insert(to_key, data);
        }
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<(), String> {
        let key = path.to_string_lossy().to_string();
        let mut files = self.files.lock().map_err(|e| e.to_string())?;
        files.remove(&key);
        Ok(())
    }

    fn remove_dir_all(&self, path: &Path) -> Result<(), String> {
        let prefix = path.to_string_lossy().to_string();
        let mut files = self.files.lock().map_err(|e| e.to_string())?;
        files.retain(|k, _| !k.starts_with(&prefix));
        Ok(())
    }

    fn metadata(&self, path: &Path) -> Result<Option<FileMetadata>, String> {
        let key = path.to_string_lossy().to_string();
        let files = self.files.lock().map_err(|e| e.to_string())?;
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
