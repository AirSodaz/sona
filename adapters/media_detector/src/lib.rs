use std::path::Path;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(Clone, Copy, Debug, Default)]
pub struct MagicNumberMediaFileValidator;

#[async_trait::async_trait]
impl sona_core::ports::runtime::MediaFileValidator for MagicNumberMediaFileValidator {
    async fn is_valid_media_file(&self, path: &Path) -> bool {
        crate::is_valid_media_file(path).await
    }
}

/// Checks if a file is a valid audio or video file by reading its magic numbers.
/// Returns true if the file is recognized as audio or video.
pub async fn is_valid_media_file(path: impl AsRef<Path>) -> bool {
    let mut file = match File::open(path).await {
        Ok(f) => f,
        Err(_) => return false,
    };

    // Read up to 8192 bytes (enough for most magic numbers, including ID3 tag padding).
    let mut buf = [0; 8192];
    let n = match file.read(&mut buf).await {
        Ok(n) => n,
        Err(_) => return false,
    };

    if n == 0 {
        return false;
    }

    sona_core::runtime::media_detector::is_valid_media_bytes(&buf[..n])
}

pub async fn check_media_formats(paths: Vec<String>) -> Vec<bool> {
    let mut results = Vec::with_capacity(paths.len());
    for path in paths {
        results.push(is_valid_media_file(&path).await);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::MagicNumberMediaFileValidator;
    use sona_core::ports::runtime::MediaFileValidator;

    #[tokio::test]
    async fn runtime_capability_media_validator_delegates_to_magic_number_detection() {
        let directory = tempfile::tempdir().unwrap();
        let valid = directory.path().join("sample.amr");
        let invalid = directory.path().join("sample.txt");
        std::fs::write(&valid, b"#!AMR\n\x00\x00").unwrap();
        std::fs::write(&invalid, b"plain text").unwrap();
        let validator = MagicNumberMediaFileValidator;

        assert!(validator.is_valid_media_file(&valid).await);
        assert!(!validator.is_valid_media_file(&invalid).await);
    }
}
