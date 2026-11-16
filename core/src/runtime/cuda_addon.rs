use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

#[cfg(feature = "specta")]
use specta::Type;

pub const CUDA_ADDON_DIR_NAME: &str = "cuda";
pub const CUDA_ADDON_SUBPATH: &str = "runtimes/cuda";
pub const CUDA_ADDON_MANIFEST_FILENAME: &str = "cuda-addon-manifest.json";
pub const EXPECTED_CUDA_ADDON_LLAMA_CPP_REF: &str = env!("SONA_CUDA_ADDON_LLAMA_CPP_REF");
pub const EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION: &str =
    env!("SONA_CUDA_ADDON_SHERPA_ONNX_VERSION");

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[cfg_attr(feature = "specta", derive(Type))]
#[serde(rename_all = "camelCase")]
pub struct CudaAddonInspection {
    pub is_installed: bool,
    pub is_active: bool,
    pub is_compatible: bool,
    pub path: String,
    pub missing_files: Vec<String>,
    pub validation_error: Option<String>,
    pub version: Option<String>,
    pub cuda_version: Option<String>,
    pub llama_cpp_ref: Option<String>,
    pub sherpa_onnx_version: Option<String>,
    pub required_llama_cpp_ref: String,
    pub required_sherpa_onnx_version: String,
    pub required_version: Option<String>,
    pub required_cuda_version: Option<String>,
    pub download_url: Option<String>,
    pub expected_sha256: Option<String>,
    pub update_available: bool,
    pub update_check_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CudaAddonManifest {
    #[serde(deserialize_with = "deserialize_schema_version")]
    pub schema_version: u32,
    pub addon_version: String,
    pub cuda_version: String,
    #[serde(default)]
    pub llama_cpp_ref: Option<String>,
    #[serde(default)]
    pub sherpa_onnx_version: Option<String>,
    #[serde(default)]
    pub target_os: Option<String>,
    #[serde(default)]
    pub target_arch: Option<String>,
    #[serde(default)]
    pub published_at: Option<String>,
    #[serde(default)]
    pub platforms: HashMap<String, CudaAddonPlatformEntry>,
}

fn deserialize_schema_version<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SchemaVersion {
        Number(u32),
        String(String),
    }
    match SchemaVersion::deserialize(deserializer)? {
        SchemaVersion::Number(version) => Ok(version),
        SchemaVersion::String(version) => version
            .split('.')
            .next()
            .unwrap_or_default()
            .parse()
            .map_err(serde::de::Error::custom),
    }
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
    #[error("CUDA addon manifest is invalid: {0}")]
    InvalidManifest(String),
    #[error("CUDA addon is incompatible with this client: {0}")]
    Incompatible(String),
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

fn required_cuda_addon_file_patterns(target_os: &str, cuda_major: &str) -> Vec<String> {
    match target_os {
        "windows" => vec![
            "onnxruntime_providers_cuda.dll".into(),
            "ggml-cuda.dll".into(),
            format!("cudart64_{cuda_major}.dll"),
            format!("cublas64_{cuda_major}.dll"),
            format!("cublasLt64_{cuda_major}.dll"),
        ],
        "linux" => vec![
            "libonnxruntime_providers_cuda.so".into(),
            "libggml-cuda.so".into(),
            format!("libcudart.so.{cuda_major}"),
            format!("libcublas.so.{cuda_major}"),
            format!("libcublasLt.so.{cuda_major}"),
        ],
        _ => Vec::new(),
    }
}

pub fn inspect_cuda_addon_directory(dir: &Path) -> CudaAddonInspection {
    let path_str = dir.to_string_lossy().to_string();
    let required = required_cuda_addon_files(std::env::consts::OS);
    let mut validation_error = None;
    let mut parsed_manifest = None;

    if !dir.is_dir() {
        return CudaAddonInspection {
            is_installed: false,
            is_active: false,
            is_compatible: false,
            path: path_str,
            missing_files: required.iter().map(|s| (*s).to_string()).collect(),
            validation_error: None,
            version: None,
            cuda_version: None,
            llama_cpp_ref: None,
            sherpa_onnx_version: None,
            required_llama_cpp_ref: EXPECTED_CUDA_ADDON_LLAMA_CPP_REF.to_string(),
            required_sherpa_onnx_version: EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION.to_string(),
            required_version: None,
            required_cuda_version: None,
            download_url: None,
            expected_sha256: None,
            update_available: false,
            update_check_error: None,
        };
    }

    let manifest_path = dir.join(CUDA_ADDON_MANIFEST_FILENAME);
    if !manifest_path.is_file() {
        validation_error = Some("CUDA addon manifest is missing".to_string());
    } else {
        match std::fs::read_to_string(&manifest_path)
            .map_err(|error| error.to_string())
            .and_then(|content| {
                serde_json::from_str::<CudaAddonManifest>(&content)
                    .map_err(|error| error.to_string())
            }) {
            Ok(manifest)
                if manifest.schema_version == 1
                    && !manifest.addon_version.trim().is_empty()
                    && !manifest.cuda_version.trim().is_empty()
                    && manifest
                        .llama_cpp_ref
                        .as_ref()
                        .is_some_and(|value| !value.trim().is_empty())
                    && manifest
                        .sherpa_onnx_version
                        .as_ref()
                        .is_some_and(|value| !value.trim().is_empty())
                    && manifest.target_os.as_deref() == Some(std::env::consts::OS)
                    && manifest.target_arch.as_deref() == Some(std::env::consts::ARCH) =>
            {
                parsed_manifest = Some(manifest)
            }
            Ok(_) => {
                validation_error =
                    Some("CUDA addon manifest has unsupported or missing fields".to_string())
            }
            Err(error) => {
                validation_error = Some(format!("CUDA addon manifest is invalid: {error}"))
            }
        }
    }

    let manifest = parsed_manifest.as_ref();
    let target_os = std::env::consts::OS;
    let cuda_major = manifest
        .and_then(|value| value.cuda_version.split('.').next())
        .unwrap_or("12");
    let required_files = required_cuda_addon_file_patterns(target_os, cuda_major);
    let missing_files: Vec<String> = required_files
        .iter()
        .filter(|file_name| !dir.join(file_name).is_file())
        .cloned()
        .collect();
    let is_installed = !required_files.is_empty() && missing_files.is_empty() && manifest.is_some();
    let llama_cpp_ref = manifest.and_then(|value| value.llama_cpp_ref.clone());
    let sherpa_onnx_version = manifest.and_then(|value| value.sherpa_onnx_version.clone());
    let is_compatible = is_installed
        && llama_cpp_ref.as_deref() == Some(EXPECTED_CUDA_ADDON_LLAMA_CPP_REF)
        && sherpa_onnx_version.as_deref() == Some(EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION)
        && manifest.is_some_and(|value| {
            value.target_os.as_deref().is_none_or(|os| os == target_os)
                && value
                    .target_arch
                    .as_deref()
                    .is_none_or(|arch| arch == std::env::consts::ARCH)
        });

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
        is_compatible,
        path: path_str,
        missing_files,
        validation_error,
        version: manifest.map(|value| value.addon_version.clone()),
        cuda_version: manifest.map(|value| value.cuda_version.clone()),
        llama_cpp_ref,
        sherpa_onnx_version,
        required_llama_cpp_ref: EXPECTED_CUDA_ADDON_LLAMA_CPP_REF.to_string(),
        required_sherpa_onnx_version: EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION.to_string(),
        required_version: None,
        required_cuda_version: None,
        download_url: None,
        expected_sha256: None,
        update_available: false,
        update_check_error: None,
    }
}

