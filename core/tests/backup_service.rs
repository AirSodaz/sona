use std::collections::VecDeque;
use std::sync::Mutex;

use serde_json::{Value, json};
use sona_core::automation::repository::AutomationRepositoryState;
use sona_core::backup::{
    BackupApplyPreparedImportRequest, BackupApplyResult, BackupArchivePort, BackupDataset,
    BackupError, BackupExportRequest, BackupImportRequest, BackupInspectRequest, BackupManifest,
    BackupManifestCounts, BackupManifestScopes, BackupPrepareImportRequest, BackupRestoreDataset,
    BackupService, BackupStateRepository, PreparedBackupImport, PreparedBackupSession,
};
use sona_core::config::CURRENT_CONFIG_VERSION;
use sona_core::history::HistoryBackupSnapshot;
use sona_core::ports::time::UnixMillisClock;

const FIXED_NOW_MS: u64 = 1_783_900_800_123;

struct FixedClock;

impl UnixMillisClock for FixedClock {
    fn now_ms(&self) -> Result<u64, String> {
        Ok(FIXED_NOW_MS)
    }
}

struct FailingClock;

impl UnixMillisClock for FailingClock {
    fn now_ms(&self) -> Result<u64, String> {
        Err("clock before Unix epoch".to_string())
    }
}

fn service<'a>(archive: &'a RecordingArchive, state: &'a RecordingState) -> BackupService<'a> {
    BackupService::new(archive, state, &FixedClock)
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ArchiveCall {
    Write {
        archive_path: String,
        manifest: BackupManifest,
        project_count: usize,
    },
    Prepare(String),
    Load(String),
    Dispose(String),
}

#[derive(Default)]
struct RecordingArchive {
    calls: Mutex<Vec<ArchiveCall>>,
    write_result: Mutex<Option<Result<(), BackupError>>>,
    prepare_result: Mutex<Option<Result<PreparedBackupImport, BackupError>>>,
    load_result: Mutex<Option<Result<PreparedBackupSession, BackupError>>>,
    dispose_results: Mutex<VecDeque<Result<(), BackupError>>>,
}

impl RecordingArchive {
    fn with_prepare(preview: PreparedBackupImport) -> Self {
        Self {
            prepare_result: Mutex::new(Some(Ok(preview))),
            ..Self::default()
        }
    }

    fn calls(&self) -> Vec<ArchiveCall> {
        self.calls.lock().unwrap().clone()
    }

    fn set_load(&self, result: Result<PreparedBackupSession, BackupError>) {
        *self.load_result.lock().unwrap() = Some(result);
    }

    fn push_dispose(&self, result: Result<(), BackupError>) {
        self.dispose_results.lock().unwrap().push_back(result);
    }
}

impl BackupArchivePort for RecordingArchive {
    fn write_archive(
        &self,
        archive_path: &str,
        manifest: &BackupManifest,
        dataset: &BackupDataset,
    ) -> Result<(), BackupError> {
        self.calls.lock().unwrap().push(ArchiveCall::Write {
            archive_path: archive_path.to_string(),
            manifest: manifest.clone(),
            project_count: dataset.projects.len(),
        });
        self.write_result.lock().unwrap().take().unwrap_or(Ok(()))
    }

    fn prepare_import(&self, archive_path: &str) -> Result<PreparedBackupImport, BackupError> {
        self.calls
            .lock()
            .unwrap()
            .push(ArchiveCall::Prepare(archive_path.to_string()));
        self.prepare_result
            .lock()
            .unwrap()
            .take()
            .expect("prepare result")
    }

    fn load_prepared(&self, import_id: &str) -> Result<PreparedBackupSession, BackupError> {
        self.calls
            .lock()
            .unwrap()
            .push(ArchiveCall::Load(import_id.to_string()));
        self.load_result
            .lock()
            .unwrap()
            .take()
            .expect("load result")
    }

    fn dispose_prepared(&self, import_id: &str) -> Result<(), BackupError> {
        self.calls
            .lock()
            .unwrap()
            .push(ArchiveCall::Dispose(import_id.to_string()));
        self.dispose_results
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(()))
    }
}

