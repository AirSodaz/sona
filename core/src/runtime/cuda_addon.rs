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

pub const DEFAULT_CUDA_MAJOR_VERSION: &str = "12";

fn cuda_major_version(cuda_version: &str) -> &str {
    cuda_version
        .split('.')
        .next()
        .filter(|major| !major.is_empty())
        .unwrap_or(DEFAULT_CUDA_MAJOR_VERSION)
}

pub fn required_cuda_addon_files(target_os: &str, cuda_version: &str) -> Vec<String> {
    let major = cuda_major_version(cuda_version);
    match target_os {
        "windows" => {
            // CUDA 11 keeps the minor digits in the runtime name (`cudart64_110.dll`),
            // CUDA 12+ drops them (`cudart64_12.dll`).
            let cudart = if major == "11" {
                "cudart64_110.dll".to_string()
            } else {
                format!("cudart64_{major}.dll")
            };
            vec![
                "onnxruntime_providers_cuda.dll".to_string(),
                "ggml-cuda.dll".to_string(),
                cudart,
                format!("cublas64_{major}.dll"),
                format!("cublasLt64_{major}.dll"),
            ]
        }
        "linux" => {
            let cudart = if major == "11" {
                "libcudart.so.11.0".to_string()
            } else {
                format!("libcudart.so.{major}")
            };
            vec![
                "libonnxruntime_providers_cuda.so".to_string(),
                "libggml-cuda.so".to_string(),
                cudart,
                format!("libcublas.so.{major}"),
                format!("libcublasLt.so.{major}"),
            ]
        }
        _ => Vec::new(),
    }
}

fn validate_cuda_addon_manifest(manifest: &CudaAddonManifest) -> Result<(), String> {
    if manifest.schema_version != 1 {
        return Err(format!(
            "unsupported manifest schema version {}",
            manifest.schema_version
        ));
    }
    if manifest.addon_version.trim().is_empty() {
        return Err("manifest is missing the addon version".to_string());
    }
    if manifest.cuda_version.trim().is_empty() {
        return Err("manifest is missing the CUDA version".to_string());
    }
    if manifest
        .llama_cpp_ref
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("manifest is missing the llama.cpp revision".to_string());
    }
    if manifest
        .sherpa_onnx_version
        .as_deref()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("manifest is missing the sherpa-onnx version".to_string());
    }
    match manifest.target_os.as_deref() {
        Some(os) if os == std::env::consts::OS => {}
        Some(os) => {
            return Err(format!(
                "manifest targets {os}, but this client runs {}",
                std::env::consts::OS
            ));
        }
        None => return Err("manifest is missing the target operating system".to_string()),
    }
    match manifest.target_arch.as_deref() {
        Some(arch) if arch == std::env::consts::ARCH => {}
        Some(arch) => {
            return Err(format!(
                "manifest targets {arch}, but this client runs {}",
                std::env::consts::ARCH
            ));
        }
        None => return Err("manifest is missing the target architecture".to_string()),
    }
    Ok(())
}

