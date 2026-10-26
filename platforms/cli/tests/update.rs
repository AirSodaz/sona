use axum::routing::get;
use axum::{Json, Router};
use sona_core::runtime::app_update::{AppPlatformAsset, AppUpdateManifest, current_platform_key};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;

const TEST_PUBKEY: &str = "RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3";
const TEST_SIGNATURE: &str = "untrusted comment: signature from minisign secret key\nRUQf6LRCGA9i559r3g7V1qNyJDApGip8MfqcadIgT9CuhV3EMhHoN1mGTkUidF/z7SrlQgXdy8ofjb7bNJJylDOocrCo8KLzZwo=\ntrusted comment: timestamp:1633700835\tfile:test\tprehashed\nwLMDjy9FLAuxZ3q4NlEvkgtyhrr0gtTu6KC4KBJdITbbOeAi1zBIYo0v4iTgt8jJpIidRJnp94ABQkJAgAooBQ==";

#[test]
fn update_help_succeeds_and_displays_options() {
    let output = sona_cli::run_cli_from_args(["sona-cli", "update", "--help"])
        .expect("update help should succeed");
    assert_eq!(output.stderr, "");
    let help = output.stdout;
    assert!(help.contains("--check"));
    assert!(help.contains("--force"));
    assert!(help.contains("--channel"));
    assert!(help.contains("--gui"));
    assert!(help.contains("defaults to detected client channel"));
    assert!(help.contains("--download-only"));
    assert!(help.contains("--output"));
    assert!(help.contains("--mirror"));
    assert!(help.contains("-y"));
    assert!(help.contains("--quiet"));
    assert!(help.contains("--json"));
}

#[tokio::test]
async fn update_check_detects_available_update() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: Some("Exciting features".to_string()),
        pub_date: Some("2026-04-10T12:00:00Z".to_string()),
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--check",
        "--endpoint",
        &endpoint,
        "--current-version-override",
        "0.9.6",
    ])
    .await
    .unwrap();

    assert!(output.stdout.contains("An update is available!"));
    assert!(output.stdout.contains("Latest version:  99.0.0"));
    assert!(output.stdout.contains("Exciting features"));
}

#[tokio::test]
async fn update_check_json_conforms_to_schema() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: Some("Notes".to_string()),
        pub_date: Some("2026-04-10T12:00:00Z".to_string()),
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--check",
        "--json",
        "--endpoint",
        &endpoint,
        "--current-version-override",
        "0.9.6",
    ])
    .await
    .unwrap();

    let val: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(val["update_available"], true);
    assert_eq!(val["current_version"], "0.9.6");
    assert_eq!(val["latest_version"], "99.0.0");
    assert_eq!(val["channel"], "stable");
    assert_eq!(val["notes"], "Notes");
}

#[tokio::test]
async fn update_check_reports_up_to_date() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "0.1.0".to_string(),
        notes: None,
        pub_date: None,
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--check",
        "--endpoint",
        &endpoint,
        "--current-version-override",
        "0.9.6",
    ])
    .await
    .unwrap();

    assert!(output.stdout.contains("Sona is already up to date"));
}

#[tokio::test]
async fn update_download_only_verifies_signature() {
    let temp = tempfile::tempdir().unwrap();
    let target_file = temp.path().join("downloaded-asset.bin");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let asset_url = format!("http://{addr}/asset.bin");
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: asset_url,
            signature: TEST_SIGNATURE.to_string(),
        },
    );

    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: None,
        pub_date: None,
        platforms,
    };

    let app = Router::new()
        .route("/updater.json", get(|| async move { Json(manifest) }))
        .route("/asset.bin", get(|| async move { "test" }));

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--download-only",
        "-o",
        target_file.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--pubkey",
        TEST_PUBKEY,
        "--current-version-override",
        "0.9.6",
        "--quiet",
    ])
    .await
    .unwrap();

    assert!(
        output
            .stdout
            .contains("Successfully downloaded and verified")
    );
    assert!(target_file.is_file());
    assert_eq!(std::fs::read(&target_file).unwrap(), b"test");
}

#[tokio::test]
async fn update_signature_mismatch_fails_closed() {
    let temp = tempfile::tempdir().unwrap();
    let target_file = temp.path().join("downloaded-asset.bin");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let asset_url = format!("http://{addr}/asset.bin");
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: asset_url,
            signature: TEST_SIGNATURE.to_string(),
        },
    );

    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: None,
        pub_date: None,
        platforms,
    };

    // Return corrupted bytes instead of "test"
    let app = Router::new()
        .route("/updater.json", get(|| async move { Json(manifest) }))
        .route("/asset.bin", get(|| async move { "corrupted-content" }));

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let endpoint = format!("http://{addr}/updater.json");

    let res = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--download-only",
        "-o",
        target_file.to_str().unwrap(),
        "--endpoint",
        &endpoint,
        "--pubkey",
        TEST_PUBKEY,
        "--current-version-override",
        "0.9.6",
        "--quiet",
    ])
    .await;

    assert!(res.is_err(), "Expected signature verification failure");
    assert!(
        !target_file.exists(),
        "Target file must not exist when verification fails"
    );
}

