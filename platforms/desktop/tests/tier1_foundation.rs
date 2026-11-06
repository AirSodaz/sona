//! Tier 1 Integration Tests: Core Linking & Desktop Integration Foundation (Features 1–8)
//!
//! Covers:
//! - Feature 4: Decoupled Core Direct Linking (`DesktopServices` composition without Tauri)
//! - Feature 5: In-Memory Real-Time Event Bus (`AppEventBus` broadcast streaming)
//! - Feature 6: Single-Instance Lock acquisition and duplicate detection semantics
//! - Feature 7: Auto-Start Integration & registry config generator verification
//! - Core Project Service direct CRUD lifecycle assertions

mod harness;

use serde_json::json;
use std::sync::Arc;

use harness::TestHarness;
use sona_core::ports::event::EventEmitterPort;
use sona_core::project::{ProjectCreateInput, ProjectUpdateInput};
use sona_desktop_lib::event_bus::AppEventBus;
use sona_desktop_lib::platform::auto_launch::{
    self, APP_NAME, AUTO_START_ARG, AutoLaunchBuilder, AutoLaunchManager, AutoLaunchStatus,
    MockAutoLaunch,
};
use sona_desktop_lib::platform::database::DesktopSqliteState;
use sona_desktop_lib::platform::single_instance::{
    SONA_SINGLE_INSTANCE_LOCK_NAME, SingleInstanceGuard,
};
use sona_desktop_lib::services::DesktopServices;

// ============================================================================
// Test 1: DesktopServices direct composition and instantiation without Tauri runtime
// ============================================================================

#[tokio::test]
async fn test_desktop_services_composition_without_tauri() {
    let harness = TestHarness::new();
    let services = &harness.services;

    // Verify all domain services are instantiated and functional in pure Rust
    let projects = services
        .projects
        .list()
        .await
        .expect("projects.list() should succeed");
    assert_eq!(projects.len(), 0, "Initial project list should be empty");

    let config = services
        .config
        .load()
        .expect("config.load() should succeed");
    assert_eq!(config, None, "Initial app config should be empty");

    let sync_status = services
        .sync
        .get_status()
        .await
        .expect("sync.get_status() should succeed");
    assert_eq!(
        sync_status.state,
        sona_core::sync::SyncLifecycleState::Disabled,
        "Initial sync state should be Disabled"
    );

    let ledger_snapshot = services
        .task_ledger
        .load_snapshot()
        .await
        .expect("task_ledger.load_snapshot() should succeed");
    assert_eq!(
        ledger_snapshot.tasks.len(),
        0,
        "Initial task ledger should be empty"
    );

    let history_items = services
        .history
        .query_db(|s| s.list_items(Default::default()))
        .await
        .expect("history.query_db() should succeed");
    assert_eq!(
        history_items.len(),
        0,
        "Initial history items should be empty"
    );

    // Verify recording session tracker
    assert!(
        services.recording_session.lock().await.is_none(),
        "Initial recording session should be empty"
    );

    // Verify downloads service handle exists
    assert!(Arc::strong_count(&services.downloads) >= 1);
}

#[tokio::test]
async fn test_harness_event_waiter_and_paths() {
    let harness = TestHarness::new();
    let mut sub = harness.subscribe();
    let path = harness.temp_path();
    assert!(path.exists(), "Harness temporary directory must exist");

    let bus = harness.event_bus.clone();
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        bus.emit("ping-event", json!({"seq": 42})).unwrap();
    });

    let event = TestHarness::wait_for_event(
        &mut sub,
        "ping-event",
        std::time::Duration::from_millis(500),
    )
    .await
    .expect("wait_for_event should succeed");

    assert_eq!(event.name, "ping-event");
    assert_eq!(event.payload["seq"], 42);
}

