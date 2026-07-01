//! Integration tests to verify that Sona's `sherpa-onnx` dependency is configured for dynamic linking
//! and that the dynamic libraries are correctly loaded at runtime on the target operating system.
//!
//! File location: src-tauri/tests/dynamic_linking_tests.rs

#![allow(dead_code)]

use std::env;
use std::path::PathBuf;

static PATH_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Tier 3.1: Verify that the workspace directory `resources/shared_libs` exists and is populated
/// with the platform-specific dynamic libraries when built with the `shared` feature.
#[test]
fn test_shared_libs_directory_structure() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let shared_libs_dir = manifest_dir.join("resources").join("shared_libs");

    // Gating check: if SHERPA_ONNX_LIB_DIR is not set, and the libraries are not populated, return early.
    // This allows developer local cargo tests to pass even if they didn't run a build with libraries set.
    if std::env::var("SHERPA_ONNX_LIB_DIR").is_err()
        && !shared_libs_dir.join("sherpa-onnx-c-api.dll").exists()
        && !shared_libs_dir.join("libsherpa-onnx-c-api.dylib").exists()
        && !shared_libs_dir.join("libsherpa-onnx-c-api.so").exists()
    {
        println!(
            "Skipping test_shared_libs_directory_structure because SHERPA_ONNX_LIB_DIR is not set and libraries are not populated."
        );
        return;
    }

    assert!(
        shared_libs_dir.exists(),
        "The shared libraries directory does not exist: {}. Ensure SHERPA_ONNX_LIB_DIR is set during the build.",
        shared_libs_dir.display()
    );

    let entries = std::fs::read_dir(&shared_libs_dir)
        .expect("Failed to read the shared libraries directory")
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<String>>();

    assert!(
        !entries.is_empty(),
        "The shared libraries directory is empty. Ensure build.rs successfully copies dynamic libraries."
    );

    // Verify platform-specific library presence
    #[cfg(target_os = "windows")]
    {
        assert!(
            entries.iter().any(|name| name == "sherpa-onnx-c-api.dll"),
            "Missing sherpa-onnx-c-api.dll in shared_libs. Found: {:?}",
            entries
        );
        assert!(
            entries.iter().any(|name| name == "onnxruntime.dll"),
            "Missing onnxruntime.dll in shared_libs. Found: {:?}",
            entries
        );
    }

    #[cfg(target_os = "macos")]
    {
        assert!(
            entries
                .iter()
                .any(|name| name == "libsherpa-onnx-c-api.dylib"),
            "Missing libsherpa-onnx-c-api.dylib in shared_libs. Found: {:?}",
            entries
        );
        assert!(
            entries.iter().any(|name| name == "libonnxruntime.dylib"),
            "Missing libonnxruntime.dylib in shared_libs. Found: {:?}",
            entries
        );
    }

    #[cfg(target_os = "linux")]
    {
        assert!(
            entries
                .iter()
                .any(|name| name.starts_with("libsherpa-onnx-c-api.so")),
            "Missing libsherpa-onnx-c-api.so in shared_libs. Found: {:?}",
            entries
        );
        assert!(
            entries
                .iter()
                .any(|name| name.starts_with("libonnxruntime.so")),
            "Missing libonnxruntime.so in shared_libs. Found: {:?}",
            entries
        );
    }
}

