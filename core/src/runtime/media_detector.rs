pub fn is_valid_media_bytes(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let kind = infer::get(bytes);
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
    if bytes.len() >= 6 && &bytes[..6] == b"#!AMR\n" {
        return true;
    }

    // WMA/ASF (Advanced Systems Format) GUID: 30 26 B2 75 8E 66 CF 11 A6 D9 00 AA 00 62 CE 6C
    let asf_guid: [u8; 16] = [
        0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE,
        0x6C,
    ];
    if bytes.len() >= 16 && bytes[..16] == asf_guid {
        return true;
    }

    false
}
