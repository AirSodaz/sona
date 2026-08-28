use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

#[cfg(feature = "specta")]
use specta::Type;

pub const CUDA_ADDON_DIR_NAME: &str = "cuda";
pub const CUDA_ADDON_SUBPATH: &str = "runtimes/cuda";
pub const CUDA_ADDON_MANIFEST_FILENAME: &str = "cuda-addon-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct CudaAddonInspection {
    pub is_installed: bool,
    pub is_active: bool,
    pub path: String,
    pub missing_files: Vec<String>,
    pub version: Option<String>,
    pub cuda_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CudaAddonManifest {
    pub schema_version: u32,
    pub addon_version: String,
    pub cuda_version: String,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub platforms: HashMap<String, CudaAddonPlatformEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CudaAddonPlatformEntry {
    pub filename: String,
    pub url: String,
    pub sha256: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CudaAddonActivationError {
    #[error("CUDA addon directory does not exist: {0}")]
    NotFound(PathBuf),
    #[error("CUDA addon is missing required files: {0:?}")]
    Incomplete(Vec<String>),
    #[error("Failed to register DLL directory on Windows: {0}")]
    SystemError(String),
}

static ACTIVE_CUDA_ADDON_DIR: LazyLock<Mutex<Option<PathBuf>>> = LazyLock::new(|| Mutex::new(None));

pub fn required_cuda_addon_files(target_os: &str) -> &'static [&'static str] {
    match target_os {
        "windows" => &[
            "onnxruntime_providers_cuda.dll",
            "ggml-cuda.dll",
            "cudart64_12.dll",
            "cublas64_12.dll",
            "cublasLt64_12.dll",
        ],
        "linux" => &[
            "libonnxruntime_providers_cuda.so",
            "libggml-cuda.so",
            "libcudart.so.12",
            "libcublas.so.12",
            "libcublasLt.so.12",
        ],
        _ => &[],
    }
}

pub fn inspect_cuda_addon_directory(dir: &Path) -> CudaAddonInspection {
    let path_str = dir.to_string_lossy().to_string();
    let required = required_cuda_addon_files(std::env::consts::OS);

    if !dir.is_dir() {
        return CudaAddonInspection {
            is_installed: false,
            is_active: false,
            path: path_str,
            missing_files: required.iter().map(|s| (*s).to_string()).collect(),
            version: None,
            cuda_version: None,
        };
    }

    let mut missing_files = Vec::new();
    for file_name in required {
        if !dir.join(file_name).exists() {
            missing_files.push((*file_name).to_string());
        }
    }

    let is_installed = missing_files.is_empty() && !required.is_empty();

    let mut version = None;
    let mut cuda_version = None;

    let manifest_path = dir.join(CUDA_ADDON_MANIFEST_FILENAME);
    if manifest_path.is_file() {
        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<CudaAddonManifest>(&content) {
                version = Some(manifest.addon_version);
                cuda_version = Some(manifest.cuda_version);
            }
        }
    }

    if version.is_none() {
        let version_path = dir.join("version.txt");
        if version_path.is_file() {
            if let Ok(content) = std::fs::read_to_string(&version_path) {
                let trimmed = content.trim().to_string();
                if !trimmed.is_empty() {
                    version = Some(trimmed);
                }
            }
        }
    }

    let active_dir = active_cuda_addon_dir();
    let is_active = active_dir.as_ref().is_some_and(|active| {
        if active == dir {
            return true;
        }
        if let (Ok(active_canonical), Ok(dir_canonical)) =
            (active.canonicalize(), dir.canonicalize())
        {
            return active_canonical == dir_canonical;
        }
        false
    });

    CudaAddonInspection {
        is_installed,
        is_active,
        path: path_str,
        missing_files,
        version,
        cuda_version,
    }
}

pub fn activate_cuda_addon_directory(
    dir: &Path,
) -> Result<CudaAddonInspection, CudaAddonActivationError> {
    if !dir.is_dir() {
        return Err(CudaAddonActivationError::NotFound(dir.to_path_buf()));
    }

    let inspection = inspect_cuda_addon_directory(dir);
    if !inspection.is_installed {
        return Err(CudaAddonActivationError::Incomplete(
            inspection.missing_files,
        ));
    }

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::ffi::OsStrExt;
        unsafe extern "system" {
            fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
        }
        let wide: Vec<u16> = dir
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let res = unsafe { SetDllDirectoryW(wide.as_ptr()) };
        if res == 0 {
            return Err(CudaAddonActivationError::SystemError(
                std::io::Error::last_os_error().to_string(),
            ));
        }
    }

    if let Ok(mut lock) = ACTIVE_CUDA_ADDON_DIR.lock() {
        *lock = Some(dir.to_path_buf());
    }

    let mut active_inspection = inspection;
    active_inspection.is_active = true;
    Ok(active_inspection)
}

pub fn deactivate_cuda_addon() {
    #[cfg(target_os = "windows")]
    {
        unsafe extern "system" {
            fn SetDllDirectoryW(lpPathName: *const u16) -> i32;
        }
        unsafe {
            SetDllDirectoryW(std::ptr::null());
        }
    }

    if let Ok(mut lock) = ACTIVE_CUDA_ADDON_DIR.lock() {
        *lock = None;
    }
}

pub fn is_cuda_addon_active() -> bool {
    ACTIVE_CUDA_ADDON_DIR
        .lock()
        .map(|guard| guard.is_some())
        .unwrap_or(false)
}

pub fn active_cuda_addon_dir() -> Option<PathBuf> {
    ACTIVE_CUDA_ADDON_DIR
        .lock()
        .ok()
        .and_then(|guard| guard.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_temp_dir() -> tempfile::TempDir {
        let target_temp = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../target/test-temp");
        std::fs::create_dir_all(&target_temp).ok();
        tempfile::tempdir_in(&target_temp).unwrap_or_else(|_| tempfile::tempdir().unwrap())
    }

    #[test]
    fn required_files_vary_by_platform() {
        assert_eq!(required_cuda_addon_files("windows").len(), 5);
        assert_eq!(required_cuda_addon_files("linux").len(), 5);
        assert_eq!(required_cuda_addon_files("macos").len(), 0);
    }

    #[test]
    fn inspect_empty_or_missing_directory_reports_not_installed() {
        let temp = test_temp_dir();
        let inspection = inspect_cuda_addon_directory(temp.path());

        #[cfg(any(target_os = "windows", target_os = "linux"))]
        {
            assert!(!inspection.is_installed);
            assert_eq!(inspection.missing_files.len(), 5);
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux")))]
        {
            assert!(!inspection.is_installed);
        }
    }

    #[test]
    fn inspect_complete_directory_detects_version_and_installation() {
        let temp = test_temp_dir();
        let required = required_cuda_addon_files(std::env::consts::OS);
        for file in required {
            std::fs::write(temp.path().join(file), b"test").unwrap();
        }

        let manifest = serde_json::json!({
            "schemaVersion": 1,
            "addonVersion": "0.1.0",
            "cudaVersion": "12.4",
            "platforms": {}
        });
        std::fs::write(
            temp.path().join(CUDA_ADDON_MANIFEST_FILENAME),
            manifest.to_string(),
        )
        .unwrap();

        let inspection = inspect_cuda_addon_directory(temp.path());
        if !required.is_empty() {
            assert!(inspection.is_installed);
            assert_eq!(inspection.version.as_deref(), Some("0.1.0"));
            assert_eq!(inspection.cuda_version.as_deref(), Some("12.4"));
            assert!(inspection.missing_files.is_empty());
        }
    }
}
