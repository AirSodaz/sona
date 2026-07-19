use std::fs;

use sona_archive::ArchiveOperation;

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
    let missing_source = temp.path().join("missing");
    let archive_path = temp.path().join("archive.tar.bz2");

    let error = sona_archive::create_tar_bz2(
        missing_source.to_str().unwrap(),
        archive_path.to_str().unwrap(),
    )
    .unwrap_err();

    assert_eq!(error.operation, ArchiveOperation::InspectSource);
    assert_eq!(error.source, missing_source);
    assert_eq!(error.target.as_deref(), Some(archive_path.as_path()));
    assert!(error.reason.contains("Source directory does not exist"));
}

#[test]
fn extraction_errors_preserve_archive_and_target_paths() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("missing.tar.bz2");
    let target_dir = temp.path().join("extract");

    let error = sona_archive::extract_tar_bz2(
        archive_path.to_str().unwrap(),
        target_dir.to_str().unwrap(),
        |_| {},
    )
    .unwrap_err();

    assert_eq!(error.operation, ArchiveOperation::OpenArchive);
    assert_eq!(error.source, archive_path);
    assert_eq!(error.target.as_deref(), Some(target_dir.as_path()));
}
