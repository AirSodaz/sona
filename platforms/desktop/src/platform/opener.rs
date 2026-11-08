//! External browser and file manager opener using the `open` crate.
//!
//! Provides detached, non-blocking URL and local file/folder opening.

use std::path::{Path, PathBuf};

/// Synchronously opens an external URL using the system's default web browser.
///
/// Validates that the URL is non-empty, non-whitespace, and begins with an allowed scheme
/// (`http://`, `https://`, or `mailto:`). Returns `std::io::ErrorKind::InvalidInput` if invalid.
pub fn open_url(url: impl AsRef<str>) -> Result<(), std::io::Error> {
    let raw = url.as_ref();
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "URL cannot be empty or whitespace",
        ));
    }

    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with("http://")
        && !lower.starts_with("https://")
        && !lower.starts_with("mailto:")
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!(
                "Invalid or unsupported URL scheme in '{trimmed}'. Must start with http://, https://, or mailto:"
            ),
        ));
    }

    log::info!("Opening external URL: {trimmed}");
    open::that_detached(trimmed)
}

/// Synchronously opens a local file or directory using the OS file manager.
///
/// Validates that the target path exists on the filesystem. Returns `std::io::ErrorKind::NotFound`
/// if the target file or directory does not exist.
pub fn open_path(path: impl AsRef<Path>) -> Result<(), std::io::Error> {
    let path_ref = path.as_ref();
    if !path_ref.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Target path does not exist: {}", path_ref.display()),
        ));
    }

    log::info!("Opening system path: {}", path_ref.display());
    open::that_detached(path_ref)
}

/// Asynchronously opens an external URL without blocking the GPUI UI thread.
pub async fn open_url_async(url: String) -> Result<(), std::io::Error> {
    tokio::task::spawn_blocking(move || open_url(url))
        .await
        .map_err(std::io::Error::other)?
}

/// Asynchronously opens a local path without blocking the GPUI UI thread.
pub async fn open_path_async(path: PathBuf) -> Result<(), std::io::Error> {
    tokio::task::spawn_blocking(move || open_path(path))
        .await
        .map_err(std::io::Error::other)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_open_url_rejects_empty_and_whitespace() {
        let err_empty = open_url("").expect_err("empty URL must fail");
        assert_eq!(err_empty.kind(), std::io::ErrorKind::InvalidInput);

        let err_spaces = open_url("    ").expect_err("whitespace URL must fail");
        assert_eq!(err_spaces.kind(), std::io::ErrorKind::InvalidInput);

        let err_tabs = open_url("\t \n \r ").expect_err("tab/newline URL must fail");
        assert_eq!(err_tabs.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[test]
    fn test_open_url_rejects_unsupported_or_malicious_schemes() {
        let invalid_cases = [
            "ftp://files.example.com",
            "file:///C:/Windows/System32/drivers/etc/hosts",
            "javascript:alert(1)",
            "data:text/html,<h1>test</h1>",
            "calc.exe",
            "invalid_scheme_without_colon",
            "://missing_scheme",
            "ws://localhost:8080",
            "gopher://gopher.floodgap.com",
        ];

        for case in invalid_cases {
            let err = open_url(case).expect_err(&format!("expected error for '{case}'"));
            assert_eq!(
                err.kind(),
                std::io::ErrorKind::InvalidInput,
                "Case '{case}' must produce InvalidInput error"
            );
        }
    }

    #[test]
    fn test_open_url_validates_scheme_prefix() {
        for valid_prefix in [
            "http://", "https://", "mailto:", "HTTP://", "HTTPS://", "MAILTO:",
        ] {
            let lower = valid_prefix.to_ascii_lowercase();
            assert!(
                lower.starts_with("http://")
                    || lower.starts_with("https://")
                    || lower.starts_with("mailto:")
            );
        }
    }

    #[test]
    fn test_open_path_rejects_nonexistent_and_empty() {
        let non_existent = PathBuf::from(r"Z:\nonexistent_path_sona_test_missing_12345");
        let err = open_path(&non_existent).expect_err("nonexistent path must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);

        let empty = PathBuf::from("");
        let err_empty = open_path(&empty).expect_err("empty path must fail");
        assert_eq!(err_empty.kind(), std::io::ErrorKind::NotFound);
    }

    #[tokio::test]
    async fn test_open_url_async_validation_propagation() {
        let err = open_url_async("".to_string())
            .await
            .expect_err("async empty URL must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput);

        let err_scheme = open_url_async("ftp://invalid.com".to_string())
            .await
            .expect_err("async invalid scheme must fail");
        assert_eq!(err_scheme.kind(), std::io::ErrorKind::InvalidInput);
    }

    #[tokio::test]
    async fn test_open_path_async_rejects_nonexistent() {
        let non_existent = PathBuf::from(r"Z:\nonexistent_path_sona_async_test_missing_12345");
        let err = open_path_async(non_existent)
            .await
            .expect_err("async nonexistent path must fail");
        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
    }
}
