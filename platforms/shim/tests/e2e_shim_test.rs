use sona_shim::dispatcher::{detect_role, resolve_target_channel, resolve_target_executable};
use sona_shim::installer::{install_shims, uninstall_shims};
use sona_shim::registry::{
    Channel, TargetRole, get_default_channel, get_registered_channel, register_channel,
    set_default_channel, unregister_channel,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static REG_LOCK: Mutex<()> = Mutex::new(());

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("sona-shim-test-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
struct EnvGuard {
    vars: Vec<(&'static str, Option<String>)>,
}

impl EnvGuard {
    fn new(vars: &[(&'static str, &str)]) -> Self {
        let mut tracked = Vec::new();
        unsafe {
            for (k, v) in vars {
                let prev = std::env::var(k).ok();
                std::env::set_var(k, v);
                tracked.push((*k, prev));
            }
        }
        Self { vars: tracked }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        unsafe {
            for (k, prev) in &self.vars {
                if let Some(val) = prev {
                    std::env::set_var(k, val);
                } else {
                    std::env::remove_var(k);
                }
            }
        }
    }
}
struct RegistryGuard {
    subkey: String,
}

impl RegistryGuard {
    fn new(name: &str) -> Self {
        let subkey = format!(r"Software\SonaTest_{}_{}", name, std::process::id());
        Self { subkey }
    }
}

impl Drop for RegistryGuard {
    fn drop(&mut self) {
        #[cfg(windows)]
        {
            let _ = sona_shim::registry::win_delete_registry_key(&format!(
                r"{}\Channels\stable",
                self.subkey
            ));
            let _ = sona_shim::registry::win_delete_registry_key(&format!(
                r"{}\Channels\nightly",
                self.subkey
            ));
            let _ =
                sona_shim::registry::win_delete_registry_key(&format!(r"{}\Channels", self.subkey));
            let _ =
                sona_shim::registry::win_delete_registry_key(&format!(r"{}\Settings", self.subkey));
            let _ = sona_shim::registry::win_delete_registry_key(&self.subkey);
        }
    }
}

#[test]
fn test_role_detection_all_variants() {
    assert_eq!(detect_role("sona-cli"), TargetRole::Cli);
    assert_eq!(detect_role("sona-cli.exe"), TargetRole::Cli);
    assert_eq!(detect_role("sona-cli-stable.exe"), TargetRole::Cli);
    assert_eq!(detect_role("sona-cli-nightly.exe"), TargetRole::Cli);
    assert_eq!(detect_role("sona.exe"), TargetRole::Cli);
    assert_eq!(detect_role("sona"), TargetRole::Cli);

    assert_eq!(detect_role("sona-mcp"), TargetRole::Mcp);
    assert_eq!(detect_role("sona-mcp.exe"), TargetRole::Mcp);
    assert_eq!(detect_role("sona-mcp-stable.exe"), TargetRole::Mcp);
    assert_eq!(detect_role("sona-mcp-nightly.exe"), TargetRole::Mcp);

    assert_eq!(detect_role("sona-shim"), TargetRole::Shim);
    assert_eq!(detect_role("sona-shim.exe"), TargetRole::Shim);
}

#[test]
fn test_channel_resolution_logic() {
    let cwd = Path::new(".");

    // Suffix rules
    let mut args = vec!["arg1".to_string()];
    assert_eq!(
        resolve_target_channel("sona-cli-nightly", &mut args, cwd, TargetRole::Cli),
        Channel::Nightly
    );
    assert_eq!(
        resolve_target_channel("sona-mcp-nightly.exe", &mut args, cwd, TargetRole::Mcp),
        Channel::Nightly
    );
    assert_eq!(
        resolve_target_channel("sona-cli-stable.exe", &mut args, cwd, TargetRole::Cli),
        Channel::Stable
    );

    // +flag rules
    let mut args_plus_nightly = vec![
        "+nightly".to_string(),
        "models".to_string(),
        "list".to_string(),
    ];
    assert_eq!(
        resolve_target_channel("sona-cli", &mut args_plus_nightly, cwd, TargetRole::Cli),
        Channel::Nightly
    );
    assert_eq!(
        args_plus_nightly,
        vec!["models".to_string(), "list".to_string()]
    );

    let mut args_plus_stable = vec!["+stable".to_string(), "transcribe".to_string()];
    assert_eq!(
        resolve_target_channel("sona-cli.exe", &mut args_plus_stable, cwd, TargetRole::Cli),
        Channel::Stable
    );
    assert_eq!(args_plus_stable, vec!["transcribe".to_string()]);
}

#[test]
fn test_dual_channel_registration_and_discovery() {
    let _guard = REG_LOCK.lock().unwrap();
    let reg_guard = RegistryGuard::new("dual");
    let reg_dir = TempDir::new("dual-reg");
    let reg_dir_str = reg_dir.path.to_string_lossy().to_string();
    let _env = EnvGuard::new(&[
        ("SONA_DIR", &reg_dir_str),
        ("SONA_REGISTRY_SUBKEY", &reg_guard.subkey),
        ("SONA_TEST_ISOLATED", "1"),
    ]);
    let stable_dir = TempDir::new("stable-install");
    let nightly_dir = TempDir::new("nightly-install");
    let exe_name = if cfg!(windows) {
        "sona-cli.exe"
    } else {
        "sona-cli"
    };
    let mcp_name = if cfg!(windows) {
        "sona-mcp.exe"
    } else {
        "sona-mcp"
    };

    fs::write(stable_dir.path.join(exe_name), "stable-cli").unwrap();
    fs::write(stable_dir.path.join(mcp_name), "stable-mcp").unwrap();
    fs::write(nightly_dir.path.join(exe_name), "nightly-cli").unwrap();
    fs::write(nightly_dir.path.join(mcp_name), "nightly-mcp").unwrap();

    // Register both channels
    register_channel(
        Channel::Stable,
        &stable_dir.path,
        Some("0.9.7"),
        Some(exe_name),
    )
    .unwrap();
    register_channel(
        Channel::Nightly,
        &nightly_dir.path,
        Some("0.9.7-45"),
        Some(exe_name),
    )
    .unwrap();

    let stable_reg = get_registered_channel(Channel::Stable).expect("Stable should be registered");
    assert_eq!(stable_reg.install_path, stable_dir.path);
    assert_eq!(stable_reg.version.as_deref(), Some("0.9.7"));

    let nightly_reg =
        get_registered_channel(Channel::Nightly).expect("Nightly should be registered");
    assert_eq!(nightly_reg.install_path, nightly_dir.path);
    assert_eq!(nightly_reg.version.as_deref(), Some("0.9.7-45"));

    // Verify targeted executable resolution
    let resolved_stable_cli = resolve_target_executable(TargetRole::Cli, Channel::Stable);
    assert_eq!(resolved_stable_cli, Some(stable_dir.path.join(exe_name)));

    let resolved_nightly_cli = resolve_target_executable(TargetRole::Cli, Channel::Nightly);
    assert_eq!(resolved_nightly_cli, Some(nightly_dir.path.join(exe_name)));

    let resolved_stable_mcp = resolve_target_executable(TargetRole::Mcp, Channel::Stable);
    assert_eq!(resolved_stable_mcp, Some(stable_dir.path.join(mcp_name)));

    let resolved_nightly_mcp = resolve_target_executable(TargetRole::Mcp, Channel::Nightly);
    assert_eq!(resolved_nightly_mcp, Some(nightly_dir.path.join(mcp_name)));

    // Clean up registry via unregister
    let _ = unregister_channel(Channel::Stable);
    let _ = unregister_channel(Channel::Nightly);
}

#[test]
fn test_reference_counting_uninstallation() {
    let _guard = REG_LOCK.lock().unwrap();
    let reg_guard = RegistryGuard::new("rc");
    let reg_dir = TempDir::new("rc-reg");
    let reg_dir_str = reg_dir.path.to_string_lossy().to_string();
    let _env = EnvGuard::new(&[
        ("SONA_DIR", &reg_dir_str),
        ("SONA_REGISTRY_SUBKEY", &reg_guard.subkey),
        ("SONA_TEST_ISOLATED", "1"),
    ]);
    let stable_dir = TempDir::new("rc-stable");
    let nightly_dir = TempDir::new("rc-nightly");
    let mock_shim_dir = TempDir::new("rc-mock-shim");
    let mock_shim = mock_shim_dir.path.join(if cfg!(windows) {
        "sona-shim.exe"
    } else {
        "sona-shim"
    });
    fs::write(&mock_shim, b"mock-shim-binary").unwrap();
    install_shims(&mock_shim, None).unwrap();
    let exe_name = if cfg!(windows) {
        "sona-cli.exe"
    } else {
        "sona-cli"
    };
    fs::write(stable_dir.path.join(exe_name), "stable-cli").unwrap();
    fs::write(nightly_dir.path.join(exe_name), "nightly-cli").unwrap();

    register_channel(
        Channel::Stable,
        &stable_dir.path,
        Some("0.9.7"),
        Some(exe_name),
    )
    .unwrap();
    register_channel(
        Channel::Nightly,
        &nightly_dir.path,
        Some("0.9.7-45"),
        Some(exe_name),
    )
    .unwrap();
    set_default_channel(Channel::Stable).unwrap();

    // Uninstall Stable only (with force=false)
    let report = uninstall_shims(Some(Channel::Stable), false).unwrap();
    assert_eq!(report.removed_channel, Some("stable".to_string()));
    assert_eq!(report.active_remaining_channel, Some("nightly".to_string()));
    assert!(!report.shims_removed);
    assert!(!report.path_removed);

    // Default channel should have automatically smoothly switched to Nightly
    assert_eq!(get_default_channel(), Channel::Nightly);

    // Now uninstall Nightly
    let report2 = uninstall_shims(Some(Channel::Nightly), false).unwrap();
    assert_eq!(report2.removed_channel, Some("nightly".to_string()));
    assert_eq!(report2.active_remaining_channel, None);
    assert!(report2.shims_removed);
    assert!(report2.path_removed);

    #[cfg(windows)]
    {
        assert!(
            !sona_shim::registry::win_registry_key_exists(&reg_guard.subkey),
            "Registry subkey {} should have been deleted after uninstalling the last channel",
            reg_guard.subkey
        );
    }
    // Clean up
    let _ = unregister_channel(Channel::Stable);
    let _ = unregister_channel(Channel::Nightly);
}

#[test]
fn test_cli_flags_validation() {
    let shim_bin = env!("CARGO_BIN_EXE_sona-shim");

    // test list --help
    let output = std::process::Command::new(shim_bin)
        .args(["list", "--help"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: sona-shim list [--json]"));

    // test install --channel stable without --path fails
    let output = std::process::Command::new(shim_bin)
        .args(["install", "--channel", "stable"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Option '--channel' requires '--path <PATH>'"));
}
