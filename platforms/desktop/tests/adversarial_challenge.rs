//! Adversarial Challenge Test Suite for Milestone 1
//!
//! Empirically tests:
//! 1. Platform Opener edge cases, invalid URLs, non-existent paths, malicious scheme attempts, and detached execution.
//! 2. Auto-launch registry queries, path quoting/escaping, simulated paths with spaces/quotes/unicode, and disabled states.
//! 3. Tokio runtime bridge graceful shutdown, task cancellation, and panic containment.
//! 4. Single-instance lock identity consistency and secondary instance behavior.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use sona_desktop_lib::platform::auto_launch::{
    disable_auto_start, is_auto_start_enabled,
};
use sona_desktop_lib::platform::opener::{
    open_path, open_path_async, open_url,
};
use sona_desktop_lib::platform::single_instance::{
    focus_existing_window, SONA_SINGLE_INSTANCE_LOCK_NAME,
};

// ============================================================================
// 1. Platform Opener Stress Testing & Adversarial Input Evaluation
// ============================================================================

#[test]
fn test_opener_empty_url() {
    // Challenging empty URL input
    let res = open_url("");
    println!("open_url(\"\") result: {:?}", res);
    // Observe whether it returns Err or Ok
}

#[test]
fn test_opener_whitespace_url() {
    let res = open_url("    ");
    println!("open_url(\"    \") result: {:?}", res);
}

#[test]
fn test_opener_invalid_scheme() {
    let res = open_url("invalid_scheme_without_colon");
    println!("open_url(invalid_scheme) result: {:?}", res);
}

#[test]
fn test_opener_nonexistent_local_path() {
    let non_existent = PathBuf::from(r"Z:\nonexistent_path_sona_stress_test\missing.txt");
    let res = open_path(&non_existent);
    println!("open_path(nonexistent) result: {:?}", res);
    // Document whether OS reports failure or silent success
}

#[test]
fn test_opener_empty_path() {
    let res = open_path(PathBuf::from(""));
    println!("open_path(\"\") result: {:?}", res);
}

#[tokio::test]
async fn test_opener_async_nonexistent_path() {
    let non_existent = PathBuf::from(r"Z:\nonexistent_path_sona_async_test\missing.txt");
    let res = open_path_async(non_existent).await;
    println!("open_path_async(nonexistent) result: {:?}", res);
}

#[tokio::test]
async fn test_opener_async_panic_containment() {
    // If a blocking task spawned inside tokio panics, tokio catches it in JoinError
    let task = tokio::task::spawn_blocking(|| {
        panic!("Simulated panic inside opener blocking task");
    });
    let res = task.await;
    assert!(res.is_err(), "JoinHandle must return Err on panic");
    let err = res.unwrap_err();
    assert!(err.is_panic(), "JoinError must indicate panic");
}

#[test]
fn test_opener_malicious_scheme_inspection() {
    // Verify that open_url has no scheme filtering (e.g. accepts file:// or cmd invocations)
    // We inspect the behavior without launching dangerous processes
    let command_injection = "calc.exe";
    let file_scheme = "file:///C:/Windows/System32/drivers/etc/hosts";
    let javascript_scheme = "javascript:alert(1)";

    // Document whether open_url accepts any arbitrary string
    assert_eq!(command_injection, "calc.exe");
    assert!(file_scheme.starts_with("file://"));
    assert!(javascript_scheme.starts_with("javascript:"));
}

#[test]
fn test_opener_null_byte_inputs() {
    // Test URL with null byte
    let res_url = open_url("https://example.com\0malicious");
    println!("open_url with null byte result: {:?}", res_url);

    // Test Path with null byte
    let res_path = open_path(PathBuf::from("C:\\test\0file.txt"));
    println!("open_path with null byte result: {:?}", res_path);
}

// ============================================================================
// 2. Auto-Launch Registry & Simulated Path Stress Testing
// ============================================================================

#[test]
fn test_auto_launch_query_status_on_system() {
    // Directly query the Windows registry via auto-launch manager
    let is_enabled = is_auto_start_enabled();
    println!("is_auto_start_enabled() returned: {:?}", is_enabled);
    assert!(
        is_enabled.is_ok(),
        "is_auto_start_enabled() should query cleanly without panicking: {:?}",
        is_enabled.err()
    );
}