#[tokio::test]
async fn test_desktop_services_builder_validation_errors() {
    let temp_dir = tempfile::tempdir().unwrap();
    let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp_dir.path()).unwrap());
    let sqlite = DesktopSqliteState::new(ctx);
    let event_bus = Arc::new(AppEventBus::new(16));

    // Missing event_emitter should fail
    let res_missing_emitter = DesktopServices::builder().sqlite(sqlite.clone()).build();
    match res_missing_emitter {
        Err(err) => {
            assert!(
                err.contains("EventEmitterPort is required"),
                "Expected error message regarding missing EventEmitterPort, got: {err}"
            );
        }
        Ok(_) => panic!("Building DesktopServices without EventEmitterPort must fail"),
    }

    // Missing sqlite should fail
    let res_missing_sqlite = DesktopServices::builder().event_emitter(event_bus).build();
    match res_missing_sqlite {
        Err(err) => {
            assert!(
                err.contains("DesktopSqliteState is required"),
                "Expected error message regarding missing DesktopSqliteState, got: {err}"
            );
        }
        Ok(_) => panic!("Building DesktopServices without DesktopSqliteState must fail"),
    }
}

#[tokio::test]
async fn test_desktop_services_dynamic_hot_reload() {
    let temp1 = tempfile::tempdir().unwrap();
    let temp2 = tempfile::tempdir().unwrap();

    let ctx1 = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp1.path()).unwrap());
    let sqlite = DesktopSqliteState::new(ctx1);
    let event_bus = Arc::new(AppEventBus::new(32));

    let services = DesktopServices::builder()
        .sqlite(sqlite.clone())
        .event_emitter(event_bus)
        .sync_config_path(temp1.path().join("sync.json"))
        .build()
        .expect("DesktopServices build failed");

    // Create a project in context 1
    services
        .projects
        .create(ProjectCreateInput {
            name: "Context 1 Project".to_string(),
            description: None,
            icon: None,
            color: None,
            pipeline: None,
        })
        .await
        .unwrap();
    assert_eq!(services.projects.list().await.unwrap().len(), 1);

    // Dynamically hot-reload database context to context 2
    let ctx2 = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp2.path()).unwrap());
    sqlite.reload(ctx2).expect("sqlite reload failed");

    // The services instance should now seamlessly query context 2 without crashing
    assert_eq!(
        services.projects.list().await.unwrap().len(),
        0,
        "New context should be fresh and empty"
    );

    // Create a project in context 2
    services
        .projects
        .create(ProjectCreateInput {
            name: "Context 2 Project".to_string(),
            description: None,
            icon: None,
            color: None,
            pipeline: None,
        })
        .await
        .unwrap();
    assert_eq!(services.projects.list().await.unwrap().len(), 1);
}

#[tokio::test]
async fn test_desktop_services_concurrent_multithreaded_access() {
    let harness = Arc::new(TestHarness::new());
    let mut handles = Vec::new();

    for i in 0..10 {
        let h = harness.clone();
        handles.push(tokio::spawn(async move {
            h.services
                .projects
                .create(ProjectCreateInput {
                    name: format!("Concurrent Project {}", i),
                    description: Some("Created across threads".to_string()),
                    icon: None,
                    color: None,
                    pipeline: None,
                })
                .await
                .expect("Concurrent project creation failed");
        }));
    }

    for handle in handles {
        handle.await.expect("Task join failed");
    }

    let all_projects = harness.services.projects.list().await.unwrap();
    assert_eq!(
        all_projects.len(),
        10,
        "All 10 concurrent project creations should succeed"
    );
}

// ============================================================================
// Test 2: In-memory AppEventBus emit and receive verification without JSON/IPC overhead
// ============================================================================

#[tokio::test]
async fn test_app_event_bus_direct_emit_and_receive() {
    let bus = AppEventBus::new(64);
    let mut subscriber = bus.subscribe();

    // Verify subscriber count
    assert_eq!(bus.receiver_count(), 1);

    // Emit event
    let payload = json!({
        "status": "recording",
        "sample_rate": 16000,
        "channels": 1,
        "peak_level": 0.82
    });
    bus.emit("audio-state-changed", payload.clone())
        .expect("emit should succeed");

    // Receive event
    let received = subscriber
        .recv()
        .await
        .expect("subscriber should receive event promptly");

    assert_eq!(received.name, "audio-state-changed");
    assert_eq!(received.payload, payload);
}

