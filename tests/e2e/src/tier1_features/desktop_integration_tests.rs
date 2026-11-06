//! Tier 1: Desktop Integration Tests (Features 6, 7, and Native Helpers)

#[cfg(test)]
mod tests {
    use crate::harness::{AutoLaunchConfig, SingleInstanceLock};

    #[test]
    fn test_single_instance_lock_acquisition_and_duplicate_detection() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_id = format!("sona_test_instance_{}", uuid::Uuid::new_v4());

        let instance_a = SingleInstanceLock::try_acquire(&test_id, temp_dir.path())
            .expect("Lock acquisition failed");
        assert_eq!(instance_a.identifier, test_id);
        assert!(instance_a.is_single());

        let instance_b = SingleInstanceLock::try_acquire(&test_id, temp_dir.path())
            .expect("Lock attempt should return ok with is_single=false");
        assert!(!instance_b.is_single());

        drop(instance_a);

        let instance_c = SingleInstanceLock::try_acquire(&test_id, temp_dir.path())
            .expect("Subsequent lock acquisition failed");
        assert!(instance_c.is_single());
    }

    #[test]
    fn test_single_instance_lock_distinct_identifiers_do_not_conflict() {
        let temp_dir = tempfile::tempdir().unwrap();
        let id_a = format!("sona_test_a_{}", uuid::Uuid::new_v4());
        let id_b = format!("sona_test_b_{}", uuid::Uuid::new_v4());

        let lock_a = SingleInstanceLock::try_acquire(&id_a, temp_dir.path()).unwrap();
        let lock_b = SingleInstanceLock::try_acquire(&id_b, temp_dir.path()).unwrap();

        assert!(lock_a.is_single());
        assert!(lock_b.is_single());
    }

    #[test]
    fn test_single_instance_rapid_acquire_release_cycles() {
        let temp_dir = tempfile::tempdir().unwrap();
        let test_id = format!("sona_rapid_test_{}", uuid::Uuid::new_v4());

        for _ in 0..10 {
            let lock = SingleInstanceLock::try_acquire(&test_id, temp_dir.path()).unwrap();
            assert!(lock.is_single());
            drop(lock);
        }
    }

    #[test]
    fn test_auto_launch_windows_registry_command_with_spaces() {
        let config = AutoLaunchConfig::new(
            "Sona",
            r"C:\Program Files\Sona Desktop\sona.exe",
            vec!["--autostart".to_string(), "--silent".to_string()],
        );

        let cmd = config.windows_registry_command();
        assert_eq!(
            cmd,
            r#""C:\Program Files\Sona Desktop\sona.exe" --autostart --silent"#
        );
        assert_eq!(
            AutoLaunchConfig::WINDOWS_RUN_KEY,
            r"Software\Microsoft\Windows\CurrentVersion\Run"
        );
    }

    #[test]
    fn test_auto_launch_windows_registry_command_without_spaces() {
        let config = AutoLaunchConfig::new(
            "Sona",
            r"C:\Tools\Sona\sona.exe",
            vec!["--autostart".to_string()],
        );

        let cmd = config.windows_registry_command();
        assert_eq!(cmd, r"C:\Tools\Sona\sona.exe --autostart");
    }

    #[test]
    fn test_auto_launch_windows_command_parsing_roundtrip() {
        let original_path = r"C:\Program Files\Sona\sona.exe";
        let original_args = vec!["--autostart".to_string(), "--minimized".to_string()];

        let config = AutoLaunchConfig::new("Sona", original_path, original_args.clone());
        let cmd = config.windows_registry_command();

        let (parsed_path, parsed_args) =
            AutoLaunchConfig::parse_windows_command(&cmd).expect("Command parsing failed");

        assert_eq!(parsed_path.to_str().unwrap(), original_path);
        assert_eq!(parsed_args, original_args);
    }

    #[test]
    fn test_auto_launch_linux_desktop_entry_generation() {
        let config = AutoLaunchConfig::new(
            "sona",
            "/usr/bin/sona",
            vec!["--autostart".to_string()],
        );

        let entry = config.linux_desktop_entry();
        assert!(entry.contains("[Desktop Entry]"));
        assert!(entry.contains("Type=Application"));
        assert!(entry.contains("Name=sona"));
        assert!(entry.contains("Exec=/usr/bin/sona --autostart"));
        assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
    }

    #[test]
    fn test_auto_launch_macos_plist_generation() {
        let config = AutoLaunchConfig::new(
            "Sona",
            "/Applications/Sona.app/Contents/MacOS/sona",
            vec!["--autostart".to_string()],
        );

        let plist = config.macos_launch_agent_plist("com.sona.app");
        assert!(plist.contains("<key>Label</key>"));
        assert!(plist.contains("<string>com.sona.app</string>"));
        assert!(plist.contains("<string>/Applications/Sona.app/Contents/MacOS/sona</string>"));
        assert!(plist.contains("<string>--autostart</string>"));
        assert!(plist.contains("<key>RunAtLoad</key>"));
        assert!(plist.contains("<true/>"));
    }

    #[test]
    fn test_auto_launch_parse_command_errors() {
        assert!(AutoLaunchConfig::parse_windows_command("").is_err());
        assert!(AutoLaunchConfig::parse_windows_command("   ").is_err());
        assert!(AutoLaunchConfig::parse_windows_command("\"unclosed quote").is_err());
    }
}
