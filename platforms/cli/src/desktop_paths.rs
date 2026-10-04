use std::path::{Path, PathBuf};

use sona_core::models::paths::ModelsDirStatus;

pub fn default_models_dir() -> Option<PathBuf> {
    default_models_dir_with_env(|name| std::env::var_os(name))
}

pub(crate) fn default_models_dir_with_env<F>(read_env: F) -> Option<PathBuf>
where
    F: FnOnce(&str) -> Option<std::ffi::OsString>,
{
    if let Some(env_dir) = read_env("SONA_MODELS_DIR").filter(|s| !s.is_empty()) {
        return Some(PathBuf::from(env_dir));
    }
    sona_runtime_fs::default_desktop_models_dir()
}

pub fn models_dir_status(path: &Path) -> ModelsDirStatus {
    sona_runtime_fs::models_dir_status(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[test]
    fn default_models_dir_reads_sona_models_dir_when_present() {
        let custom = PathBuf::from("/custom/models");
        let resolved = default_models_dir_with_env(|name| {
            assert_eq!(name, "SONA_MODELS_DIR");
            Some(OsString::from(&custom))
        });
        assert_eq!(resolved, Some(custom));
    }

    #[test]
    fn default_models_dir_ignores_empty_env_var() {
        let resolved = default_models_dir_with_env(|_| Some(OsString::new()));
        assert_eq!(resolved, sona_runtime_fs::default_desktop_models_dir());
    }
}