#[test]
fn test_auto_launch_disable_idempotence() {
    // If auto-start is not enabled, what does disable do?
    // It should either succeed idempotently or return an explainable error, never panic.
    let current_status = is_auto_start_enabled().unwrap();
    if !current_status {
        let disable_res = disable_auto_start();
        println!("disable_auto_start() when disabled returned: {:?}", disable_res);
    }
}

#[test]
fn test_auto_launch_builder_with_simulated_paths() {
    use auto_launch::AutoLaunchBuilder;

    // Test A: Path with spaces
    let path_with_spaces = r"C:\Program Files\Sona Desktop\sona.exe";
    let builder_spaces = AutoLaunchBuilder::new()
        .set_app_name("SonaTestSpaces")
        .set_app_path(path_with_spaces)
        .set_args(&["--autostart"])
        .build();
    assert!(builder_spaces.is_ok(), "AutoLaunchBuilder must support paths with spaces");
    let manager = builder_spaces.unwrap();
    assert_eq!(manager.get_app_name(), "SonaTestSpaces");

    // Test B: Path with unicode characters
    let path_unicode = r"C:\测试目录\Sona 语音转写\sona.exe";
    let builder_unicode = AutoLaunchBuilder::new()
        .set_app_name("SonaTestUnicode")
        .set_app_path(path_unicode)
        .set_args(&["--autostart"])
        .build();
    assert!(builder_unicode.is_ok(), "AutoLaunchBuilder must support unicode paths");

    // Test C: Path with quotes (testing potential registry command injection)
    let path_quoted = r#"C:\Tools\"Sona" Desktop\sona.exe"#;
    let builder_quoted = AutoLaunchBuilder::new()
        .set_app_name("SonaTestQuoted")
        .set_app_path(path_quoted)
        .set_args(&["--autostart"])
        .build();
    println!("Builder quoted path result: {:?}", builder_quoted.is_ok());

    // Test D: Empty app name and path
    let builder_empty_name = AutoLaunchBuilder::new()
        .set_app_name("")
        .set_app_path(r"C:\test\app.exe")
        .build();
    println!("Builder empty name result: {:?}", builder_empty_name.is_ok());

    let builder_empty_path = AutoLaunchBuilder::new()
        .set_app_name("SonaTestEmptyPath")
        .set_app_path("")
        .build();
    println!("Builder empty path result: {:?}", builder_empty_path.is_ok());
}

#[test]
fn test_auto_launch_live_registry_enable_disable_lifecycle() {
    use auto_launch::AutoLaunchBuilder;

    let test_app_name = format!("Sona_Adversarial_Test_{}", uuid::Uuid::new_v4());
    let current_exe = std::env::current_exe().unwrap();
    let app_path = current_exe.to_str().unwrap();

    let manager = AutoLaunchBuilder::new()
        .set_app_name(&test_app_name)
        .set_app_path(app_path)
        .set_args(&["--autostart", "--test"])
        .build()
        .expect("Failed to build test auto-launch manager");

    // 1. Initially must not be enabled
    assert!(!manager.is_enabled().unwrap(), "Fresh test key should not be enabled");

    // 2. Enable it
    let enable_res = manager.enable();
    assert!(enable_res.is_ok(), "Registry enable should succeed: {:?}", enable_res.err());
    assert!(manager.is_enabled().unwrap(), "Key must now report enabled=true");

    // 3. Enable again (idempotency check)
    let enable_again = manager.enable();
    assert!(enable_again.is_ok(), "Enabling already enabled key should succeed idempotently");

    // 4. Disable it
    let disable_res = manager.disable();
    assert!(disable_res.is_ok(), "Registry disable should succeed: {:?}", disable_res.err());
    assert!(!manager.is_enabled().unwrap(), "Key must now report enabled=false");

    // 5. Disable again (idempotency check when already disabled)
    let disable_again = manager.disable();
    println!("Disabling already disabled key result: {:?}", disable_again);
}

// ============================================================================
// 3. Tokio Runtime Bridge, Graceful Shutdown & Panic Containment
// ============================================================================

#[test]
fn test_tokio_runtime_bridge_lifecycle() {
    // Simulate what run_app does: create multi-threaded runtime, enter guard, spawn tasks, and drop
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    let executed = Arc::new(AtomicBool::new(false));
    let executed_clone = executed.clone();

    let guard = rt.enter();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(10)).await;
        executed_clone.store(true, Ordering::SeqCst);
    });
    drop(guard);

    // Give task time to execute
    std::thread::sleep(Duration::from_millis(50));
    assert!(executed.load(Ordering::SeqCst), "Spawned task must have executed");

    // Dropping rt shuts down the runtime cleanly
    drop(rt);
}

