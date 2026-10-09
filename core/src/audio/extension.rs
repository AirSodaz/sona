//! Standard audio extension and format detection utilities.
//!
//! Provides pure functions to deduce canonical audio file extensions
//! from MIME types, container/codec magic bytes, and file paths.

/// Infers canonical audio extension from a MIME type string.
pub fn infer_audio_extension_from_mime(mime: &str) -> Option<&'static str> {
    let mime_lower = mime.to_ascii_lowercase();
    let trimmed = mime_lower.trim();
    if trimmed.contains("wav") || trimmed.contains("wave") {
        Some("wav")
    } else if trimmed.contains("webm") {
        Some("webm")
    } else if trimmed.contains("mp4") || trimmed.contains("m4a") {
        Some("m4a")
    } else if trimmed.contains("ogg") {
        Some("ogg")
    } else if trimmed.contains("flac") {
        Some("flac")
    } else if trimmed.contains("mp3") || trimmed.contains("mpeg") {
        Some("mp3")
    } else if trimmed.contains("aac") {
        Some("aac")
    } else if trimmed.contains("opus") {
        Some("opus")
    } else {
        None
    }
}

/// Infers canonical audio extension by inspecting initial file header magic bytes.
pub fn infer_audio_extension_from_magic(bytes: &[u8]) -> Option<&'static str> {
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WAVE" {
        Some("wav")
    } else if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xF6) == 0xF0 {
        // AAC ADTS: 12-bit sync word 0xFFF, layer is always 0b00 (e.g. 0xFFF0, 0xFFF1, 0xFFF8, 0xFFF9)
        Some("aac")
    } else if bytes.starts_with(b"ID3")
        || (bytes.len() >= 2
            && bytes[0] == 0xFF
            && (bytes[1] & 0xE0) == 0xE0
            && (bytes[1] & 0x06) != 0x00
            && (bytes[1] & 0x18) != 0x08)
    {
        // MPEG Audio: 11-bit sync word 0xFFE, layer != 0b00 (Layer III is 0b01), version != 0b01
        Some("mp3")
    } else if bytes.starts_with(b"OggS") {
        Some("ogg")
    } else if bytes.starts_with(b"fLaC") {
        Some("flac")
    } else if bytes.len() >= 8 && &bytes[4..8] == b"ftyp" {
        Some("m4a")
    } else if bytes.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        Some("webm")
    } else {
        None
    }
}

/// Extracts normalized audio extension from a file path or returns `fallback`.
pub fn infer_audio_extension_from_path(path: &str, fallback: &str) -> String {
    let file_name = path.rsplit(['/', '\\']).next().unwrap_or(path).trim();

    if let Some(dot_idx) = file_name.rfind('.') {
        let ext = file_name[dot_idx + 1..].trim();
        if !ext.is_empty() {
            return ext.to_ascii_lowercase();
        }
    }

    fallback.to_string()
}

