use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use bzip2::write::BzEncoder;
use serde_json::{Value, json};
use sona_archive::{
    FsBackupAdapter, FsBackupArchiveRepository, MAX_BACKUP_ENTRIES, MAX_BACKUP_EXPANDED_BYTES,
    MAX_BACKUP_FILE_BYTES,
};
use sona_core::automation::repository::{
    AutomationProcessedRecord, AutomationRepositoryState, AutomationRuleRecord,
    AutomationRuleRecordExportConfig, AutomationRuleRecordStageConfig,
};
use sona_core::backup::{
    BackupApplyResult, BackupArchivePort, BackupDataset, BackupError, BackupExportRequest,
    BackupInspectRequest, BackupManifest, BackupManifestCounts, BackupManifestScopes,
    BackupRestoreDataset, BackupStateRepository,
};
use sona_core::history::{
    HistoryAudioStatus, HistoryBackupSnapshot, HistoryItemKind, HistoryItemRecord,
    HistoryItemStatus,
};
use sona_core::ports::time::UnixMillisClock;
use sona_core::project::{ProjectDefaults, ProjectRecord};
use tar::{EntryType, Header};
use uuid::Uuid;

const MANIFEST_PATH: &str = "manifest.json";

#[derive(Clone)]
struct TestEntry {
    path: String,
    data: Vec<u8>,
    entry_type: EntryType,
}

fn project() -> ProjectRecord {
    ProjectRecord {
        id: "project-1".to_string(),
        name: "Project One".to_string(),
        description: "Test project".to_string(),
        icon: "folder".to_string(),
        created_at: 1,
        updated_at: 2,
        defaults: ProjectDefaults {
            summary_template_id: "general".to_string(),
            translation_language: "en".to_string(),
            polish_preset_id: "general".to_string(),
            polish_scenario: None,
            polish_context: None,
            export_file_name_prefix: "sona".to_string(),
            enabled_text_replacement_set_ids: vec![],
            enabled_hotword_set_ids: vec![],
            enabled_polish_keyword_set_ids: vec![],
            enabled_speaker_profile_ids: vec![],
        },
    }
}

fn history_item() -> HistoryItemRecord {
    HistoryItemRecord {
        id: "history-1".to_string(),
        timestamp: 10,
        duration: 2.5,
        audio_path: "".to_string(),
        audio_status: HistoryAudioStatus::Removed,
        transcript_path: "history/history-1.json".to_string(),
        title: "History One".to_string(),
        preview_text: "hello".to_string(),
        icon: None,
        kind: HistoryItemKind::Recording,
        search_content: "hello".to_string(),
        project_id: Some("project-1".to_string()),
        status: HistoryItemStatus::Complete,
        draft_source: None,
    }
}

fn automation() -> AutomationRepositoryState {
    AutomationRepositoryState {
        rules: vec![AutomationRuleRecord {
            id: "rule-1".to_string(),
            name: "Rule One".to_string(),
            project_id: "project-1".to_string(),
            preset_id: "general".to_string(),
            watch_directory: "C:/watch".to_string(),
            recursive: true,
            enabled: true,
            stage_config: AutomationRuleRecordStageConfig {
                auto_polish: true,
                polish_preset_id: "general".to_string(),
                auto_translate: false,
                translation_language: "en".to_string(),
                export_enabled: true,
            },
            export_config: AutomationRuleRecordExportConfig {
                directory: "C:/exports".to_string(),
                format: "txt".to_string(),
                mode: "original".to_string(),
                prefix: "sona".to_string(),
            },
            created_at: 1,
            updated_at: 2,
        }],
        processed_entries: vec![AutomationProcessedRecord {
            id: "processed-1".to_string(),
            rule_id: "rule-1".to_string(),
            file_path: "C:/watch/input.wav".to_string(),
            source_fingerprint: "fingerprint".to_string(),
            size: 10,
            mtime_ms: 20,
            status: "complete".to_string(),
            processed_at: 30,
            history_id: Some("history-1".to_string()),
            export_path: Some("C:/exports/input.txt".to_string()),
            error_message: None,
        }],
    }
}