fn read_cuda_addon_version_file(dir: &Path) -> Option<String> {
    let content = std::fs::read_to_string(dir.join("version.txt")).ok()?;
    let trimmed = content.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub fn inspect_cuda_addon_directory(dir: &Path) -> CudaAddonInspection {
    let path_str = dir.to_string_lossy().to_string();

    if !dir.is_dir() {
        let missing_files =
            required_cuda_addon_files(std::env::consts::OS, DEFAULT_CUDA_MAJOR_VERSION);
        return CudaAddonInspection {
            is_installed: false,
            is_active: false,
            is_compatible: false,
            path: path_str,
            missing_files,
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
    let (parsed_manifest, validation_error) = if !manifest_path.is_file() {
        (None, Some("CUDA addon manifest is missing".to_string()))
    } else {
        match std::fs::read_to_string(&manifest_path)
            .map_err(|error| format!("CUDA addon manifest is unreadable: {error}"))
            .and_then(|content| {
                serde_json::from_str::<CudaAddonManifest>(&content)
                    .map_err(|error| format!("CUDA addon manifest is invalid: {error}"))
            }) {
            Ok(manifest) => {
                let validation_error = validate_cuda_addon_manifest(&manifest).err();
                (Some(manifest), validation_error)
            }
            Err(error) => (None, Some(error)),
        }
    };

    // Metadata is reported even when the manifest fails validation, so the UI can show
    // which version is installed and why it is unusable.
    let manifest = parsed_manifest.as_ref();
    let version = manifest
        .map(|value| value.addon_version.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| read_cuda_addon_version_file(dir));
    let cuda_version = manifest
        .map(|value| value.cuda_version.trim().to_string())
        .filter(|value| !value.is_empty());
    let llama_cpp_ref = manifest.and_then(|value| value.llama_cpp_ref.clone());
    let sherpa_onnx_version = manifest.and_then(|value| value.sherpa_onnx_version.clone());

    let required_files = required_cuda_addon_files(
        std::env::consts::OS,
        cuda_version
            .as_deref()
            .unwrap_or(DEFAULT_CUDA_MAJOR_VERSION),
    );
    let missing_files: Vec<String> = required_files
        .iter()
        .filter(|file_name| !dir.join(file_name).is_file())
        .cloned()
        .collect();
    let is_installed =
        validation_error.is_none() && !required_files.is_empty() && missing_files.is_empty();
    let is_compatible = is_installed
        && llama_cpp_ref.as_deref() == Some(EXPECTED_CUDA_ADDON_LLAMA_CPP_REF)
        && sherpa_onnx_version.as_deref() == Some(EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION);

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
        version,
        cuda_version,
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

    fn write_required_files(dir: &Path, cuda_version: &str) -> Vec<String> {
        let required = required_cuda_addon_files(std::env::consts::OS, cuda_version);
        for file in &required {
            std::fs::write(dir.join(file), b"test").unwrap();
        }
        required
    }

    fn valid_manifest(cuda_version: &str) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": 1,
            "addonVersion": "0.1.0",
            "cudaVersion": cuda_version,
            "llamaCppRef": EXPECTED_CUDA_ADDON_LLAMA_CPP_REF,
            "sherpaOnnxVersion": EXPECTED_CUDA_ADDON_SHERPA_ONNX_VERSION,
            "targetOs": std::env::consts::OS,
            "targetArch": std::env::consts::ARCH,
        })
    }

    fn write_manifest(dir: &Path, manifest: &serde_json::Value) {
        std::fs::write(dir.join(CUDA_ADDON_MANIFEST_FILENAME), manifest.to_string()).unwrap();
    }

    #[test]
    fn required_files_vary_by_platform() {
        assert_eq!(required_cuda_addon_files("windows", "12.4").len(), 5);
        assert_eq!(required_cuda_addon_files("linux", "12.4").len(), 5);
        assert_eq!(required_cuda_addon_files("macos", "12.4").len(), 0);
    }

    #[test]
    fn required_files_follow_cuda_major_version() {
        let windows_13 = required_cuda_addon_files("windows", "13.0");
        assert!(windows_13.contains(&"cudart64_13.dll".to_string()));
        assert!(windows_13.contains(&"cublasLt64_13.dll".to_string()));

        let linux_12 = required_cuda_addon_files("linux", "12.4.1");
        assert!(linux_12.contains(&"libcudart.so.12".to_string()));
        assert!(linux_12.contains(&"libcublasLt.so.12".to_string()));

        assert_eq!(
            required_cuda_addon_files("windows", "12.4"),
            required_cuda_addon_files("windows", "")
        );
    }

    #[test]
    fn required_files_use_cuda_11_runtime_names() {
        let windows_11 = required_cuda_addon_files("windows", "11.7");
        assert!(windows_11.contains(&"cudart64_110.dll".to_string()));
        assert!(windows_11.contains(&"cublas64_11.dll".to_string()));

        let linux_11 = required_cuda_addon_files("linux", "11.7");
        assert!(linux_11.contains(&"libcudart.so.11.0".to_string()));
        assert!(linux_11.contains(&"libcublasLt.so.11".to_string()));
    }

    #[test]
    fn inspect_empty_or_missing_directory_reports_not_installed() {
        let temp = test_temp_dir();
        let inspection = inspect_cuda_addon_directory(temp.path());

        assert!(!inspection.is_installed);
        assert!(!inspection.is_compatible);
        assert_eq!(
            inspection.missing_files.len(),
            required_cuda_addon_files(std::env::consts::OS, DEFAULT_CUDA_MAJOR_VERSION).len()
        );
    }

    #[test]
    fn inspect_complete_directory_detects_version_and_installation() {
        let temp = test_temp_dir();
        let required = write_required_files(temp.path(), "12.4");
        write_manifest(temp.path(), &valid_manifest("12.4"));

        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(inspection.validation_error.is_none());
        assert_eq!(inspection.version.as_deref(), Some("0.1.0"));
        assert_eq!(inspection.cuda_version.as_deref(), Some("12.4"));
        assert!(inspection.missing_files.is_empty());
        if !required.is_empty() {
            assert!(inspection.is_installed);
            assert!(inspection.is_compatible);
        }
    }

    #[test]
    fn cuda_11_addon_set_is_installed_and_compatible() {
        if required_cuda_addon_files(std::env::consts::OS, "11.7").is_empty() {
            return;
        }
        let temp = test_temp_dir();
        write_required_files(temp.path(), "11.7");
        write_manifest(temp.path(), &valid_manifest("11.7"));

        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(inspection.missing_files.is_empty(), "{inspection:?}");
        assert!(inspection.is_installed);
        assert!(inspection.is_compatible);
    }

    #[test]
    fn invalid_manifest_is_not_compatible() {
        let temp = test_temp_dir();
        write_required_files(temp.path(), "12.4");
        std::fs::write(temp.path().join(CUDA_ADDON_MANIFEST_FILENAME), b"{").unwrap();
        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(!inspection.is_installed);
        assert!(!inspection.is_compatible);
        assert!(
            inspection
                .validation_error
                .is_some_and(|error| error.contains("invalid"))
        );
    }

    #[test]
    fn dependency_mismatch_is_installed_but_not_compatible() {
        if required_cuda_addon_files(std::env::consts::OS, "12.4").is_empty() {
            return;
        }
        let temp = test_temp_dir();
        write_required_files(temp.path(), "12.4");
        let mut manifest = valid_manifest("12.4");
        manifest["schemaVersion"] = serde_json::json!("1.0");
        manifest["addonVersion"] = serde_json::json!("0.2.0");
        manifest["llamaCppRef"] = serde_json::json!("old-ref");
        write_manifest(temp.path(), &manifest);

        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(inspection.is_installed);
        assert!(!inspection.is_compatible);
        assert_eq!(inspection.version.as_deref(), Some("0.2.0"));
    }

    #[test]
    fn legacy_manifest_keeps_version_but_is_not_installed() {
        let temp = test_temp_dir();
        write_required_files(temp.path(), "12.4");
        let mut manifest = valid_manifest("12.4");
        manifest["addonVersion"] = serde_json::json!("0.2.0");
        manifest.as_object_mut().unwrap().remove("llamaCppRef");
        write_manifest(temp.path(), &manifest);

        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(!inspection.is_installed);
        assert!(!inspection.is_compatible);
        assert_eq!(inspection.version.as_deref(), Some("0.2.0"));
        assert_eq!(inspection.cuda_version.as_deref(), Some("12.4"));
        assert!(
            inspection
                .validation_error
                .is_some_and(|error| error.contains("llama.cpp"))
        );
    }

    #[test]
    fn version_file_is_reported_when_manifest_is_missing() {
        let temp = test_temp_dir();
        std::fs::write(temp.path().join("version.txt"), b"0.3.0\n").unwrap();

        let inspection = inspect_cuda_addon_directory(temp.path());
        assert!(!inspection.is_installed);
        assert_eq!(inspection.version.as_deref(), Some("0.3.0"));
        assert!(
            inspection
                .validation_error
                .is_some_and(|error| error.contains("missing"))
        );
    }
}