/// Best-effort canonical audio extension inference combining magic bytes, MIME, and path.
pub fn infer_audio_extension(
    path: Option<&str>,
    mime: Option<&str>,
    magic_bytes: Option<&[u8]>,
    fallback: &str,
) -> String {
    if let Some(ext) = magic_bytes.and_then(infer_audio_extension_from_magic) {
        return ext.to_string();
    }

    if let Some(ext) = mime.and_then(infer_audio_extension_from_mime) {
        return ext.to_string();
    }

    if let Some(p) = path {
        let ext = infer_audio_extension_from_path(p, "");
        if !ext.is_empty() {
            return ext;
        }
    }

    fallback.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_infer_from_mime() {
        assert_eq!(infer_audio_extension_from_mime("audio/wav"), Some("wav"));
        assert_eq!(infer_audio_extension_from_mime("audio/x-wav"), Some("wav"));
        assert_eq!(
            infer_audio_extension_from_mime("audio/webm;codecs=opus"),
            Some("webm")
        );
        assert_eq!(infer_audio_extension_from_mime("audio/mp4"), Some("m4a"));
        assert_eq!(infer_audio_extension_from_mime("audio/aac"), Some("aac"));
        assert_eq!(infer_audio_extension_from_mime("audio/ogg"), Some("ogg"));
        assert_eq!(infer_audio_extension_from_mime("audio/flac"), Some("flac"));
        assert_eq!(infer_audio_extension_from_mime("audio/mpeg"), Some("mp3"));
        assert_eq!(
            infer_audio_extension_from_mime("application/octet-stream"),
            None
        );
    }

    #[test]
    fn test_infer_from_magic() {
        assert_eq!(
            infer_audio_extension_from_magic(b"RIFF\x24\x00\x00\x00WAVEfmt "),
            Some("wav")
        );
        assert_eq!(
            infer_audio_extension_from_magic(b"ID3\x04\x00\x00\x00"),
            Some("mp3")
        );
        assert_eq!(
            infer_audio_extension_from_magic(&[0xFF, 0xFB, 0x90, 0x64]),
            Some("mp3")
        );
        assert_eq!(
            infer_audio_extension_from_magic(&[0xFF, 0xF1, 0x50, 0x80]),
            Some("aac")
        );
        assert_eq!(
            infer_audio_extension_from_magic(&[0xFF, 0xF9, 0x50, 0x80]),
            Some("aac")
        );
        assert_eq!(
            infer_audio_extension_from_magic(b"OggS\x00\x02\x00\x00"),
            Some("ogg")
        );
        assert_eq!(
            infer_audio_extension_from_magic(b"fLaC\x00\x00\x00\x22"),
            Some("flac")
        );
        assert_eq!(
            infer_audio_extension_from_magic(&[
                0x00, 0x00, 0x00, 0x20, b'f', b't', b'y', b'p', b'M', b'4', b'A', b' '
            ]),
            Some("m4a")
        );
        assert_eq!(
            infer_audio_extension_from_magic(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x86, 0x81]),
            Some("webm")
        );
        assert_eq!(infer_audio_extension_from_magic(b"random bytes"), None);
    }

    #[test]
    fn test_infer_from_path() {
        assert_eq!(
            infer_audio_extension_from_path("audio.wav", "fallback"),
            "wav"
        );
        assert_eq!(
            infer_audio_extension_from_path("/path/to/record.M4A", "fallback"),
            "m4a"
        );
        assert_eq!(
            infer_audio_extension_from_path("C:\\Users\\AppData\\sound.ogg", "fallback"),
            "ogg"
        );
        assert_eq!(
            infer_audio_extension_from_path("no_ext", "fallback"),
            "fallback"
        );
        assert_eq!(
            infer_audio_extension_from_path("trailing_dot.", "fallback"),
            "fallback"
        );
    }

    #[test]
    fn test_infer_combined() {
        assert_eq!(
            infer_audio_extension(
                Some("file.mp3"),
                Some("audio/wav"),
                Some(b"RIFF\x24\x00\x00\x00WAVEfmt "),
                "wav"
            ),
            "wav"
        );
        assert_eq!(
            infer_audio_extension(None, Some("audio/webm"), None, "wav"),
            "webm"
        );
        assert_eq!(
            infer_audio_extension(Some("recording.aac"), None, None, "wav"),
            "aac"
        );
        // ADTS magic should yield "aac" and match/reinforce audio/aac MIME instead of being misclassified as mp3
        assert_eq!(
            infer_audio_extension(
                Some("recording.unknown"),
                Some("audio/aac"),
                Some(&[0xFF, 0xF1, 0x50, 0x80]),
                "wav"
            ),
            "aac"
        );
        assert_eq!(
            infer_audio_extension(
                None,
                Some("audio/aac"),
                Some(&[0xFF, 0xF9, 0x50, 0x80]),
                "wav"
            ),
            "aac"
        );
        // MPEG frame sync should yield "mp3"
        assert_eq!(
            infer_audio_extension(
                None,
                Some("audio/mpeg"),
                Some(&[0xFF, 0xFB, 0x90, 0x64]),
                "wav"
            ),
            "mp3"
        );
        assert_eq!(
            infer_audio_extension(None, None, None, "default_ext"),
            "default_ext"
        );
    }
}