#[tokio::test]
async fn update_non_interactive_without_yes_aborts() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: None,
        pub_date: None,
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    // Non-interactive execution without -y
    let res = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--endpoint",
        &endpoint,
        "--current-version-override",
        "0.9.6",
    ])
    .await;

    assert!(
        res.is_err(),
        "Expected confirmation required error in non-interactive mode"
    );
    let err = res.err().unwrap();
    assert_eq!(err.exit_code(), 2);
    assert!(err.to_string().contains("confirmation required"));
}

struct DirGuard(PathBuf);
impl Drop for DirGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

#[tokio::test]
async fn update_download_only_without_output_persists_in_cwd() {
    let orig_dir = std::env::current_dir().unwrap();
    let temp = tempfile::tempdir().unwrap();
    std::env::set_current_dir(temp.path()).unwrap();
    let _guard = DirGuard(orig_dir);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();

    let asset_url = format!("http://{addr}/asset.bin");
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: asset_url,
            signature: TEST_SIGNATURE.to_string(),
        },
    );

    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: None,
        pub_date: None,
        platforms,
    };

    let app = Router::new()
        .route("/updater.json", get(|| async move { Json(manifest) }))
        .route("/asset.bin", get(|| async move { "test" }));

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--download-only",
        "--endpoint",
        &endpoint,
        "--pubkey",
        TEST_PUBKEY,
        "--current-version-override",
        "0.9.6",
        "--quiet",
    ])
    .await
    .unwrap();

    assert!(
        output
            .stdout
            .contains("Successfully downloaded and verified")
    );
    let downloaded_file = temp.path().join("asset.bin");
    assert!(
        downloaded_file.is_file(),
        "Downloaded file must persist in current working directory"
    );
    assert_eq!(std::fs::read(&downloaded_file).unwrap(), b"test");
}

#[tokio::test]
async fn update_interactive_decline_exits_cleanly() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: None,
        pub_date: None,
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    let mut io = sona_cli::MemoryCliIo::default();
    io.stdin_lines.push_back("n\n".to_string());

    let res = sona_cli::run_cli_with_io_async(
        [
            "sona-cli",
            "update",
            "--endpoint",
            &endpoint,
            "--current-version-override",
            "0.9.6",
        ],
        &mut io,
    )
    .await;

    assert!(
        res.is_ok(),
        "Interactive decline should exit cleanly (code 0)"
    );
    let output = res.unwrap().unwrap_or_else(|| io.into_output());
    assert!(output.stderr.contains("Update cancelled by user."));
}

#[tokio::test]
async fn update_check_auto_detects_nightly_channel_from_hyphenated_version() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: Some("Nightly build notes".to_string()),
        pub_date: Some("2026-04-10T12:00:00Z".to_string()),
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--check",
        "--json",
        "--endpoint",
        &endpoint,
        "--current-version-override",
        "0.9.6-45",
    ])
    .await
    .unwrap();

    let val: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(val["channel"], "nightly");
    assert_eq!(val["current_version"], "0.9.6-45");
}

#[tokio::test]
async fn update_check_explicit_channel_overrides_hyphenated_version() {
    let mut platforms = HashMap::new();
    platforms.insert(
        current_platform_key().unwrap().to_string(),
        AppPlatformAsset {
            url: "https://example.com/asset.exe".to_string(),
            signature: "sig".to_string(),
        },
    );
    let manifest = AppUpdateManifest {
        version: "99.0.0".to_string(),
        notes: Some("Notes".to_string()),
        pub_date: Some("2026-04-10T12:00:00Z".to_string()),
        platforms,
    };
    let app = Router::new().route("/updater.json", get(|| async move { Json(manifest) }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let endpoint = format!("http://{addr}/updater.json");

    let output = sona_cli::run_cli_from_args_async([
        "sona-cli",
        "update",
        "--check",
        "--json",
        "--channel",
        "stable",
        "--endpoint",
        &endpoint,
        "--current-version-override",
        "0.9.6-45",
    ])
    .await
    .unwrap();

    let val: serde_json::Value = serde_json::from_str(&output.stdout).unwrap();
    assert_eq!(val["channel"], "stable");
    assert_eq!(val["current_version"], "0.9.6-45");
}

#[tokio::test]
async fn update_fails_closed_on_dev_or_ci_builds() {
    // When invoked without --current-version-override in dev or CI builds,
    // update must immediately return an error without touching endpoints.
    let mut io = sona_cli::MemoryCliIo::default();
    let res = sona_cli::run_cli_with_io_async(["sona-cli", "update", "--check"], &mut io).await;

    assert!(
        res.is_err(),
        "update without override on dev/ci build must fail"
    );
    let err_msg = res.unwrap_err().to_string();
    assert!(
        err_msg.contains("Auto-update is disabled for"),
        "unexpected error: {err_msg}"
    );
}