#[tokio::test]
async fn test_app_event_bus_multi_subscriber_fanout() {
    let bus = AppEventBus::new(64);
    let mut sub1 = bus.subscribe();
    let mut sub2 = bus.subscribe();
    let mut sub3 = bus.subscribe();

    assert_eq!(bus.receiver_count(), 3);

    let event_payload = json!({
        "task_id": "task-uuid-1234",
        "progress": 75.5,
        "stage": "diarization"
    });

    bus.emit("task-progress", event_payload.clone()).unwrap();

    let r1 = sub1.recv().await.unwrap();
    let r2 = sub2.recv().await.unwrap();
    let r3 = sub3.recv().await.unwrap();

    assert_eq!(r1.payload, event_payload);
    assert_eq!(r2.payload, event_payload);
    assert_eq!(r3.payload, event_payload);
}

#[tokio::test]
async fn test_app_event_bus_fifo_order_preservation() {
    let bus = AppEventBus::new(128);
    let mut subscriber = bus.subscribe();

    for i in 0..50 {
        bus.emit("sequence-event", json!({ "index": i })).unwrap();
    }

    for i in 0..50 {
        let event = subscriber.recv().await.unwrap();
        assert_eq!(event.name, "sequence-event");
        assert_eq!(event.payload["index"], i);
    }
}

#[tokio::test]
async fn test_app_event_bus_payload_data_fidelity() {
    let bus = AppEventBus::new(32);
    let mut subscriber = bus.subscribe();

    let complex_payload = json!({
        "unicode_text": "Sona 语音转写 🎙️ 日本語 한국어",
        "null_val": null,
        "bool_val": true,
        "nested": {
            "tokens": [
                {"text": "Hello", "start": 0.0, "end": 0.5, "prob": 0.99},
                {"text": "World", "start": 0.5, "end": 1.0, "prob": 0.98}
            ]
        },
        "large_number": 9007199254740991i64
    });

    bus.emit("complex-event", complex_payload.clone()).unwrap();
    let received = subscriber.recv().await.unwrap();

    assert_eq!(received.payload, complex_payload);
    assert_eq!(
        received.payload["unicode_text"],
        "Sona 语音转写 🎙️ 日本語 한국어"
    );
    assert_eq!(received.payload["nested"]["tokens"][0]["text"], "Hello");
}

#[tokio::test]
async fn test_app_event_bus_emit_without_active_receivers_does_not_fail() {
    let bus = AppEventBus::new(16);
    assert_eq!(bus.receiver_count(), 0);

    // Emitting when there are zero subscribers must not error
    let res = bus.emit("orphan-event", json!({"info": "nobody listening"}));
    assert!(
        res.is_ok(),
        "Emitting without receivers should return Ok(())"
    );
}

// ============================================================================
// Test 3: Single-instance lock acquisition and duplicate detection semantics (Feature 6)
// ============================================================================

#[test]
fn test_single_instance_lock_acquisition_and_duplicate_detection() {
    let test_id = format!("sona_test_instance_{}", uuid::Uuid::new_v4());

    // 1. Primary instance acquires lock
    let instance_a = SingleInstanceGuard::acquire_named(&test_id)
        .expect("Lock acquisition query should succeed");
    assert!(
        instance_a.is_some(),
        "First instance must successfully acquire the exclusive lock"
    );
    let instance_a = instance_a.unwrap();
    assert!(
        instance_a.is_single(),
        "Acquired guard must report is_single=true"
    );

    // 2. Secondary duplicate instance attempts to acquire with the same identifier
    let instance_b = SingleInstanceGuard::acquire_named(&test_id)
        .expect("Duplicate lock acquisition query should succeed without OS error");
    assert!(
        instance_b.is_none(),
        "Duplicate instance must detect existing lock and return None"
    );

    // 3. Drop primary instance
    drop(instance_a);

    // 4. Third instance should now successfully acquire the released lock
    let instance_c = SingleInstanceGuard::acquire_named(&test_id)
        .expect("Subsequent lock acquisition query should succeed");
    assert!(
        instance_c.is_some(),
        "Subsequent instance must acquire lock after previous owner dropped it"
    );
    assert!(instance_c.unwrap().is_single());
}