fn dataset() -> BackupDataset {
    BackupDataset {
        config: json!({"language": "en"}),
        projects: vec![project()],
        history: HistoryBackupSnapshot {
            items: vec![history_item()],
            transcript_files: vec![("history-1.json".to_string(), json!([]))],
            summary_files: vec![("history-1".to_string(), json!({"text": "summary"}))],
            snapshot_files: vec![
                (
                    "versions/history-1/index.json".to_string(),
                    json!([{
                        "id": "snapshot-1",
                        "historyId": "history-1",
                        "reason": "polish",
                        "createdAt": 40,
                        "segmentCount": 0
                    }]),
                ),
                (
                    "versions/history-1/snapshot-1.json".to_string(),
                    json!({
                        "metadata": {
                            "id": "snapshot-1",
                            "historyId": "history-1",
                            "reason": "polish",
                            "createdAt": 40,
                            "segmentCount": 0
                        },
                        "segments": []
                    }),
                ),
            ],
        },
        automation: automation(),
        analytics_content: "{\"requests\":1}".to_string(),
    }
}

fn manifest() -> BackupManifest {
    BackupManifest {
        schema_version: 1,
        created_at: "2026-07-13T00:00:00.000Z".to_string(),
        app_version: "0.8.0".to_string(),
        history_mode: "light".to_string(),
        scopes: BackupManifestScopes {
            config: true,
            workspace: true,
            history: true,
            automation: true,
            analytics: true,
        },
        counts: BackupManifestCounts {
            projects: 1,
            history_items: 1,
            transcript_files: 1,
            summary_files: 1,
            automation_rules: 1,
            automation_processed_entries: 1,
            analytics_files: 1,
        },
    }
}

fn snapshot_files(history_id: &str) -> Vec<(String, Value)> {
    vec![
        (
            format!("versions/{history_id}/index.json"),
            json!([{
                "id": "snapshot-1",
                "historyId": history_id,
                "reason": "polish",
                "createdAt": 40,
                "segmentCount": 0
            }]),
        ),
        (
            format!("versions/{history_id}/snapshot-1.json"),
            json!({
                "metadata": {
                    "id": "snapshot-1",
                    "historyId": history_id,
                    "reason": "polish",
                    "createdAt": 40,
                    "segmentCount": 0
                },
                "segments": []
            }),
        ),
    ]
}

fn dataset_with_history_id(history_id: &str) -> BackupDataset {
    let mut source = dataset();
    source.history.items[0].id = history_id.to_string();
    source.history.items[0].transcript_path = format!("history/{history_id}.json");
    source.history.transcript_files[0].0 = format!("{history_id}.json");
    source.history.summary_files[0].0 = history_id.to_string();
    source.history.snapshot_files = snapshot_files(history_id);
    source
}

fn assert_export_fails_without_artifacts(source: &BackupDataset, name: &str) -> BackupError {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join(name);
    let repository = FsBackupArchiveRepository::new();

    let error = repository
        .write_archive(archive_path.to_str().unwrap(), &manifest(), source)
        .unwrap_err();

    assert!(!archive_path.exists());
    assert!(staging_artifacts(&archive_path).is_empty());
    error
}

fn json_entry(path: &str, value: &impl serde::Serialize) -> TestEntry {
    TestEntry {
        path: path.to_string(),
        data: serde_json::to_vec_pretty(value).unwrap(),
        entry_type: EntryType::Regular,
    }
}

fn valid_entries() -> Vec<TestEntry> {
    let dataset = dataset();
    vec![
        json_entry(MANIFEST_PATH, &manifest()),
        json_entry("config/sona-config.json", &dataset.config),
        json_entry("projects/index.json", &dataset.projects),
        json_entry("history/index.json", &dataset.history.items),
        json_entry("history/history-1.json", &json!([])),
        json_entry(
            "history/history-1.summary.json",
            &json!({"text": "summary"}),
        ),
        json_entry(
            "history/versions/history-1/index.json",
            &json!([{
                "id": "snapshot-1",
                "historyId": "history-1",
                "reason": "polish",
                "createdAt": 40,
                "segmentCount": 0
            }]),
        ),
        json_entry(
            "history/versions/history-1/snapshot-1.json",
            &json!({
                "metadata": {
                    "id": "snapshot-1",
                    "historyId": "history-1",
                    "reason": "polish",
                    "createdAt": 40,
                    "segmentCount": 0
                },
                "segments": []
            }),
        ),
        json_entry("automation/rules.json", &dataset.automation.rules),
        json_entry(
            "automation/processed.json",
            &dataset.automation.processed_entries,
        ),
        TestEntry {
            path: "analytics/llm-usage.json".to_string(),
            data: dataset.analytics_content.into_bytes(),
            entry_type: EntryType::Regular,
        },
    ]
}

