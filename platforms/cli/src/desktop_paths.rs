use std::path::PathBuf;

pub fn default_models_dir() -> Option<PathBuf> {
    default_models_dir_candidates()
        .into_iter()
        .map(|path| path.join("models"))
        .find(|path| path.exists())
        .or_else(|| {
            default_models_dir_candidates()
                .into_iter()
                .next()
                .map(|path| path.join("models"))
        })
}

fn default_models_dir_candidates() -> Vec<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        let Some(base) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) else {
            return Vec::new();
        };
        vec![base.join("com.asoda.sona"), base.join("Sona")]
    }

    #[cfg(target_os = "macos")]
    {
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let base = home.join("Library").join("Application Support");
        vec![base.join("com.asoda.sona"), base.join("Sona")]
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(data_home) = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from) {
            return vec![data_home.join("com.asoda.sona"), data_home.join("Sona")];
        }
        let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
            return Vec::new();
        };
        let base = home.join(".local").join("share");
        vec![base.join("com.asoda.sona"), base.join("Sona")]
    }

    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Vec::new()
    }
}