#[derive(Default)]
struct RecordingState {
    snapshot_calls: Mutex<usize>,
    snapshot_result: Mutex<Option<Result<BackupDataset, BackupError>>>,
    restore_calls: Mutex<Vec<BackupRestoreDataset>>,
    replace_result: Mutex<Option<Result<BackupApplyResult, BackupError>>>,
}

impl RecordingState {
    fn with_snapshot(dataset: BackupDataset) -> Self {
        Self {
            snapshot_result: Mutex::new(Some(Ok(dataset))),
            ..Self::default()
        }
    }

    fn set_replace(&self, result: Result<BackupApplyResult, BackupError>) {
        *self.replace_result.lock().unwrap() = Some(result);
    }
}

impl BackupStateRepository for RecordingState {
    fn snapshot(&self) -> Result<BackupDataset, BackupError> {
        *self.snapshot_calls.lock().unwrap() += 1;
        self.snapshot_result
            .lock()
            .unwrap()
            .take()
            .expect("snapshot result")
    }

    fn replace_all(&self, dataset: BackupRestoreDataset) -> Result<BackupApplyResult, BackupError> {
        let default_result = BackupApplyResult {
            import_id: dataset.import_id.clone(),
            manifest: dataset.manifest.clone(),
        };
        self.restore_calls.lock().unwrap().push(dataset);
        self.replace_result
            .lock()
            .unwrap()
            .take()
            .unwrap_or(Ok(default_result))
    }
}

fn empty_history() -> HistoryBackupSnapshot {
    HistoryBackupSnapshot {
        items: vec![],
        transcript_files: vec![],
        summary_files: vec![],
        snapshot_files: vec![],
    }
}

fn dataset(config: Value) -> BackupDataset {
    BackupDataset {
        config,
        projects: vec![],
        history: empty_history(),
        automation: AutomationRepositoryState::default(),
        analytics_content: "[]".to_string(),
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
            projects: 0,
            history_items: 0,
            transcript_files: 0,
            summary_files: 0,
            automation_rules: 0,
            automation_processed_entries: 0,
            analytics_files: 1,
        },
    }
}

fn preview(import_id: &str) -> PreparedBackupImport {
    PreparedBackupImport {
        import_id: import_id.to_string(),
        archive_path: "backup.sona-backup".to_string(),
        manifest: manifest(),
        config: json!({}),
        projects: vec![],
        automation_rules: vec![],
        automation_processed_entries: vec![],
        analytics_content: "[]".to_string(),
    }
}

fn session(import_id: &str, config: Value) -> PreparedBackupSession {
    PreparedBackupSession {
        import_id: import_id.to_string(),
        manifest: manifest(),
        dataset: dataset(config),
    }
}

#[test]
fn export_validates_request_before_calling_either_port() {
    let archive = RecordingArchive::default();
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let error = service
        .export_archive(BackupExportRequest {
            archive_path: "  ".to_string(),
            app_version: "0.8.0".to_string(),
        })
        .unwrap_err();

    assert!(matches!(error, BackupError::InvalidRequest(_)));
    assert_eq!(*state.snapshot_calls.lock().unwrap(), 0);
    assert!(archive.calls().is_empty());
}

#[test]
fn export_builds_manifest_from_the_typed_snapshot_and_writes_it() {
    let archive = RecordingArchive::default();
    let state = RecordingState::with_snapshot(dataset(json!({})));
    let service = service(&archive, &state);

    let result = service
        .export_archive(BackupExportRequest {
            archive_path: "backup.sona-backup".to_string(),
            app_version: "0.8.0".to_string(),
        })
        .unwrap();

    assert_eq!(result.schema_version, 1);
    assert_eq!(result.created_at, "2026-07-13T00:00:00.123Z");
    assert_eq!(result.history_mode, "light");
    assert_eq!(result.counts.analytics_files, 1);
    assert_eq!(
        archive.calls(),
        vec![ArchiveCall::Write {
            archive_path: "backup.sona-backup".to_string(),
            manifest: result,
            project_count: 0,
        }]
    );
}

