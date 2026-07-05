use sona_core::task_ledger::types::{
    TASK_LEDGER_VERSION, TaskLedgerKind, TaskLedgerRecord, TaskLedgerSnapshot, TaskLedgerStatus,
};

#[test]
fn task_ledger_transport_shape_lives_in_core() {
    let snapshot = TaskLedgerSnapshot {
        version: TASK_LEDGER_VERSION,
        updated_at: Some(2000),
        tasks: vec![TaskLedgerRecord {
            id: "task-1".to_string(),
            kind: TaskLedgerKind::LlmSummary,
            status: TaskLedgerStatus::Recoverable,
            title: "Summarize".to_string(),
            progress: 42.0,
            created_at: 1000,
            updated_at: 2000,
            retryable: true,
            cancelable: false,
            recoverable: true,
            stage: Some("summary".to_string()),
            history_id: Some("history-1".to_string()),
            project_id: None,
            file_path: None,
            automation_rule_id: None,
            source_fingerprint: None,
            error_message: Some("network paused".to_string()),
            template_id: Some("general".to_string()),
            target_language: Some("zh".to_string()),
        }],
    };

    let value = serde_json::to_value(snapshot).unwrap();

    assert_eq!(value["version"], 1);
    assert_eq!(value["tasks"][0]["kind"], "llmSummary");
    assert_eq!(value["tasks"][0]["status"], "recoverable");
    assert_eq!(value["tasks"][0]["historyId"], "history-1");
    assert_eq!(value["tasks"][0]["targetLanguage"], "zh");
    assert!(value["tasks"][0].get("history_id").is_none());
    assert!(value["tasks"][0].get("target_language").is_none());
}