pub fn activate_cuda_addon_directory(
    dir: &Path,
) -> Result<CudaAddonInspection, CudaAddonActivationError> {
    if !dir.is_dir() {
        return Err(CudaAddonActivationError::NotFound(dir.to_path_buf()));
    }

    let inspection = inspect_cuda_addon_directory(dir);
    if !inspection.missing_files.is_empty() {
        return Err(CudaAddonActivationError::Incomplete(
            inspection.missing_files,
        ));
    }
    if !inspection.is_installed {
        return Err(CudaAddonActivationError::InvalidManifest(
            inspection
                .validation_error
                .unwrap_or_else(|| "required addon metadata is unavailable".to_string()),
        ));
    }
    if !inspection.is_compatible {
        return Err(CudaAddonActivationError::Incompatible(inspection.validation_error.unwrap_or_else(|| {
            format!("installed llama.cpp {:?}, sherpa-onnx {:?}; required llama.cpp {}, sherpa-onnx {}", inspection.llama_cpp_ref, inspection.sherpa_onnx_version, EXPECTED_CUDA_ADDON_LLAMA_CPP_REF, EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION)
        })));
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
            "llamaCppRef": EXPECTED_CUDA_ADDON_LLAMA_CPP_REF,
            "sherpaOnnxVersion": EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION,
            "targetOs": std::env::consts::OS,
            "targetArch": std::env::consts::ARCH,
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
            assert!(inspection.is_compatible);
            assert!(inspection.missing_files.is_empty());
        }
    }

    #[test]
    fn invalid_manifest_is_not_compatible() {
        let temp = test_temp_dir();
        for file in required_cuda_addon_files(std::env::consts::OS) {
            std::fs::write(temp.path().join(file), b"test").unwrap();
        }
        std::fs::write(temp.path().join(CUDA_ADDON_MANIFEST_FILENAME), b"{").unwrap();
        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(!inspection.is_installed);
        assert!(!inspection.is_compatible);
        assert!(inspection.validation_error.is_some());
    }

    #[test]
    fn dependency_mismatch_is_installed_but_not_compatible() {
        let temp = test_temp_dir();
        for file in required_cuda_addon_files(std::env::consts::OS) {
            std::fs::write(temp.path().join(file), b"test").unwrap();
        }
        let manifest = serde_json::json!({
            "schemaVersion": "1.0",
            "addonVersion": "0.2.0",
            "cudaVersion": "12.4",
            "llamaCppRef": "old-ref",
            "sherpaOnnxVersion": EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION,
            "targetOs": std::env::consts::OS,
            "targetArch": std::env::consts::ARCH,
        });
        std::fs::write(
            temp.path().join(CUDA_ADDON_MANIFEST_FILENAME),
            manifest.to_string(),
        )
        .unwrap();

        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(inspection.is_installed);
        assert!(!inspection.is_compatible);
        assert_eq!(inspection.version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn required_files_follow_cuda_major_version() {
        assert!(
            required_cuda_addon_file_patterns("windows", "13")
                .contains(&"cudart64_13.dll".to_string())
        );
        assert!(
            required_cuda_addon_file_patterns("linux", "11")
                .contains(&"libcublasLt.so.11".to_string())
        );
    }
}
