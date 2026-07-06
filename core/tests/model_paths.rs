use std::fs;
use std::path::PathBuf;

use sona_core::model_paths::resolve_models_dir;

#[test]
fn resolve_models_dir_accepts_explicit_directory() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");
    fs::create_dir_all(&models_dir).unwrap();

    let resolved = resolve_models_dir(Some(models_dir.clone())).unwrap();

    assert_eq!(resolved, models_dir);
}

#[test]
fn resolve_models_dir_rejects_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir.txt");
    fs::write(&file_path, "dummy").unwrap();

    let error = resolve_models_dir(Some(file_path)).unwrap_err();

    assert!(error.contains("exists but is not a directory"));
}

#[test]
fn resolve_models_dir_allows_explicit_missing_directory_for_later_creation() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let resolved = resolve_models_dir(Some(PathBuf::from(&models_dir))).unwrap();

    assert_eq!(resolved, models_dir);
}