fn set_raw_header_path(header: &mut Header, path: &str) {
    assert!(path.len() < 100, "test path must fit the tar name field");
    let bytes = header.as_mut_bytes();
    bytes[..100].fill(0);
    bytes[..path.len()].copy_from_slice(path.as_bytes());
}

fn write_entries(path: &Path, entries: &[TestEntry]) {
    let file = File::create(path).unwrap();
    let encoder = BzEncoder::new(file, bzip2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for entry in entries {
        let mut header = Header::new_gnu();
        header.set_mode(0o600);
        header.set_uid(0);
        header.set_gid(0);
        header.set_mtime(0);
        header.set_size(entry.data.len() as u64);
        header.set_entry_type(entry.entry_type);
        set_raw_header_path(&mut header, &entry.path);
        header.set_cksum();
        builder.append(&header, entry.data.as_slice()).unwrap();
    }
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();
}

fn write_header_only_archive(path: &Path, entry_path: &str, size: u64) {
    let file = File::create(path).unwrap();
    let mut encoder = BzEncoder::new(file, bzip2::Compression::fast());
    let mut header = Header::new_gnu();
    header.set_mode(0o600);
    header.set_size(size);
    header.set_entry_type(EntryType::Regular);
    set_raw_header_path(&mut header, entry_path);
    header.set_cksum();
    encoder.write_all(header.as_bytes()).unwrap();
    encoder.finish().unwrap();
}

fn write_raw_tar(path: &Path, bytes: &[u8]) {
    fs::write(path, compress_bytes(bytes)).unwrap();
}

fn compress_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = BzEncoder::new(Vec::new(), bzip2::Compression::fast());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}

fn decoded_tar(path: &Path) -> Vec<u8> {
    let file = File::open(path).unwrap();
    let mut decoder = bzip2::read::BzDecoder::new(file);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded).unwrap();
    decoded
}

fn raw_header(entry_path: &str, size: u64) -> Header {
    let mut header = Header::new_gnu();
    header.set_mode(0o600);
    header.set_size(size);
    header.set_entry_type(EntryType::Regular);
    set_raw_header_path(&mut header, entry_path);
    header.set_cksum();
    header
}

fn archive_paths(path: &Path) -> Vec<String> {
    let file = File::open(path).unwrap();
    let decoder = bzip2::read::BzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let mut paths = archive
        .entries()
        .unwrap()
        .map(|entry| {
            entry
                .unwrap()
                .path()
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

fn staging_artifacts(archive_path: &Path) -> Vec<PathBuf> {
    let file_name = archive_path.file_name().unwrap().to_string_lossy();
    let prefix = format!(".{file_name}.sona-staging-");
    fs::read_dir(archive_path.parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .collect()
}

fn prepare_artifacts(archive_path: &Path) -> Vec<PathBuf> {
    let file_name = archive_path.file_name().unwrap().to_string_lossy();
    let prefix = format!(".{file_name}.sona-prepare-");
    fs::read_dir(std::env::temp_dir())
        .unwrap()
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().starts_with(&prefix))
        })
        .collect()
}

fn assert_prepare_fails_cleanly(
    repository: &FsBackupArchiveRepository,
    archive_path: &Path,
) -> String {
    let error = repository
        .prepare_import(archive_path.to_str().unwrap())
        .unwrap_err();
    assert!(!error.to_string().is_empty());
    assert!(
        prepare_artifacts(archive_path).is_empty(),
        "prepare failure left extraction artifacts"
    );
    error.to_string()
}

struct FixedBackupState(BackupDataset);

impl BackupStateRepository for FixedBackupState {
    fn snapshot(&self) -> Result<BackupDataset, BackupError> {
        Ok(self.0.clone())
    }

    fn replace_all(
        &self,
        _dataset: BackupRestoreDataset,
    ) -> Result<BackupApplyResult, BackupError> {
        unreachable!("export and inspect must not replace backup state")
    }
}

struct FixedClock(u64);

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(self.0)
    }
}

