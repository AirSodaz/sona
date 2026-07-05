use sona_core::paths::select_desktop_models_dir_from_app_roots;

#[test]
fn selects_existing_models_dir_from_candidate_roots() {
    let dir = tempfile::tempdir().unwrap();
    let first_root = dir.path().join("Sona");
    let preferred_root = dir.path().join("com.asoda.sona");
    std::fs::create_dir_all(preferred_root.join("models")).unwrap();

    let result = select_desktop_models_dir_from_app_roots([first_root, preferred_root.clone()]);

    assert_eq!(result, Some(preferred_root.join("models")));
}

#[test]
fn falls_back_to_first_candidate_models_dir_when_none_exist() {
    let dir = tempfile::tempdir().unwrap();
    let first_root = dir.path().join("com.asoda.sona");
    let second_root = dir.path().join("Sona");

    let result = select_desktop_models_dir_from_app_roots([first_root.clone(), second_root]);

    assert_eq!(result, Some(first_root.join("models")));
}

#[test]
fn returns_none_when_no_candidate_roots_are_available() {
    let result = select_desktop_models_dir_from_app_roots(Vec::new());

    assert_eq!(result, None);
}
