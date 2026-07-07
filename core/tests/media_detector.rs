use std::io::Write;

#[test]
fn media_magic_bytes_detect_audio_video_and_reject_plain_text() {
    assert!(sona_core::media_detector::is_valid_media_bytes(
        b"#!AMR\n\x00\x00"
    ));
    assert!(sona_core::media_detector::is_valid_media_bytes(&[
        0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE,
        0x6C,
    ]));
    assert!(!sona_core::media_detector::is_valid_media_bytes(
        b"hello world"
    ));
}

#[tokio::test]
async fn media_file_detection_reads_magic_bytes_from_disk() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = dir.path().join("sample.amr");
    let text_path = dir.path().join("sample.txt");

    std::fs::File::create(&audio_path)
        .unwrap()
        .write_all(b"#!AMR\nrest")
        .unwrap();
    std::fs::File::create(&text_path)
        .unwrap()
        .write_all(b"not media")
        .unwrap();

    assert!(sona_core::media_detector::is_valid_media_file(&audio_path).await);
    assert!(!sona_core::media_detector::is_valid_media_file(&text_path).await);
}
