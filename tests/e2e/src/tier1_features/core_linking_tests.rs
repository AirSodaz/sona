//! Tier 1: Core Linking Tests (Features 4, 5, and Core Services)

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use serde_json::json;

    use crate::harness::{AppEventBus, TestHarness};
    use sona_core::ports::event::EventEmitterPort;
    use sona_core::project::{ProjectCreateInput, ProjectUpdateInput};
    use sona_desktop_lib::platform::database::DesktopSqliteState;
    use sona_desktop_lib::services::DesktopServices;

    #[tokio::test]
    async fn test_desktop_services_composition_without_tauri() {
        let harness = TestHarness::new();
        let services = &harness.services;

        let projects = services.projects.list().await.expect("projects.list() should succeed");
        assert_eq!(projects.len(), 0);

        let config = services.config.load().expect("config.load() should succeed");
        assert_eq!(config, None);

        let sync_status = services.sync.get_status().await.expect("sync.get_status() should succeed");
        assert_eq!(
            sync_status.state,
            sona_core::sync::SyncLifecycleState::Disabled
        );

        let ledger_snapshot = services
            .task_ledger
            .load_snapshot()
            .await
            .expect("task_ledger.load_snapshot() should succeed");
        assert_eq!(ledger_snapshot.tasks.len(), 0);

        let history_items = services
            .history
            .query_db(|s| s.list_items(Default::default()))
            .await
            .expect("history.query_db() should succeed");
        assert_eq!(history_items.len(), 0);

        assert!(services.recording_session.lock().await.is_none());
        assert!(Arc::strong_count(&services.downloads) >= 1);
    }

    #[tokio::test]
    async fn test_desktop_services_builder_validation_errors() {
        let temp_dir = tempfile::tempdir().unwrap();
        let ctx = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp_dir.path()).unwrap());
        let sqlite = DesktopSqliteState::new(ctx);
        let event_bus = Arc::new(AppEventBus::new(16));

        let res_missing_emitter = DesktopServices::builder()
            .sqlite(sqlite.clone())
            .build();
        match res_missing_emitter {
            Err(err) => {
                assert!(err.contains("EventEmitterPort is required"));
            }
            Ok(_) => panic!("Building without EventEmitterPort must fail"),
        }

        let res_missing_sqlite = DesktopServices::builder()
            .event_emitter(event_bus)
            .build();
        match res_missing_sqlite {
            Err(err) => {
                assert!(err.contains("DesktopSqliteState is required"));
            }
            Ok(_) => panic!("Building without DesktopSqliteState must fail"),
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

        let ctx2 = Arc::new(sona_sqlite::SqliteApplicationContext::open(temp2.path()).unwrap());
        sqlite.reload(ctx2).expect("sqlite reload failed");

        assert_eq!(services.projects.list().await.unwrap().len(), 0);

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
        assert_eq!(all_projects.len(), 10);
    }

    #[tokio::test]
    async fn test_app_event_bus_direct_emit_and_receive() {
        let bus = AppEventBus::new(64);
        let mut subscriber = bus.subscribe();

        assert_eq!(bus.receiver_count(), 1);

        let payload = json!({
            "status": "recording",
            "sample_rate": 16000,
            "channels": 1,
            "peak_level": 0.82
        });
        bus.emit("audio-state-changed", payload.clone()).unwrap();

        let received = subscriber.recv().await.unwrap();
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

        assert_eq!(sub1.recv().await.unwrap().payload, event_payload);
        assert_eq!(sub2.recv().await.unwrap().payload, event_payload);
        assert_eq!(sub3.recv().await.unwrap().payload, event_payload);
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

        let res = bus.emit("orphan-event", json!({"info": "nobody listening"}));
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_project_service_create_retrieve_list_lifecycle() {
        let harness = TestHarness::new();
        let projects_service = &harness.services.projects;

        let initial_list = projects_service.list().await.unwrap();
        assert!(initial_list.is_empty());

        let project_1 = projects_service
            .create(ProjectCreateInput {
                name: "Research Interviews 2026".to_string(),
                description: Some("Audio recordings from Q3 user interviews".to_string()),
                color: Some("#4F46E5".to_string()),
                icon: Some("folder-microphone".to_string()),
                pipeline: None,
            })
            .await
            .unwrap();

        assert!(!project_1.id.is_empty());
        assert_eq!(project_1.name, "Research Interviews 2026");
        assert_eq!(
            project_1.description,
            "Audio recordings from Q3 user interviews"
        );
        assert_eq!(project_1.color.as_deref(), Some("#4F46E5"));
        assert_eq!(project_1.icon.as_deref(), Some("folder-microphone"));

        let list_after_one = projects_service.list().await.unwrap();
        assert_eq!(list_after_one.len(), 1);
        assert_eq!(list_after_one[0].id, project_1.id);

        let project_2 = projects_service
            .create(ProjectCreateInput {
                name: "Keynote Speeches".to_string(),
                description: None,
                color: Some("#10B981".to_string()),
                icon: Some("presentation".to_string()),
                pipeline: None,
            })
            .await
            .unwrap();

        let list_after_two = projects_service.list().await.unwrap();
        assert_eq!(list_after_two.len(), 2);

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
            .unwrap();

        assert!(updated.is_some());
        let updated_record = updated.unwrap();
        assert_eq!(updated_record.name, "Renamed Interviews Project");

        projects_service
            .delete(project_2.id.clone())
            .await
            .unwrap();

        let list_after_delete = projects_service.list().await.unwrap();
        assert_eq!(list_after_delete.len(), 1);
        assert_eq!(list_after_delete[0].id, project_1.id);
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

        let reordered = projects_service
            .reorder(vec![p3.id.clone(), p1.id.clone(), p2.id.clone()])
            .await
            .unwrap();

        assert_eq!(reordered.len(), 3);
        assert_eq!(reordered[0].id, p3.id);
        assert_eq!(reordered[1].id, p1.id);
        assert_eq!(reordered[2].id, p2.id);

        let list = projects_service.list().await.unwrap();
        assert_eq!(list[0].id, p3.id);
        assert_eq!(list[1].id, p1.id);
        assert_eq!(list[2].id, p2.id);
    }
}