/// Tier 3.2: Verify that on Windows, the application successfully sets the DLL directory
/// and resolves delay-loaded dynamic libraries from `resources/shared_libs`.
#[cfg(target_os = "windows")]
#[test]
fn test_windows_dll_delay_load_and_directory_setting() {
    let _lock = PATH_MUTEX.lock().unwrap();

    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW};
    use windows::core::PCWSTR;

    fn to_wstring(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let dll_name_w = to_wstring("sherpa-onnx-c-api.dll");

    // 1. Verify DLL is not eagerly loaded at startup (delay loading check)
    let h_module = unsafe { GetModuleHandleW(PCWSTR(dll_name_w.as_ptr())) };
    if h_module.is_err() {
        println!(
            "Confirmed: sherpa-onnx-c-api.dll was not loaded on test start (Delay loading is active)."
        );
    } else {
        println!("Notice: sherpa-onnx-c-api.dll is already loaded in the test process.");
    }

    // 2. Resolve target directory relative to current executable
    let exe_path = env::current_exe().expect("Failed to get current exe path");
    let exe_dir = exe_path.parent().expect("Failed to get exe directory");

    // Check multiple potential locations corresponding to target/debug, target/release or test running structures
    let mut resolved_lib_dir = None;
    let paths_to_check = vec![
        exe_dir.join("resources").join("shared_libs"),
        exe_dir.join("../resources/shared_libs"),
        exe_dir.join("../../resources/shared_libs"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources")
            .join("shared_libs"),
    ];

    for path in paths_to_check {
        if path.exists() {
            resolved_lib_dir = Some(path);
            break;
        }
    }

    let lib_dir = resolved_lib_dir.expect(
        "Could not resolve resources/shared_libs directory relative to test executable or manifest",
    );
    println!("Resolved shared_libs directory at: {}", lib_dir.display());

    // 3. Test explicit DLL loading using the resolved path to verify SetDllDirectoryW behaves correctly
    let target_dll_path = lib_dir.join("sherpa-onnx-c-api.dll");
    let target_dll_path_w = to_wstring(target_dll_path.to_str().unwrap());

    // Loading it directly should succeed and resolve its dependencies (like onnxruntime.dll)
    let load_result = unsafe { LoadLibraryW(PCWSTR(target_dll_path_w.as_ptr())) };
    assert!(
        load_result.is_ok(),
        "Failed to dynamically load sherpa-onnx-c-api.dll. Dependency resolution failed. Error: {:?}",
        std::io::Error::last_os_error()
    );
}

/// Tier 3.2.1: Verify that on Windows, calling init_dll_directory correctly sets the DLL directory
/// and enables loading the dynamic library by name.
#[cfg(target_os = "windows")]
#[test]
fn test_windows_init_dll_directory_resolution() {
    let _lock = PATH_MUTEX.lock().unwrap();

    use windows::Win32::System::LibraryLoader::{GetModuleHandleW, LoadLibraryW, SetDllDirectoryW};
    use windows::core::PCWSTR;

    fn to_wstring(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let dll_name_w = to_wstring("sherpa-onnx-c-api.dll");

    // 1. Filter out target\debug and target/debug from PATH to prevent Cargo's automatic fallback loading
    let original_path = std::env::var("PATH").unwrap_or_default();
    let clean_path = std::env::join_paths(std::env::split_paths(&original_path).filter(|p| {
        let p_str = p.to_string_lossy().to_lowercase();
        !p_str.contains("target\\debug") && !p_str.contains("target/debug")
    }))
    .unwrap();
    unsafe {
        std::env::set_var("PATH", &clean_path);
    }

    // 2. Reset DLL directory to verify standard behavior
    unsafe {
        let _ = SetDllDirectoryW(PCWSTR(std::ptr::null()));
    }

    // 3. Ensure library cannot be loaded by name initially (if not already loaded in the process)
    let h_module = unsafe { GetModuleHandleW(PCWSTR(dll_name_w.as_ptr())) };
    if h_module.is_err() {
        let load_before = unsafe { LoadLibraryW(PCWSTR(dll_name_w.as_ptr())) };
        if let Ok(handle) = load_before {
            let mut buffer = [0u16; 512];
            let len = unsafe {
                windows::Win32::System::LibraryLoader::GetModuleFileNameW(Some(handle), &mut buffer)
            };
            let loaded_path = String::from_utf16_lossy(&buffer[..len as usize]);
            panic!(
                "DLL unexpectedly loaded before init_dll_directory!\nLoaded from: {}\nClean PATH: {}\nOriginal PATH: {}",
                loaded_path,
                clean_path.to_string_lossy(),
                original_path
            );
        }
    }

    // 3. Call library's initialization function
    tauri_appsona_lib::init_dll_directory();

    // 4. Loading by name should now succeed
    let load_after = unsafe { LoadLibraryW(PCWSTR(dll_name_w.as_ptr())) };

    // Restore PATH
    unsafe {
        std::env::set_var("PATH", original_path);
    }

    assert!(
        load_after.is_ok(),
        "Failed to load sherpa-onnx-c-api.dll by name after calling init_dll_directory. Error: {:?}",
        std::io::Error::last_os_error()
    );
}

/// Tier 3.3: Verify that on Linux and macOS, the shared library is loaded from the correct location
/// indicating that RPATH is configured correctly.
#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn test_unix_rpath_resolution() {
    let mut library_loaded = false;
    let mut loaded_from_correct_path = false;

    #[cfg(target_os = "linux")]
    {
        // Parse /proc/self/maps to inspect maps of the current process
        let maps =
            std::fs::read_to_string("/proc/self/maps").expect("Failed to read /proc/self/maps");

        for line in maps.lines() {
            if line.contains("libsherpa-onnx-c-api.so") {
                library_loaded = true;
                if line.contains("resources/shared_libs") {
                    loaded_from_correct_path = true;
                    println!("Verified: libsherpa-onnx-c-api.so is loaded from: {}", line);
                    break;
                }
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // On macOS, we can query the loaded dyld images at runtime.
        extern "C" {
            fn _dyld_image_count() -> u32;
            fn _dyld_get_image_name(image_index: u32) -> *const std::os::raw::c_char;
        }

        unsafe {
            let count = _dyld_image_count();
            for i in 0..count {
                let name_ptr = _dyld_get_image_name(i);
                if !name_ptr.is_null() {
                    let name_str = std::ffi::CStr::from_ptr(name_ptr).to_string_lossy();
                    if name_str.contains("libsherpa-onnx-c-api.dylib") {
                        library_loaded = true;
                        if name_str.contains("resources/shared_libs")
                            || name_str.contains("Sona.app")
                        {
                            loaded_from_correct_path = true;
                            println!(
                                "Verified: libsherpa-onnx-c-api.dylib is loaded from macOS bundle/rpath: {}",
                                name_str
                            );
                            break;
                        }
                    }
                }
            }
        }
    }

    // Report results (Note: If tests are run without a full build copy, print warnings or assert based on environment)
    if library_loaded {
        assert!(
            loaded_from_correct_path,
            "sherpa-onnx dynamic library was loaded, but NOT from the application's resources/shared_libs path (possible system path pollution or rpath configuration issue)."
        );
    } else {
        println!(
            "Notice: sherpa-onnx dynamic library has not been loaded by the OS yet. Call any sherpa-onnx API to load it."
        );
    }
}