#[test]
fn test_single_instance_lock_distinct_identifiers_do_not_conflict() {
    assert_eq!(SONA_SINGLE_INSTANCE_LOCK_NAME, "sona-desktop-lock");

    let id_a = format!("sona_test_a_{}", uuid::Uuid::new_v4());
    let id_b = format!("sona_test_b_{}", uuid::Uuid::new_v4());

    let lock_a =
        SingleInstanceGuard::acquire_named(&id_a).expect("Lock A acquisition should succeed");
    let lock_b =
        SingleInstanceGuard::acquire_named(&id_b).expect("Lock B acquisition should succeed");

    assert!(lock_a.is_some(), "Lock A should be acquired");
    assert!(lock_b.is_some(), "Lock B should be acquired");
    assert!(lock_a.unwrap().is_single());
    assert!(lock_b.unwrap().is_single());
}

#[test]
fn test_single_instance_rapid_acquire_release_cycles() {
    let test_id = format!("sona_rapid_test_{}", uuid::Uuid::new_v4());

    for _ in 0..10 {
        let lock = SingleInstanceGuard::acquire_named(&test_id)
            .expect("Rapid acquisition query should succeed");
        assert!(lock.is_some(), "Rapid acquisition should acquire lock");
        assert!(lock.unwrap().is_single());
    }
}

// ============================================================================
// Test 4: Auto-start integration & auto_launch production verification (Feature 7)
// ============================================================================

#[test]
fn test_auto_launch_production_constants_and_builder() {
    assert_eq!(APP_NAME, "Sona");
    assert_eq!(AUTO_START_ARG, "--autostart");

    let manager_res = auto_launch::build_auto_launch();
    assert!(
        manager_res.is_ok(),
        "build_auto_launch() should resolve current_exe cleanly: {:?}",
        manager_res.err()
    );
    let manager = manager_res.unwrap();
    assert_eq!(manager.get_app_name(), APP_NAME);
    let current_exe = std::env::current_exe().expect("Failed to get current executable path");
    assert_eq!(
        std::path::PathBuf::from(manager.get_app_path()),
        current_exe
    );
}

#[test]
fn test_auto_launch_system_status_queries() {
    let mock = Arc::new(MockAutoLaunch::new(APP_NAME, "sona.exe", &[AUTO_START_ARG]));
    auto_launch::set_mock_manager(Some(mock.clone()));

    let status_res = auto_launch::is_auto_start_enabled();
    assert!(
        status_res.is_ok(),
        "is_auto_start_enabled() should query cleanly without error: {:?}",
        status_res.err()
    );
    let status = status_res.unwrap();
    assert!(!status);

    let alias_res = auto_launch::is_auto_launch_enabled();
    assert!(
        alias_res.is_ok(),
        "is_auto_launch_enabled() alias should query without error: {:?}",
        alias_res.err()
    );
    assert_eq!(status, alias_res.unwrap());

    let payload_res = auto_launch::get_auto_launch_status();
    assert!(
        payload_res.is_ok(),
        "get_auto_launch_status() should return AutoLaunchStatus: {:?}",
        payload_res.err()
    );
    let payload = payload_res.unwrap();
    assert_eq!(payload.enabled, status);

    auto_launch::clear_mock_manager();
}

#[test]
fn test_auto_launch_status_struct_serialization_fidelity() {
    let enabled_status = AutoLaunchStatus { enabled: true };
    let json_val = serde_json::to_value(enabled_status).expect("Serialization failed");
    assert_eq!(json_val, json!({ "enabled": true }));

    let roundtrip: AutoLaunchStatus =
        serde_json::from_value(json_val).expect("Deserialization failed");
    assert_eq!(roundtrip, enabled_status);

    let disabled_status = AutoLaunchStatus { enabled: false };
    let json_disabled = serde_json::to_value(disabled_status).expect("Serialization failed");
    assert_eq!(json_disabled, json!({ "enabled": false }));
    let roundtrip_disabled: AutoLaunchStatus =
        serde_json::from_value(json_disabled).expect("Deserialization failed");
    assert_eq!(roundtrip_disabled, disabled_status);
}

