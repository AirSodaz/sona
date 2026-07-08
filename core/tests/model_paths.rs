use std::path::PathBuf;

use sona_core::models::paths::{ModelsDirStatus, resolve_models_dir, status_of};

#[test]
fn resolve_models_dir_accepts_explicit_directory() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let resolved = resolve_models_dir(Some(models_dir.clone()), None, |_| {
        ModelsDirStatus::Directory
    })
    .unwrap();

    assert_eq!(resolved, models_dir);
}

#[test]
fn resolve_models_dir_rejects_existing_file() {
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("not_a_dir.txt");

    let error =
        resolve_models_dir(Some(file_path), None, |_| ModelsDirStatus::NotDirectory).unwrap_err();

    assert!(error.contains("exists but is not a directory"));
}

#[test]
fn resolve_models_dir_allows_explicit_missing_directory_for_later_creation() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let resolved = resolve_models_dir(Some(PathBuf::from(&models_dir)), None, |_| {
        ModelsDirStatus::Missing
    })
    .unwrap();

    assert_eq!(resolved, models_dir);
}

#[test]
fn resolve_models_dir_uses_adapter_supplied_default() {
    let dir = tempfile::tempdir().unwrap();
    let models_dir = dir.path().join("models");

    let resolved =
        resolve_models_dir(None, Some(models_dir.clone()), |_| ModelsDirStatus::Missing).unwrap();

    assert_eq!(resolved, models_dir);
}

#[test]
fn status_of_maps_adapter_probe_results() {
    assert_eq!(status_of(false, false), ModelsDirStatus::Missing);
    assert_eq!(status_of(true, true), ModelsDirStatus::Directory);
    assert_eq!(status_of(true, false), ModelsDirStatus::NotDirectory);
}