#[test]
fn export_maps_clock_failure_without_writing_an_archive() {
    let archive = RecordingArchive::default();
    let state = RecordingState::with_snapshot(dataset(json!({})));
    let service = BackupService::new(&archive, &state, &FailingClock);

    let error = service
        .export_archive(BackupExportRequest {
            archive_path: "backup.sona-backup".to_string(),
            app_version: "0.8.0".to_string(),
        })
        .unwrap_err();

    assert_eq!(
        error,
        BackupError::State("Backup clock: clock before Unix epoch".to_string())
    );
    assert!(archive.calls().is_empty());
}

#[test]
fn prepare_returns_the_validated_archive_preview_without_disposing_it() {
    let expected = preview("prepared-1");
    let archive = RecordingArchive::with_prepare(expected.clone());
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let actual = service
        .prepare_import(BackupPrepareImportRequest {
            archive_path: "backup.sona-backup".to_string(),
        })
        .unwrap();

    assert_eq!(actual, expected);
    assert_eq!(
        archive.calls(),
        vec![ArchiveCall::Prepare("backup.sona-backup".to_string())]
    );
}

#[test]
fn apply_rejects_a_manifest_count_mismatch_before_replacing_state_and_disposes() {
    let mut prepared = preview("prepared-2");
    prepared.manifest.counts.projects = 1;
    prepared.projects.push(json!({"id": "preview-project"}));
    let archive = RecordingArchive::default();
    archive.set_load(Ok(PreparedBackupSession {
        import_id: prepared.import_id,
        manifest: prepared.manifest,
        dataset: dataset(json!({})),
    }));
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let error = service
        .apply_prepared_import(BackupApplyPreparedImportRequest {
            import_id: "prepared-2".to_string(),
            default_rule_set_name: "Imported Rules".to_string(),
        })
        .unwrap_err();

    assert!(matches!(error, BackupError::InvalidBackup(_)));
    assert!(state.restore_calls.lock().unwrap().is_empty());
    assert_eq!(
        archive.calls(),
        vec![
            ArchiveCall::Load("prepared-2".to_string()),
            ArchiveCall::Dispose("prepared-2".to_string()),
        ]
    );
}

#[test]
fn apply_migrates_config_and_builds_the_pure_stored_state_before_replace() {
    let archive = RecordingArchive::default();
    archive.set_load(Ok(session(
        "prepared-3",
        json!({
            "configVersion": 1,
            "textReplacements": [{"id": "old", "from": "foo", "to": "bar"}]
        }),
    )));
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let result = service
        .apply_prepared_import(BackupApplyPreparedImportRequest {
            import_id: "prepared-3".to_string(),
            default_rule_set_name: "Imported Rules".to_string(),
        })
        .unwrap();

    assert_eq!(result.import_id, "prepared-3");
    let calls = state.restore_calls.lock().unwrap();
    let restored = calls.first().expect("restore dataset");
    assert_eq!(restored.import_id, "prepared-3");
    assert_eq!(restored.config_state.config_version, CURRENT_CONFIG_VERSION);
    assert_eq!(
        restored.config_state.library.text_replacement_sets[0].name,
        "Imported Rules"
    );
    let base: Value = serde_json::from_str(&restored.config_state.base_config_json).unwrap();
    assert_eq!(base["configVersion"], CURRENT_CONFIG_VERSION);
    assert_eq!(restored.config_state.updated_at, FIXED_NOW_MS as i64);
    assert_eq!(
        archive.calls(),
        vec![
            ArchiveCall::Load("prepared-3".to_string()),
            ArchiveCall::Dispose("prepared-3".to_string()),
        ]
    );
}