#[test]
fn test_auto_launch_builder_with_spaced_executable_paths() {
    let path_with_spaces = r"C:\Program Files\Sona Desktop\sona.exe";
    let builder_res = AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(path_with_spaces)
        .set_args(&[AUTO_START_ARG, "--silent"])
        .build();

    assert!(
        builder_res.is_ok(),
        "AutoLaunchBuilder must support paths with spaces"
    );
    let manager = builder_res.unwrap();
    assert_eq!(manager.get_app_name(), APP_NAME);
    assert_eq!(manager.get_app_path(), path_with_spaces);
}

#[test]
fn test_auto_launch_builder_with_unicode_executable_paths() {
    let unicode_path = r"C:\用户\Sona 语音转写\bin\sona.exe";
    let builder_res = AutoLaunchBuilder::new()
        .set_app_name(APP_NAME)
        .set_app_path(unicode_path)
        .set_args(&[AUTO_START_ARG])
        .build();

    assert!(
        builder_res.is_ok(),
        "AutoLaunchBuilder must support unicode paths"
    );
    let manager = builder_res.unwrap();
    assert_eq!(manager.get_app_name(), APP_NAME);
    assert_eq!(manager.get_app_path(), unicode_path);
}

#[test]
fn test_auto_launch_isolated_enable_disable_lifecycle() {
    let test_app_name = format!("Sona_Tier1_Test_{}", uuid::Uuid::new_v4());
    let current_exe = std::env::current_exe().expect("Failed to get current executable path");
    let app_path = current_exe
        .to_str()
        .expect("Executable path must be valid UTF-8");

    let manager = MockAutoLaunch::new(&test_app_name, app_path, &[AUTO_START_ARG]);

    // 1. Initially must not be enabled
    assert!(
        !manager
            .is_enabled()
            .expect("Failed to query initial status"),
        "Fresh test key should not be enabled"
    );

    // 2. Enable
    let enable_res = manager.enable();
    assert!(
        enable_res.is_ok(),
        "Mock enable should succeed: {:?}",
        enable_res.err()
    );
    assert!(
        manager
            .is_enabled()
            .expect("Failed to query status after enable"),
        "Key must report enabled=true after enable"
    );

    // 3. Disable
    let disable_res = manager.disable();
    assert!(
        disable_res.is_ok(),
        "Mock disable should succeed: {:?}",
        disable_res.err()
    );
    assert!(
        !manager
            .is_enabled()
            .expect("Failed to query status after disable"),
        "Key must report enabled=false after disable"
    );
}

// ============================================================================
// Test 5: Direct core project service creation, retrieval, and listing assertions
// ============================================================================

