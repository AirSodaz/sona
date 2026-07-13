use serde_json::json;
use sona_core::storage_usage::{
    AudioUsageCategory, DatabaseUsageCategory, FileUsageCategory, SQLiteIndexUsageEntry,
    SQLiteUsageSummary, StorageUsageError, StorageUsageMeasurements, StorageUsageRepository,
    StorageUsageService, WebviewCacheUsageCategory, build_webview_clear_result,
};
use std::sync::Arc;

#[derive(Clone)]
struct FixedRepository {
    measurements: StorageUsageMeasurements,
}

impl StorageUsageRepository for FixedRepository {
    fn collect_measurements(&self) -> Result<StorageUsageMeasurements, StorageUsageError> {
        Ok(self.measurements.clone())
    }
}

struct FailingRepository;

impl StorageUsageRepository for FailingRepository {
    fn collect_measurements(&self) -> Result<StorageUsageMeasurements, StorageUsageError> {
        Err(StorageUsageError::Repository("scan failed".to_string()))
    }
}

fn file_usage(bytes: u64, file_count: u64) -> FileUsageCategory {
    FileUsageCategory { bytes, file_count }
}

fn measurements() -> StorageUsageMeasurements {
    StorageUsageMeasurements {
        history_audio: file_usage(10, 1),
        speaker_samples: file_usage(20, 2),
        database: DatabaseUsageCategory {
            bytes: 40,
            sqlite: SQLiteUsageSummary {
                data_bytes: 24,
                index_bytes: 8,
                free_page_bytes: 8,
                dbstat_available: true,
                ..Default::default()
            },
        },
        models: file_usage(50, 3),
        temporary: file_usage(60, 4),
        webview_cache: WebviewCacheUsageCategory {
            bytes: Some(70),
            clear_supported: true,
            path: Some("C:\\cache".to_string()),
        },
        other: file_usage(80, 5),
    }
}

#[test]
fn service_aggregates_measurements_into_the_public_snapshot() {
    let service = StorageUsageService::new(Arc::new(FixedRepository {
        measurements: measurements(),
    }));

    let snapshot = service
        .load_snapshot_at("2026-07-13T08:00:00+00:00".to_string())
        .unwrap();

    assert_eq!(snapshot.generated_at, "2026-07-13T08:00:00+00:00");
    assert_eq!(
        snapshot.categories.audio,
        AudioUsageCategory {
            bytes: 30,
            history_audio_bytes: 10,
            speaker_sample_bytes: 20,
            file_count: 3,
        }
    );
    assert_eq!(snapshot.categories.database.bytes, 40);
    assert_eq!(snapshot.categories.models, file_usage(50, 3));
    assert_eq!(snapshot.categories.temporary, file_usage(60, 4));
    assert_eq!(snapshot.categories.webview_cache.bytes, Some(70));
    assert_eq!(snapshot.categories.other, file_usage(80, 5));
    assert_eq!(snapshot.total_bytes, 330);
}

#[test]
fn service_uses_saturating_totals_and_optional_webview_bytes() {
    let mut input = measurements();
    input.history_audio.bytes = u64::MAX;
    input.speaker_samples.bytes = 1;
    input.history_audio.file_count = u64::MAX;
    input.speaker_samples.file_count = 1;
    input.webview_cache.bytes = None;
    input.webview_cache.path = None;
    let service = StorageUsageService::new(Arc::new(FixedRepository {
        measurements: input,
    }));

    let snapshot = service.load_snapshot_at("fixed".to_string()).unwrap();

    assert_eq!(snapshot.categories.audio.bytes, u64::MAX);
    assert_eq!(snapshot.categories.audio.file_count, u64::MAX);
    assert_eq!(snapshot.categories.webview_cache.bytes, None);
    assert_eq!(snapshot.categories.webview_cache.path, None);
    assert_eq!(snapshot.total_bytes, u64::MAX);
}

#[test]
fn service_preserves_repository_errors() {
    let error = StorageUsageService::new(Arc::new(FailingRepository))
        .load_snapshot_at("unused".to_string())
        .unwrap_err();

    assert_eq!(
        error,
        StorageUsageError::Repository("scan failed".to_string())
    );
    assert_eq!(
        error.to_string(),
        "Storage usage repository error: scan failed"
    );
}

#[test]
fn webview_clear_result_is_core_owned_and_pure() {
    let result = build_webview_clear_result(Some(128), Some(64));

    assert_eq!(result.before_bytes, Some(128));
    assert_eq!(result.after_bytes, Some(64));
    assert!(result.clear_requested);
}

#[test]
fn public_models_preserve_the_existing_camel_case_json_contract() {
    let mut input = measurements();
    input.webview_cache.bytes = None;
    input.webview_cache.path = None;
    input.database.sqlite.index_entries = vec![SQLiteIndexUsageEntry {
        schema: "main".to_string(),
        name: "idx_history".to_string(),
        bytes: 8,
    }];
    let snapshot = StorageUsageService::new(Arc::new(FixedRepository {
        measurements: input,
    }))
    .load_snapshot_at("fixed".to_string())
    .unwrap();

    assert_eq!(
        serde_json::to_value(snapshot).unwrap(),
        json!({
            "generatedAt": "fixed",
            "totalBytes": 260,
            "categories": {
                "audio": {
                    "bytes": 30,
                    "historyAudioBytes": 10,
                    "speakerSampleBytes": 20,
                    "fileCount": 3
                },
                "database": {
                    "bytes": 40,
                    "sqlite": {
                        "mainDbBytes": 0,
                        "mainWalBytes": 0,
                        "mainShmBytes": 0,
                        "analyticsDbBytes": 0,
                        "analyticsWalBytes": 0,
                        "analyticsShmBytes": 0,
                        "dataBytes": 24,
                        "indexBytes": 8,
                        "freePageBytes": 8,
                        "indexEntries": [{
                            "schema": "main",
                            "name": "idx_history",
                            "bytes": 8
                        }],
                        "dbstatAvailable": true
                    }
                },
                "models": {"bytes": 50, "fileCount": 3},
                "temporary": {"bytes": 60, "fileCount": 4},
                "webviewCache": {"bytes": null, "clearSupported": true},
                "other": {"bytes": 80, "fileCount": 5}
            }
        })
    );
    assert_eq!(
        serde_json::to_value(build_webview_clear_result(Some(128), Some(64))).unwrap(),
        json!({
            "beforeBytes": 128,
            "afterBytes": 64,
            "clearRequested": true
        })
    );
}
