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

    // Read up to 8192 bytes (enough for most magic numbers, including ID3 tags padding)
    let mut buf = [0; 8192];
    let n = match file.read(&mut buf).await {
        Ok(n) => n,
        Err(_) => return false,
    };

    if n == 0 {
        return false;
    }

    let kind = infer::get(&buf[..n]);
    if let Some(kind) = kind {
        let mime = kind.mime_type();
        if mime.starts_with("audio/")
            || mime.starts_with("video/")
            || mime.starts_with("application/ogg")
        {
            return true;
        }

        // Some specific formats might be classified differently but are valid for us
        // ASF/WMA/WMV
        if mime == "application/vnd.ms-asf" {
            return true;
        }
    }

    // Fallback for formats that might not be in `infer` or have a different mime type

    // AMR: "#!AMR\n"
    if n >= 6 && &buf[..6] == b"#!AMR\n" {
        return true;
    }

    // WMA/ASF (Advanced Systems Format) GUID: 30 26 B2 75 8E 66 CF 11 A6 D9 00 AA 00 62 CE 6C
    let asf_guid: [u8; 16] = [
        0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE,
        0x6C,
    ];
    if n >= 16 && buf[..16] == asf_guid {
        return true;
    }

    false
}

#[tauri::command]
pub async fn check_media_formats(paths: Vec<String>) -> Result<Vec<bool>, String> {
    let mut results = Vec::with_capacity(paths.len());
    for path in paths {
        let is_valid = is_valid_media_file(&path).await;
        results.push(is_valid);
    }
    Ok(results)
}