#[test]
fn filesystem_backup_adapter_composes_archive_state_and_clock() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("adapter.sona-backup");
    let archive_path = archive_path.to_string_lossy().into_owned();
    let adapter = FsBackupAdapter::new(FixedBackupState(dataset()), FixedClock(1_234));

    let manifest = adapter
        .export_archive(BackupExportRequest {
            archive_path: archive_path.clone(),
            app_version: "0.8.0".to_string(),
        })
        .unwrap();

    assert_eq!(manifest.created_at, "1970-01-01T00:00:01.234Z");
    assert!(Path::new(&archive_path).is_file());

    let preview = adapter
        .inspect_archive(BackupInspectRequest { archive_path })
        .unwrap();

    assert_eq!(preview.manifest.app_version, "0.8.0");
    assert_eq!(preview.manifest.counts.projects, 1);
}

#[test]
fn exposes_documented_archive_limits() {
    assert_eq!(MAX_BACKUP_ENTRIES, 100_000);
    assert_eq!(MAX_BACKUP_FILE_BYTES, 64 * 1024 * 1024);
    assert_eq!(MAX_BACKUP_EXPANDED_BYTES, 4 * 1024 * 1024 * 1024);
}

#[test]
fn writes_prepares_loads_and_disposes_v1_archive_with_exact_layout() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("roundtrip.sona-backup");
    let repository = FsBackupArchiveRepository::new();
    let source = dataset();

    repository
        .write_archive(archive_path.to_str().unwrap(), &manifest(), &source)
        .unwrap();

    assert_eq!(
        archive_paths(&archive_path),
        vec![
            "analytics/llm-usage.json",
            "automation/processed.json",
            "automation/rules.json",
            "config/sona-config.json",
            "history/history-1.json",
            "history/history-1.summary.json",
            "history/index.json",
            "history/versions/history-1/index.json",
            "history/versions/history-1/snapshot-1.json",
            "manifest.json",
            "projects/index.json",
        ]
    );

    let preview = repository
        .prepare_import(archive_path.to_str().unwrap())
        .unwrap();
    assert_eq!(preview.manifest, manifest());
    assert_eq!(preview.config, source.config);
    assert_eq!(
        preview.projects,
        vec![serde_json::to_value(project()).unwrap()]
    );
    assert_eq!(
        preview.automation_rules,
        vec![serde_json::to_value(&source.automation.rules[0]).unwrap()]
    );
    assert_eq!(preview.analytics_content, source.analytics_content);
    assert_eq!(prepare_artifacts(&archive_path).len(), 1);

    let session = repository.load_prepared(&preview.import_id).unwrap();
    assert_eq!(session.import_id, preview.import_id);
    assert_eq!(session.manifest, preview.manifest);
    assert_eq!(session.dataset.projects, vec![project()]);
    assert_eq!(session.dataset.history.items, vec![history_item()]);
    assert_eq!(session.dataset.history.transcript_files.len(), 1);
    assert_eq!(session.dataset.history.summary_files.len(), 1);
    assert_eq!(session.dataset.history.snapshot_files.len(), 2);
    assert_eq!(session.dataset.automation, automation());
    assert!(prepare_artifacts(&archive_path).is_empty());
    assert!(repository.load_prepared(&preview.import_id).is_err());

    repository.dispose_prepared(&preview.import_id).unwrap();
}

#[test]
fn export_rejects_snapshot_records_without_an_index() {
    let mut source = dataset();
    source.history.snapshot_files.remove(0);

    assert_export_fails_without_artifacts(&source, "missing-index.sona");
}

#[test]
fn export_rejects_orphan_snapshot_records_for_unknown_history() {
    let mut source = dataset();
    source.history.snapshot_files = snapshot_files("unknown-history");

    assert_export_fails_without_artifacts(&source, "orphan-snapshot.sona");
}

#[test]
fn export_rejects_missing_indexed_snapshot_records() {
    let mut source = dataset();
    source.history.snapshot_files.remove(1);

    assert_export_fails_without_artifacts(&source, "missing-record.sona");
}

