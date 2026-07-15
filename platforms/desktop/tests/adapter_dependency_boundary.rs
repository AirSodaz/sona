fn desktop_manifest() -> toml::Value {
    toml::from_str(include_str!("../Cargo.toml")).expect("desktop Cargo.toml should parse")
}

fn dependencies<'a>(manifest: &'a toml::Value, section: &str) -> &'a toml::value::Table {
    manifest
        .get(section)
        .and_then(toml::Value::as_table)
        .unwrap_or_else(|| panic!("desktop Cargo.toml should contain [{section}]"))
}

#[test]
fn adapter_owned_implementation_crates_stay_out_of_desktop_runtime_dependencies() {
    let manifest = desktop_manifest();
    let runtime = dependencies(&manifest, "dependencies");

    for dependency in [
        "bytes",
        "fs3",
        "glob",
        "hex",
        "hmac",
        "ipnet",
        "rig-core",
        "rusqlite",
        "sha2",
        "tower-http",
    ] {
        assert!(
            !runtime.contains_key(dependency),
            "{dependency} belongs behind an adapter crate, not in the desktop host"
        );
    }
}

#[test]
fn reqwest_is_available_to_desktop_tests_but_not_the_runtime_host() {
    let manifest = desktop_manifest();
    let runtime = dependencies(&manifest, "dependencies");
    let development = dependencies(&manifest, "dev-dependencies");

    assert!(!runtime.contains_key("reqwest"));
    assert!(development.contains_key("reqwest"));
}