#[tokio::test]
async fn test_project_service_create_retrieve_list_lifecycle() {
    let harness = TestHarness::new();
    let projects_service = &harness.services.projects;

    // 1. Initial listing must be empty
    let initial_list = projects_service.list().await.unwrap();
    assert!(initial_list.is_empty());

    // 2. Create first project
    let project_1 = projects_service
        .create(ProjectCreateInput {
            name: "Research Interviews 2026".to_string(),
            description: Some("Audio recordings from Q3 user interviews".to_string()),
            color: Some("#4F46E5".to_string()),
            icon: Some("folder-microphone".to_string()),
            pipeline: None,
        })
        .await
        .expect("Project creation failed");

    assert!(!project_1.id.is_empty());
    assert_eq!(project_1.name, "Research Interviews 2026");
    assert_eq!(
        project_1.description,
        "Audio recordings from Q3 user interviews"
    );
    assert_eq!(project_1.color.as_deref(), Some("#4F46E5"));
    assert_eq!(project_1.icon.as_deref(), Some("folder-microphone"));

    // 3. List contains newly created project
    let list_after_one = projects_service.list().await.unwrap();
    assert_eq!(list_after_one.len(), 1);
    assert_eq!(list_after_one[0].id, project_1.id);
    assert_eq!(list_after_one[0].name, "Research Interviews 2026");

    // 4. Create second project
    let project_2 = projects_service
        .create(ProjectCreateInput {
            name: "Keynote Speeches".to_string(),
            description: None,
            color: Some("#10B981".to_string()),
            icon: Some("presentation".to_string()),
            pipeline: None,
        })
        .await
        .expect("Second project creation failed");

    let list_after_two = projects_service.list().await.unwrap();
    assert_eq!(list_after_two.len(), 2);
    let ids: Vec<&str> = list_after_two.iter().map(|p| p.id.as_str()).collect();
    assert!(ids.contains(&project_1.id.as_str()));
    assert!(ids.contains(&project_2.id.as_str()));

    // 5. Update project 1 metadata
    let updated = projects_service
        .update(
            project_1.id.clone(),
            ProjectUpdateInput {
                name: Some("Renamed Interviews Project".to_string()),
                description: Some("Updated description".to_string()),
                color: Some("#F59E0B".to_string()),
                icon: Some("archive".to_string()),
                pipeline: None,
            },
        )
        .await
        .expect("Update failed");

    assert!(updated.is_some());
    let updated_record = updated.unwrap();
    assert_eq!(updated_record.name, "Renamed Interviews Project");
    assert_eq!(updated_record.color.as_deref(), Some("#F59E0B"));

    // 6. Delete project 2
    projects_service
        .delete(project_2.id.clone())
        .await
        .expect("Delete failed");

    let list_after_delete = projects_service.list().await.unwrap();
    assert_eq!(list_after_delete.len(), 1);
    assert_eq!(list_after_delete[0].id, project_1.id);
    assert_eq!(list_after_delete[0].name, "Renamed Interviews Project");
}

#[tokio::test]
async fn test_project_service_reorder_semantics() {
    let harness = TestHarness::new();
    let projects_service = &harness.services.projects;

    let p1 = projects_service
        .create(ProjectCreateInput {
            name: "Project A".to_string(),
            description: None,
            color: None,
            icon: None,
            pipeline: None,
        })
        .await
        .unwrap();

    let p2 = projects_service
        .create(ProjectCreateInput {
            name: "Project B".to_string(),
            description: None,
            color: None,
            icon: None,
            pipeline: None,
        })
        .await
        .unwrap();

    let p3 = projects_service
        .create(ProjectCreateInput {
            name: "Project C".to_string(),
            description: None,
            color: None,
            icon: None,
            pipeline: None,
        })
        .await
        .unwrap();

    // Reorder as [C, A, B]
    let reordered = projects_service
        .reorder(vec![p3.id.clone(), p1.id.clone(), p2.id.clone()])
        .await
        .expect("Reorder failed");

    assert_eq!(reordered.len(), 3);
    assert_eq!(reordered[0].id, p3.id);
    assert_eq!(reordered[1].id, p1.id);
    assert_eq!(reordered[2].id, p2.id);

    // Verify persisted order via list
    let list = projects_service.list().await.unwrap();
    assert_eq!(list[0].id, p3.id);
    assert_eq!(list[1].id, p1.id);
    assert_eq!(list[2].id, p2.id);
}

#[tokio::test]
async fn test_project_service_update_nonexistent_returns_none() {
    let harness = TestHarness::new();
    let non_existent_id = uuid::Uuid::new_v4().to_string();

    let result = harness
        .services
        .projects
        .update(
            non_existent_id,
            ProjectUpdateInput {
                name: Some("Non-existent".to_string()),
                description: None,
                color: None,
                icon: None,
                pipeline: None,
            },
        )
        .await
        .expect("Call should complete without fatal error");

    assert!(
        result.is_none(),
        "Updating non-existent project should return None"
    );
}
