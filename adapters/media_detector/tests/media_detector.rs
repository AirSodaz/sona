use std::io::Write;

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

    assert!(sona_media_detector::is_valid_media_file(&audio_path).await);
    assert!(!sona_media_detector::is_valid_media_file(&text_path).await);
}

#[tokio::test]
async fn check_media_formats_preserves_path_order() {
    let dir = tempfile::tempdir().unwrap();
    let audio_path = dir.path().join("sample.amr");
    let text_path = dir.path().join("sample.txt");
    let missing_path = dir.path().join("missing.mp3");

    std::fs::File::create(&audio_path)
        .unwrap()
        .write_all(b"#!AMR\nrest")
        .unwrap();
    std::fs::File::create(&text_path)
        .unwrap()
        .write_all(b"not media")
        .unwrap();

    let result = sona_media_detector::check_media_formats(vec![
        audio_path.display().to_string(),
        text_path.display().to_string(),
        missing_path.display().to_string(),
    ])
    .await
    .unwrap();

    assert_eq!(result, vec![true, false, false]);
}