#[test]
fn export_rejects_snapshot_index_history_mismatch() {
    let mut source = dataset();
    source.history.snapshot_files[0].1[0]["historyId"] = json!("other-history");
    source.history.snapshot_files[1].1["metadata"]["historyId"] = json!("other-history");

    assert_export_fails_without_artifacts(&source, "mismatched-history.sona");
}

#[test]
fn export_rejects_snapshot_record_metadata_mismatch() {
    let mut source = dataset();
    source.history.snapshot_files[1].1["metadata"]["reason"] = json!("different");

    assert_export_fails_without_artifacts(&source, "mismatched-metadata.sona");
}

#[test]
fn export_rejects_extra_unindexed_snapshot_records() {
    let mut source = dataset();
    source.history.snapshot_files.push((
        "versions/history-1/snapshot-2.json".to_string(),
        json!({
            "metadata": {
                "id": "snapshot-2",
                "historyId": "history-1",
                "reason": "extra",
                "createdAt": 41,
                "segmentCount": 0
            },
            "segments": []
        }),
    ));

    assert_export_fails_without_artifacts(&source, "extra-record.sona");
}

#[test]
fn export_rejects_duplicate_snapshot_paths() {
    let mut source = dataset();
    source
        .history
        .snapshot_files
        .push(source.history.snapshot_files[1].clone());

    assert_export_fails_without_artifacts(&source, "duplicate-snapshot.sona");
}

#[test]
fn export_rejects_paths_that_require_tar_extension_records() {
    let source = dataset_with_history_id(&"h".repeat(88));
    let error = assert_export_fails_without_artifacts(&source, "overlong-path.sona");

    assert!(matches!(error, BackupError::Archive(_)));
    assert!(error.to_string().contains("tar header"));
}

#[test]
fn export_accepts_paths_at_the_direct_tar_name_boundary() {
    let source = dataset_with_history_id(&"h".repeat(67));
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("path-boundary.sona");
    let repository = FsBackupArchiveRepository::new();

    repository
        .write_archive(archive_path.to_str().unwrap(), &manifest(), &source)
        .unwrap();
    let paths = archive_paths(&archive_path);

    assert!(paths.iter().any(|path| path.len() == 100));
    repository
        .prepare_import(archive_path.to_str().unwrap())
        .unwrap();
}

#[test]
fn disposing_a_prepared_session_is_idempotent() {
    let repository = FsBackupArchiveRepository::new();
    repository.dispose_prepared("unknown").unwrap();
    repository.dispose_prepared("unknown").unwrap();
}

#[test]
fn disposing_a_prepared_workspace_removes_its_disk_state() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("dispose-workspace.sona");
    write_entries(&archive_path, &valid_entries());
    let repository = FsBackupArchiveRepository::new();
    let preview = repository
        .prepare_import(archive_path.to_str().unwrap())
        .unwrap();
    assert_eq!(prepare_artifacts(&archive_path).len(), 1);

    repository.dispose_prepared(&preview.import_id).unwrap();
    repository.dispose_prepared(&preview.import_id).unwrap();

    assert!(prepare_artifacts(&archive_path).is_empty());
}

#[test]
fn publishes_from_a_same_directory_staging_file() {
    let temp = tempfile::tempdir().unwrap();
    let output_dir = temp.path().join("nested");
    let archive_path = output_dir.join("atomic.sona-backup");
    let repository = FsBackupArchiveRepository::new();

    repository
        .write_archive(archive_path.to_str().unwrap(), &manifest(), &dataset())
        .unwrap();

    assert!(archive_path.is_file());
    assert!(staging_artifacts(&archive_path).is_empty());
}

#[test]
fn atomically_replaces_an_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("replace.sona-backup");
    fs::write(&archive_path, b"old archive").unwrap();
    let repository = FsBackupArchiveRepository::new();

    repository
        .write_archive(archive_path.to_str().unwrap(), &manifest(), &dataset())
        .unwrap();

    assert_ne!(fs::read(&archive_path).unwrap(), b"old archive");
    assert!(archive_paths(&archive_path).contains(&MANIFEST_PATH.to_string()));
    assert!(staging_artifacts(&archive_path).is_empty());
}

