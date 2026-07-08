use std::fs;

#[test]
fn creates_and_extracts_tar_bz2_archive() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    let nested = source.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::write(source.join("root.txt"), "root").unwrap();
    fs::write(nested.join("child.txt"), "child").unwrap();

    let archive_path = temp.path().join("archive.tar.bz2");
    let extract_dir = temp.path().join("extract");

    sona_archive::create_tar_bz2(source.to_str().unwrap(), archive_path.to_str().unwrap()).unwrap();
    sona_archive::extract_tar_bz2(
        archive_path.to_str().unwrap(),
        extract_dir.to_str().unwrap(),
        |_| {},
    )
    .unwrap();

    assert_eq!(
        fs::read_to_string(extract_dir.join("root.txt")).unwrap(),
        "root"
    );
    assert_eq!(
        fs::read_to_string(extract_dir.join("nested").join("child.txt")).unwrap(),
        "child"
    );
}

#[test]
fn rejects_missing_source_directory() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("archive.tar.bz2");

    let error = sona_archive::create_tar_bz2(
        temp.path().join("missing").to_str().unwrap(),
        archive_path.to_str().unwrap(),
    )
    .unwrap_err();

    assert!(error.contains("Source directory does not exist"));
}