#[test]
fn apply_keeps_the_primary_error_when_cleanup_also_fails() {
    let archive = RecordingArchive::default();
    archive.set_load(Ok(session("prepared-4", json!({}))));
    archive.push_dispose(Err(BackupError::Archive("cleanup".to_string())));
    let state = RecordingState::default();
    state.set_replace(Err(BackupError::State("primary".to_string())));
    let service = service(&archive, &state);

    let error = service
        .apply_prepared_import(BackupApplyPreparedImportRequest {
            import_id: "prepared-4".to_string(),
            default_rule_set_name: "Imported Rules".to_string(),
        })
        .unwrap_err();

    assert_eq!(error, BackupError::State("primary".to_string()));
}

#[test]
fn apply_propagates_cleanup_error_after_a_successful_replace() {
    let archive = RecordingArchive::default();
    archive.set_load(Ok(session("prepared-5", json!({}))));
    archive.push_dispose(Err(BackupError::Archive("cleanup".to_string())));
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let error = service
        .apply_prepared_import(BackupApplyPreparedImportRequest {
            import_id: "prepared-5".to_string(),
            default_rule_set_name: "Imported Rules".to_string(),
        })
        .unwrap_err();

    assert_eq!(error, BackupError::Archive("cleanup".to_string()));
    assert_eq!(state.restore_calls.lock().unwrap().len(), 1);
}

#[test]
fn dispose_is_idempotent_at_the_service_boundary() {
    let archive = RecordingArchive::default();
    let state = RecordingState::default();
    let service = service(&archive, &state);

    service.dispose_prepared_import("prepared-6").unwrap();
    service.dispose_prepared_import("prepared-6").unwrap();

    assert_eq!(
        archive.calls(),
        vec![
            ArchiveCall::Dispose("prepared-6".to_string()),
            ArchiveCall::Dispose("prepared-6".to_string()),
        ]
    );
}

#[test]
fn inspect_prepares_validates_and_disposes_without_loading_or_replacing() {
    let expected = preview("prepared-7");
    let archive = RecordingArchive::with_prepare(expected.clone());
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let actual = service
        .inspect_archive(BackupInspectRequest {
            archive_path: "backup.sona-backup".to_string(),
        })
        .unwrap();

    assert_eq!(actual, expected);
    assert_eq!(
        archive.calls(),
        vec![
            ArchiveCall::Prepare("backup.sona-backup".to_string()),
            ArchiveCall::Dispose("prepared-7".to_string()),
        ]
    );
    assert!(state.restore_calls.lock().unwrap().is_empty());
}

#[test]
fn one_shot_import_requires_confirmation_before_any_port_call() {
    let archive = RecordingArchive::default();
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let error = service
        .import_archive(BackupImportRequest {
            archive_path: "backup.sona-backup".to_string(),
            default_rule_set_name: "Imported Rules".to_string(),
            confirm_replace: false,
        })
        .unwrap_err();

    assert!(matches!(error, BackupError::ConfirmationRequired));
    assert!(archive.calls().is_empty());
    assert!(state.restore_calls.lock().unwrap().is_empty());
}

#[test]
fn one_shot_import_prepares_applies_and_disposes_the_session() {
    let prepared = preview("prepared-8");
    let archive = RecordingArchive::with_prepare(prepared.clone());
    archive.set_load(Ok(PreparedBackupSession {
        import_id: prepared.import_id,
        manifest: prepared.manifest,
        dataset: dataset(json!({})),
    }));
    let state = RecordingState::default();
    let service = service(&archive, &state);

    let result = service
        .import_archive(BackupImportRequest {
            archive_path: "backup.sona-backup".to_string(),
            default_rule_set_name: "Imported Rules".to_string(),
            confirm_replace: true,
        })
        .unwrap();

    assert_eq!(result.import_id, "prepared-8");
    assert_eq!(state.restore_calls.lock().unwrap().len(), 1);
    assert_eq!(
        archive.calls(),
        vec![
            ArchiveCall::Prepare("backup.sona-backup".to_string()),
            ArchiveCall::Load("prepared-8".to_string()),
            ArchiveCall::Dispose("prepared-8".to_string()),
        ]
    );
}