#[cfg(windows)]
#[test]
fn failed_windows_replacement_preserves_existing_output_and_removes_staging() {
    use std::os::windows::fs::OpenOptionsExt;

    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("locked.sona-backup");
    fs::write(&archive_path, b"old archive").unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(0)
        .open(&archive_path)
        .unwrap();
    let repository = FsBackupArchiveRepository::new();

    let error = repository
        .write_archive(archive_path.to_str().unwrap(), &manifest(), &dataset())
        .unwrap_err();

    assert!(!error.to_string().is_empty());
    drop(locked);
    assert_eq!(fs::read(&archive_path).unwrap(), b"old archive");
    assert!(staging_artifacts(&archive_path).is_empty());
}

#[test]
fn rejects_malformed_json_and_cleans_extraction() {
    let temp = tempfile::tempdir().unwrap();
    let repository = FsBackupArchiveRepository::new();

    for invalid_path in [
        "manifest.json",
        "config/sona-config.json",
        "projects/index.json",
        "history/index.json",
        "automation/rules.json",
        "automation/processed.json",
        "analytics/llm-usage.json",
    ] {
        let archive_path = temp
            .path()
            .join(format!("malformed-{}.sona", Uuid::new_v4()));
        let mut entries = valid_entries();
        entries
            .iter_mut()
            .find(|entry| entry.path == invalid_path)
            .unwrap()
            .data = b"{".to_vec();
        write_entries(&archive_path, &entries);
        assert_prepare_fails_cleanly(&repository, &archive_path);
    }
}

#[test]
fn rejects_unsupported_manifest_policy_and_count_mismatches() {
    let temp = tempfile::tempdir().unwrap();
    let repository = FsBackupArchiveRepository::new();
    let mut invalid_manifests = Vec::new();

    let mut unsupported_schema = manifest();
    unsupported_schema.schema_version = 2;
    invalid_manifests.push(unsupported_schema);
    let mut unsupported_mode = manifest();
    unsupported_mode.history_mode = "full".to_string();
    invalid_manifests.push(unsupported_mode);
    for scope in 0..5 {
        let mut missing_scope = manifest();
        match scope {
            0 => missing_scope.scopes.config = false,
            1 => missing_scope.scopes.workspace = false,
            2 => missing_scope.scopes.history = false,
            3 => missing_scope.scopes.automation = false,
            _ => missing_scope.scopes.analytics = false,
        }
        invalid_manifests.push(missing_scope);
    }
    for count in 0..7 {
        let mut mismatched = manifest();
        match count {
            0 => mismatched.counts.projects += 1,
            1 => mismatched.counts.history_items += 1,
            2 => mismatched.counts.transcript_files += 1,
            3 => mismatched.counts.summary_files += 1,
            4 => mismatched.counts.automation_rules += 1,
            5 => mismatched.counts.automation_processed_entries += 1,
            _ => mismatched.counts.analytics_files += 1,
        }
        invalid_manifests.push(mismatched);
    }

    for invalid_manifest in invalid_manifests {
        let archive_path = temp
            .path()
            .join(format!("manifest-{}.sona", Uuid::new_v4()));
        let mut entries = valid_entries();
        entries
            .iter_mut()
            .find(|entry| entry.path == MANIFEST_PATH)
            .unwrap()
            .data = serde_json::to_vec(&invalid_manifest).unwrap();
        write_entries(&archive_path, &entries);
        assert_prepare_fails_cleanly(&repository, &archive_path);
    }
}

#[test]
fn rejects_portable_path_traversal_and_windows_prefixes() {
    let temp = tempfile::tempdir().unwrap();
    let repository = FsBackupArchiveRepository::new();

    for unsafe_path in [
        "../outside.json",
        "..\\outside.json",
        "/absolute.json",
        "\\absolute.json",
        "C:/outside.json",
        "C:\\outside.json",
        "//server/share.json",
        "\\\\server\\share.json",
    ] {
        let archive_path = temp
            .path()
            .join(format!("traversal-{}.sona", Uuid::new_v4()));
        write_entries(
            &archive_path,
            &[TestEntry {
                path: unsafe_path.to_string(),
                data: b"{}".to_vec(),
                entry_type: EntryType::Regular,
            }],
        );
        assert_prepare_fails_cleanly(&repository, &archive_path);
    }
}

