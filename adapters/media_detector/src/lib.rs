use std::path::Path;

use tokio::fs::File;
use tokio::io::AsyncReadExt;

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

pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    let mut results = Vec::with_capacity(paths.len());
    for path in paths {
        results.push(is_valid_media_file(&path).await);
    }
    Ok(results)
}