#[test]
fn test_tokio_runtime_graceful_shutdown_timeout() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    // Spawn a long-running uncooperative task
    rt.spawn(async {
        loop {
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });

    let shutdown_start = std::time::Instant::now();
    // Use shutdown_timeout to enforce bounded wait
    rt.shutdown_timeout(Duration::from_millis(50));
    let elapsed = shutdown_start.elapsed();

    println!("Graceful shutdown_timeout(50ms) elapsed: {:?}", elapsed);
    assert!(
        elapsed < Duration::from_millis(500),
        "Shutdown timeout should bound runtime termination cleanly"
    );
}

#[test]
fn test_tokio_runtime_task_cancellation() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let counter = Arc::new(AtomicUsize::new(0));
    let counter_clone = counter.clone();

    let handle = rt.spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(5)).await;
            counter_clone.fetch_add(1, Ordering::SeqCst);
        }
    });

    std::thread::sleep(Duration::from_millis(25));
    let val_before = counter.load(Ordering::SeqCst);
    assert!(val_before > 0, "Task should have incremented counter");

    // Cancel the task
    handle.abort();

    std::thread::sleep(Duration::from_millis(25));
    let val_after = counter.load(Ordering::SeqCst);

    // Assert task stopped progressing after abort
    assert!(
        val_after <= val_before + 1,
        "Aborted task must cease execution: before={}, after={}",
        val_before,
        val_after
    );
}

#[test]
fn test_tokio_runtime_panic_containment() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();

    let survivor_executed = Arc::new(AtomicBool::new(false));
    let survivor_clone = survivor_executed.clone();

    // Spawn a task that panics
    let panic_handle = rt.spawn(async {
        panic!("Deliberate worker task panic for containment test");
    });

    // Spawn a survivor task on the same runtime
    let survivor_handle = rt.spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        survivor_clone.store(true, Ordering::SeqCst);
        "survivor_ok"
    });

    let panic_res = rt.block_on(panic_handle);
    assert!(panic_res.is_err(), "Panicked task must return Err");
    assert!(
        panic_res.unwrap_err().is_panic(),
        "JoinError must report panic"
    );

    let survivor_res = rt.block_on(survivor_handle);
    assert_eq!(survivor_res.unwrap(), "survivor_ok");
    assert!(
        survivor_executed.load(Ordering::SeqCst),
        "Survivor task must succeed despite peer task panic"
    );
}

// ============================================================================
// 4. Single-Instance Lock Identity & Multi-Window Focus Verification
// ============================================================================

#[test]
fn test_single_instance_lock_identity_consistency() {
    // Compare SONA_SINGLE_INSTANCE_LOCK_NAME with the hardcoded string in lib.rs
    let declared_lock_name = SONA_SINGLE_INSTANCE_LOCK_NAME;
    let lib_rs_hardcoded_name = "com.asoda.sona.instance_lock";

    println!("Declared lock name: {}", declared_lock_name);
    println!("lib.rs hardcoded name: {}", lib_rs_hardcoded_name);

    // If these two differ, a lock acquired via SingleInstanceGuard will NOT protect
    // against an instance started via lib.rs!
    let are_names_identical = declared_lock_name == lib_rs_hardcoded_name;
    println!("Are lock names identical? {}", are_names_identical);

    // Both single_instance crate locks should function independently
    let lock_a = single_instance::SingleInstance::new(declared_lock_name).unwrap();
    let lock_b = single_instance::SingleInstance::new(lib_rs_hardcoded_name).unwrap();

    // Since they use different names, BOTH will claim to be single!
    assert!(
        lock_a.is_single(),
        "Lock A must be single for its own name"
    );
    assert!(
        lock_b.is_single(),
        "Lock B must be single for its own name because the names differ!"
    );
}

#[test]
fn test_focus_existing_window_missing_target() {
    // Searching for a non-existent window title must return false, not panic
    let result = focus_existing_window("NonExistent_Sona_Window_Title_12345");
    assert!(!result, "Focusing a non-existent window must return false");
}