#[test]
fn rejects_backslashes_in_archive_paths_instead_of_normalizing_them() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("backslash.sona");
    write_entries(
        &archive_path,
        &[TestEntry {
            path: "nested\\file.json".to_string(),
            data: b"{}".to_vec(),
            entry_type: EntryType::Regular,
        }],
    );

    let error = assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
    assert!(error.contains("unsafe entry path"));
}

#[test]
fn rejects_paths_that_duplicate_after_normalization() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp
        .path()
        .join(format!("duplicate-{}.sona", Uuid::new_v4()));
    let mut entries = valid_entries();
    let manifest_data = entries[0].data.clone();
    entries.push(TestEntry {
        path: "./manifest.json".to_string(),
        data: manifest_data,
        entry_type: EntryType::Regular,
    });
    write_entries(&archive_path, &entries);

    assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
}

#[test]
fn rejects_every_unsupported_tar_entry_type() {
    let temp = tempfile::tempdir().unwrap();
    let repository = FsBackupArchiveRepository::new();

    for entry_type in [
        EntryType::Symlink,
        EntryType::Link,
        EntryType::Char,
        EntryType::Block,
        EntryType::Fifo,
        EntryType::Continuous,
        EntryType::XHeader,
        EntryType::XGlobalHeader,
        EntryType::GNULongName,
        EntryType::GNULongLink,
        EntryType::GNUSparse,
        EntryType::new(b'Z'),
    ] {
        let archive_path = temp
            .path()
            .join(format!("entry-type-{}.sona", Uuid::new_v4()));
        write_entries(
            &archive_path,
            &[TestEntry {
                path: "manifest.json".to_string(),
                data: vec![],
                entry_type,
            }],
        );
        let error = assert_prepare_fails_cleanly(&repository, &archive_path);
        assert!(
            error.contains("unsupported non-regular"),
            "unexpected rejection for {entry_type:?}: {error}"
        );
    }
}

#[test]
fn accepts_safe_directory_entries_without_treating_them_as_dataset_files() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("directories.sona");
    let mut entries = [
        "config/",
        "projects/",
        "history/",
        "automation/",
        "analytics/",
    ]
    .into_iter()
    .map(|path| TestEntry {
        path: path.to_string(),
        data: vec![],
        entry_type: EntryType::Directory,
    })
    .collect::<Vec<_>>();
    entries.extend(valid_entries());
    write_entries(&archive_path, &entries);
    let repository = FsBackupArchiveRepository::new();

    let preview = repository
        .prepare_import(archive_path.to_str().unwrap())
        .unwrap();

    assert_eq!(preview.manifest, manifest());
    repository.dispose_prepared(&preview.import_id).unwrap();
}

#[test]
fn rejects_unsafe_and_duplicate_directory_entries() {
    let temp = tempfile::tempdir().unwrap();
    let repository = FsBackupArchiveRepository::new();

    let unsafe_archive = temp.path().join("unsafe-directory.sona");
    write_entries(
        &unsafe_archive,
        &[TestEntry {
            path: "nested\\directory/".to_string(),
            data: vec![],
            entry_type: EntryType::Directory,
        }],
    );
    let error = assert_prepare_fails_cleanly(&repository, &unsafe_archive);
    assert!(error.contains("unsafe entry path"));

    let duplicate_archive = temp.path().join("duplicate-directory.sona");
    write_entries(
        &duplicate_archive,
        &[
            TestEntry {
                path: "nested/".to_string(),
                data: vec![],
                entry_type: EntryType::Directory,
            },
            TestEntry {
                path: "nested".to_string(),
                data: vec![],
                entry_type: EntryType::Directory,
            },
        ],
    );
    let error = assert_prepare_fails_cleanly(&repository, &duplicate_archive);
    assert!(error.contains("duplicate path"));
}

#[test]
fn rejects_nonzero_decoded_data_after_the_tar_terminator() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("trailing-decoded.sona");
    write_entries(&archive_path, &valid_entries());
    let mut decoded = decoded_tar(&archive_path);
    decoded.extend_from_slice(b"trailing");
    write_raw_tar(&archive_path, &decoded);

    let error = assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
    assert!(error.contains("trailing"));
}

