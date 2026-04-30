use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

use super::backup::build_backup_manifest;
use super::fs_utils::{create_tar_bz2_archive, write_json_pretty_atomic};
use super::types::{HistoryDraftSource, HistoryItemKind, HistoryItemRecord, HistoryItemStatus};
use super::{
    ANALYTICS_DIR_NAME, ANALYTICS_USAGE_FILE_NAME, AUTOMATION_DIR_NAME,
    AUTOMATION_PROCESSED_FILE_NAME, AUTOMATION_RULES_FILE_NAME, CONFIG_DIR_NAME, CONFIG_FILE_NAME,
    HISTORY_DIR_NAME, PROJECTS_DIR_NAME, PROJECTS_INDEX_FILE_NAME,
};

pub(super) fn sample_history_item(id: &str, status: HistoryItemStatus) -> HistoryItemRecord {
    HistoryItemRecord {
        id: id.to_string(),
        timestamp: 1,
        duration: 2.0,
        audio_path: format!("{id}.wav"),
        transcript_path: format!("{id}.json"),
        title: format!("Item {id}"),
        preview_text: String::new(),
        icon: None,
        kind: HistoryItemKind::Recording,
        search_content: String::new(),
        project_id: None,
        status,
        draft_source: if status == HistoryItemStatus::Draft {
            Some(HistoryDraftSource::LiveRecord)
        } else {
            None
        },
    }
}

pub(super) fn create_valid_backup_archive(archive_path: &Path) -> PathBuf {
    let staging_dir = tempdir().unwrap();
    let history_dir = staging_dir.path().join(HISTORY_DIR_NAME);
    let config_dir = staging_dir.path().join(CONFIG_DIR_NAME);
    let projects_dir = staging_dir.path().join(PROJECTS_DIR_NAME);
    let automation_dir = staging_dir.path().join(AUTOMATION_DIR_NAME);
    let analytics_dir = staging_dir.path().join(ANALYTICS_DIR_NAME);

    fs::create_dir_all(&history_dir).unwrap();
    fs::create_dir_all(&config_dir).unwrap();
    fs::create_dir_all(&projects_dir).unwrap();
    fs::create_dir_all(&automation_dir).unwrap();
    fs::create_dir_all(&analytics_dir).unwrap();

    let item = sample_history_item("history-1", HistoryItemStatus::Complete);
    write_json_pretty_atomic(&history_dir.join(PROJECTS_INDEX_FILE_NAME), &vec![item]).unwrap();
    write_json_pretty_atomic(
        &history_dir.join("history-1.json"),
        &json!([{ "id": "seg-1", "text": "hello", "start": 0, "end": 1, "isFinal": true }]),
    )
    .unwrap();
    write_json_pretty_atomic(
        &history_dir.join("history-1.summary.json"),
        &json!({ "activeTemplateId": "general" }),
    )
    .unwrap();
    write_json_pretty_atomic(
        &staging_dir.path().join("manifest.json"),
        &build_backup_manifest("0.6.4".to_string(), 1, 1, 1, 1, 1, 1),
    )
    .unwrap();
    write_json_pretty_atomic(
        &config_dir.join(CONFIG_FILE_NAME),
        &json!({ "theme": "auto" }),
    )
    .unwrap();
    write_json_pretty_atomic(
        &projects_dir.join(PROJECTS_INDEX_FILE_NAME),
        &vec![json!({ "id": "project-1", "name": "Workspace", "defaults": {} })],
    )
    .unwrap();
    write_json_pretty_atomic(
        &automation_dir.join(AUTOMATION_RULES_FILE_NAME),
        &vec![json!({ "id": "rule-1" })],
    )
    .unwrap();
    write_json_pretty_atomic(
        &automation_dir.join(AUTOMATION_PROCESSED_FILE_NAME),
        &vec![json!({ "ruleId": "rule-1", "filePath": "C:\\watch\\file.wav" })],
    )
    .unwrap();
    fs::write(
        analytics_dir.join(ANALYTICS_USAGE_FILE_NAME),
        r#"{"schemaVersion":1}"#,
    )
    .unwrap();

    create_tar_bz2_archive(staging_dir.path(), archive_path).unwrap();
    archive_path.to_path_buf()
}