#[test]
fn rejects_concatenated_bzip_streams_after_a_valid_archive() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("concatenated.sona");
    write_entries(&archive_path, &valid_entries());
    let mut compressed = fs::read(&archive_path).unwrap();
    compressed.extend_from_slice(&compress_bytes(b"trailing"));
    fs::write(&archive_path, compressed).unwrap();

    let error = assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
    assert!(error.contains("trailing"));
}

#[test]
fn surfaces_a_truncated_bzip_trailer_after_the_tar_terminator() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp.path().join("truncated-trailer.sona");
    write_entries(&archive_path, &valid_entries());
    let mut compressed = fs::read(&archive_path).unwrap();
    compressed.pop();
    fs::write(&archive_path, compressed).unwrap();

    assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
}

#[test]
fn rejects_malformed_raw_tar_structure() {
    let temp = tempfile::tempdir().unwrap();
    let repository = FsBackupArchiveRepository::new();
    let mut malformed_archives = Vec::new();

    let mut invalid_octal = raw_header("manifest.json", 0);
    invalid_octal.as_mut_bytes()[124] = b'8';
    invalid_octal.set_cksum();
    malformed_archives.push(invalid_octal.as_bytes().to_vec());

    let mut base_256 = raw_header("manifest.json", 0);
    base_256.as_mut_bytes()[124..136].fill(0);
    base_256.as_mut_bytes()[124] = 0x80;
    base_256.set_cksum();
    malformed_archives.push(base_256.as_bytes().to_vec());

    let mut bad_checksum = raw_header("manifest.json", 0).as_bytes().to_vec();
    bad_checksum[0] ^= 1;
    malformed_archives.push(bad_checksum);

    malformed_archives.push(raw_header("manifest.json", 1).as_bytes().to_vec());
    malformed_archives.push(raw_header("manifest.json", 0).as_bytes()[..100].to_vec());

    let mut one_zero_block = raw_header("manifest.json", 0).as_bytes().to_vec();
    one_zero_block.extend_from_slice(&[0; 512]);
    malformed_archives.push(one_zero_block);

    for bytes in malformed_archives {
        let archive_path = temp
            .path()
            .join(format!("malformed-tar-{}.sona", Uuid::new_v4()));
        write_raw_tar(&archive_path, &bytes);
        assert_prepare_fails_cleanly(&repository, &archive_path);
    }
}

#[test]
fn rejects_archives_over_the_entry_count_limit() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp
        .path()
        .join(format!("entry-limit-{}.sona", Uuid::new_v4()));
    let file = File::create(&archive_path).unwrap();
    let encoder = BzEncoder::new(file, bzip2::Compression::fast());
    let mut builder = tar::Builder::new(encoder);
    for index in 0..=MAX_BACKUP_ENTRIES {
        let mut header = Header::new_gnu();
        header.set_mode(0o600);
        header.set_size(0);
        header.set_entry_type(EntryType::Regular);
        header.set_path(format!("files/{index}.json")).unwrap();
        header.set_cksum();
        builder.append(&header, io::empty()).unwrap();
    }
    let encoder = builder.into_inner().unwrap();
    encoder.finish().unwrap();

    assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
}

#[test]
fn rejects_a_file_size_over_the_per_entry_limit_before_unpacking() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp
        .path()
        .join(format!("file-limit-{}.sona", Uuid::new_v4()));
    write_header_only_archive(&archive_path, "manifest.json", MAX_BACKUP_FILE_BYTES + 1);

    let error = assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
    assert!(error.contains("file size limit"));
}

#[test]
fn rejects_unexpected_files_outside_the_v1_layout() {
    let temp = tempfile::tempdir().unwrap();
    let archive_path = temp
        .path()
        .join(format!("unexpected-{}.sona", Uuid::new_v4()));
    let mut entries = valid_entries();
    entries.push(json_entry("extra.json", &Value::Null));
    write_entries(&archive_path, &entries);

    assert_prepare_fails_cleanly(&FsBackupArchiveRepository::new(), &archive_path);
}
