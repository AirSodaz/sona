//! Empirical Adversarial Stress Test Suite for Milestone 1 (Features F1–F8)
//!
//! Stress tests:
//! 1. AppEventBus:
//!    - Concurrent multi-publishers (50 publishers x 100 events = 5,000 events)
//!    - High subscriber fanout (500 concurrent subscribers)
//!    - Lagged subscriber handling and channel overflow recovery
//!    - Empty, null, and special payloads (Null, "", {}, [], extremes)
//!    - Large JSON payloads (5MB transcript payload with 5,000 segments)
//! 2. SingleInstanceGuard:
//!    - Cross-thread mutex contention across 20 OS threads
//!    - 1,000 rapid acquire/release cycles
//!    - Divergent lock name vulnerability (sona-desktop-lock vs com.asoda.sona.instance_lock)
//! 3. Decoupled Core Direct Linking:
//!    - Concurrent multithreaded service calls across projects, config, task_ledger, and emitter
//!    - GlobalDesktopServices & GlobalEventBus thread-safety and Deref integrity

mod harness;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use harness::TestHarness;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sona_core::ports::event::EventEmitterPort;
use sona_desktop_lib::app::{GlobalDesktopServices, GlobalEventBus};
use sona_desktop_lib::event_bus::AppEventBus;
use sona_desktop_lib::platform::single_instance::{
    SingleInstanceGuard, SONA_SINGLE_INSTANCE_LOCK_NAME,
};
use tokio::sync::broadcast::error::RecvError;

// ============================================================================
// 1. AppEventBus Concurrency, Scalability & Stress Tests
// ============================================================================

#[tokio::test]
async fn test_app_event_bus_concurrent_multi_publisher_stress() {
    let bus = Arc::new(AppEventBus::new(8192));
    let mut rx1 = bus.subscribe();
    let mut rx2 = bus.subscribe();

    assert_eq!(bus.receiver_count(), 2);

    let num_publishers = 50;
    let events_per_publisher = 100;
    let total_events = num_publishers * events_per_publisher;

    let rx1_count = Arc::new(AtomicUsize::new(0));
    let rx2_count = Arc::new(AtomicUsize::new(0));

    let c1 = rx1_count.clone();
    let handle1 = tokio::spawn(async move {
        while let Ok(_event) = rx1.recv().await {
            let count = c1.fetch_add(1, Ordering::SeqCst) + 1;
            if count >= total_events {
                break;
            }
        }
    });

    let c2 = rx2_count.clone();
    let handle2 = tokio::spawn(async move {
        while let Ok(_event) = rx2.recv().await {
            let count = c2.fetch_add(1, Ordering::SeqCst) + 1;
            if count >= total_events {
                break;
            }
        }
    });

    let mut pub_handles = Vec::with_capacity(num_publishers);
    for pub_idx in 0..num_publishers {
        let bus_clone = bus.clone();
        pub_handles.push(tokio::spawn(async move {
            for seq in 0..events_per_publisher {
                let payload = json!({
                    "publisher": pub_idx,
                    "seq": seq,
                    "timestamp": 1700000000 + seq
                });
                bus_clone.emit("stress:metric", payload).unwrap();
            }
        }));
    }

    for h in pub_handles {
        h.await.unwrap();
    }

    let timeout = Duration::from_secs(5);
    let _ = tokio::time::timeout(timeout, async {
        let _ = tokio::join!(handle1, handle2);
    })
    .await;

    assert_eq!(
        rx1_count.load(Ordering::SeqCst),
        total_events,
        "Subscriber 1 must receive all 5,000 events without loss"
    );
    assert_eq!(
        rx2_count.load(Ordering::SeqCst),
        total_events,
        "Subscriber 2 must receive all 5,000 events without loss"
    );
}

#[tokio::test]
async fn test_app_event_bus_high_subscriber_fanout_500() {
    let bus = Arc::new(AppEventBus::new(1024));
    let num_subscribers = 500;
    let mut receivers = Vec::with_capacity(num_subscribers);

    for _ in 0..num_subscribers {
        receivers.push(bus.subscribe());
    }

    assert_eq!(
        bus.receiver_count(),
        num_subscribers,
        "Bus must reflect 500 active receivers"
    );

    let received_count = Arc::new(AtomicUsize::new(0));
    let mut sub_handles = Vec::with_capacity(num_subscribers);

    for mut rx in receivers {
        let counter = received_count.clone();
        sub_handles.push(tokio::spawn(async move {
            if let Ok(event) = rx.recv().await {
                if event.name == "fanout:broadcast" && event.payload["round"] == 42 {
                    counter.fetch_add(1, Ordering::SeqCst);
                }
            }
        }));
    }

    // Small yield to let tasks enter recv()
    tokio::task::yield_now().await;

    bus.emit("fanout:broadcast", json!({ "round": 42 })).unwrap();

    let timeout = Duration::from_secs(5);
    let _ = tokio::time::timeout(timeout, async {
        for h in sub_handles {
            let _ = h.await;
        }
    })
    .await;

    assert_eq!(
        received_count.load(Ordering::SeqCst),
        num_subscribers,
        "All 500 subscribers must receive broadcast event simultaneously"
    );
}

#[tokio::test]
async fn test_app_event_bus_lagged_subscriber_handling_and_recovery() {
    // Channel buffer capacity 16 to trigger Lagged error under burst
    let bus = AppEventBus::new(16);
    let mut slow_rx = bus.subscribe();

    // Emit 64 events while slow subscriber is not polling
    for i in 0..64 {
        bus.emit("fast:burst", json!({ "index": i })).unwrap();
    }

    // Slow subscriber now polls
    let first_recv = slow_rx.recv().await;
    match first_recv {
        Err(RecvError::Lagged(skipped)) => {
            println!("Confirmed RecvError::Lagged with {skipped} dropped events");
            assert!(
                skipped >= 48,
                "Expected at least 48 skipped events from buffer overflow, got {skipped}"
            );
        }
        other => panic!("Expected RecvError::Lagged, got {:?}", other),
    }

    // Next receive resumes delivery from the oldest unexpired message (index 48)
    let second_recv = slow_rx.recv().await;
    assert!(second_recv.is_ok(), "Subscriber should recover after handling Lagged");
    let event = second_recv.unwrap();
    assert_eq!(event.payload["index"], 48);

    // Emit a fresh event after recovery
    bus.emit("fresh:event", json!({ "msg": "ok" })).unwrap();

    // Drain remaining buffered fast:burst events until fresh:event arrives
    let mut found_fresh = false;
    while let Ok(msg) = slow_rx.recv().await {
        if msg.name == "fresh:event" {
            assert_eq!(msg.payload["msg"], "ok");
            found_fresh = true;
            break;
        }
    }
    assert!(found_fresh, "Subscriber must eventually receive fresh:event after draining backlog");
}

#[tokio::test]
async fn test_app_event_bus_empty_and_special_payloads() {
    let bus = AppEventBus::default();
    let mut rx = bus.subscribe();

    // 1. Null payload
    bus.emit("test:null", serde_json::Value::Null).unwrap();
    let e = rx.recv().await.unwrap();
    assert!(e.payload.is_null());

    // 2. Empty string payload
    bus.emit("test:empty_string", json!("")).unwrap();
    let e = rx.recv().await.unwrap();
    assert_eq!(e.payload.as_str(), Some(""));

    // 3. Empty object payload
    bus.emit("test:empty_object", json!({})).unwrap();
    let e = rx.recv().await.unwrap();
    assert_eq!(e.payload, json!({}));

    // 4. Empty array payload
    bus.emit("test:empty_array", json!([])).unwrap();
    let e = rx.recv().await.unwrap();
    assert_eq!(e.payload, json!([]));

    // 5. Special Unicode and emoji names and payload
    let complex_name = "event:中文:🚀:special_chars_&%$#@!";
    let complex_payload = json!({
        "emoji": "🎉🚀💻",
        "nested": {
            "empty_map": {},
            "null_field": null,
            "bool_flag": false,
            "negative_int": -999999999
        }
    });
    bus.emit(complex_name, complex_payload.clone()).unwrap();
    let e = rx.recv().await.unwrap();
    assert_eq!(e.name, complex_name);
    assert_eq!(e.payload, complex_payload);
}

#[tokio::test]
async fn test_app_event_bus_large_json_payload_stress() {
    #[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
    struct MockSegment {
        id: usize,
        start_ms: u64,
        end_ms: u64,
        text: String,
        tokens: Vec<u32>,
        speaker: String,
    }

    let bus = AppEventBus::default();
    let mut rx = bus.subscribe();

    // Generate 5,000 transcript segments (~2MB to 3MB of JSON)
    let num_segments = 5000;
    let mut segments = Vec::with_capacity(num_segments);
    for i in 0..num_segments {
        segments.push(MockSegment {
            id: i,
            start_ms: (i as u64) * 1500,
            end_ms: (i as u64) * 1500 + 1400,
            text: format!("Segment {i}: Real-time speech transcription stream segment data with confidence metrics"),
            tokens: (0..15).map(|t| (i * 100 + t) as u32).collect(),
            speaker: format!("Speaker_{}", i % 4),
        });
    }

    let start_emit = std::time::Instant::now();
    bus.emit_typed("transcript:full_dump", &segments).unwrap();
    let emit_elapsed = start_emit.elapsed();

    let start_recv = std::time::Instant::now();
    let event = rx.recv().await.expect("Failed to receive large event");
    let recv_elapsed = start_recv.elapsed();

    let start_decode = std::time::Instant::now();
    let decoded: Vec<MockSegment> = event.decode().expect("Failed to decode large payload");
    let decode_elapsed = start_decode.elapsed();

    println!(
        "Large payload ({} items): emit={:?}, recv={:?}, decode={:?}",
        num_segments, emit_elapsed, recv_elapsed, decode_elapsed
    );

    assert_eq!(decoded.len(), num_segments);
    assert_eq!(decoded[0], segments[0]);
    assert_eq!(decoded[num_segments - 1], segments[num_segments - 1]);
}

// ============================================================================
// 2. SingleInstanceGuard Concurrency, Contention & Identification Tests
// ============================================================================

#[test]
fn test_single_instance_guard_cross_thread_contention() {
    let lock_id = format!("sona-contention-test-{}", uuid::Uuid::new_v4());
    let thread_count = 20;

    let successful_acquisitions = Arc::new(AtomicUsize::new(0));
    let rejected_acquisitions = Arc::new(AtomicUsize::new(0));

    let barrier = Arc::new(std::sync::Barrier::new(thread_count));
    let mut handles = Vec::with_capacity(thread_count);

    for _ in 0..thread_count {
        let lock_name = lock_id.clone();
        let success_ctr = successful_acquisitions.clone();
        let reject_ctr = rejected_acquisitions.clone();
        let b = barrier.clone();

        handles.push(std::thread::spawn(move || {
            b.wait(); // Synchronize all threads to fire simultaneously
            let guard = SingleInstanceGuard::acquire_named(&lock_name)
                .expect("SingleInstanceGuard::acquire_named should not return Err");
            if guard.is_some() {
                success_ctr.fetch_add(1, Ordering::SeqCst);
                // Hold briefly to simulate application startup
                std::thread::sleep(Duration::from_millis(50));
            } else {
                reject_ctr.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert_eq!(
        successful_acquisitions.load(Ordering::SeqCst),
        1,
        "Exactly one thread must acquire the single-instance lock"
    );
    assert_eq!(
        rejected_acquisitions.load(Ordering::SeqCst),
        thread_count - 1,
        "All other threads must be rejected"
    );

    // After all threads terminate and guards are dropped, re-acquisition must succeed
    let re_acquire = SingleInstanceGuard::acquire_named(&lock_id).unwrap();
    assert!(
        re_acquire.is_some(),
        "Lock must be re-acquirable once previous owner dropped it"
    );
}

#[test]
fn test_single_instance_rapid_acquire_release_1000_cycles() {
    let lock_id = format!("sona-rapid-cycles-{}", uuid::Uuid::new_v4());

    for i in 0..1000 {
        let guard = SingleInstanceGuard::acquire_named(&lock_id)
            .unwrap_or_else(|e| panic!("Cycle {i} failed: {e}"));
        assert!(guard.is_some(), "Cycle {i} must acquire lock");
        drop(guard);
    }
}

#[test]
fn test_single_instance_lock_name_synchronization_and_focus_handling() {
    // Proving the vulnerability has been ERADICATED:
    // Both lib.rs and SingleInstanceGuard::acquire() now share the canonical lock
    // SONA_SINGLE_INSTANCE_LOCK_NAME ("sona-desktop-lock").
    assert_eq!(SONA_SINGLE_INSTANCE_LOCK_NAME, "sona-desktop-lock");

    // 1. Primary instance acquires lock
    let primary_guard = SingleInstanceGuard::acquire()
        .expect("Primary instance should acquire SingleInstanceGuard cleanly");
    assert!(
        primary_guard.is_some(),
        "Primary instance must successfully hold the exclusive lock"
    );
    let primary_guard = primary_guard.unwrap();
    assert!(primary_guard.is_single());

    // 2. Secondary duplicate instance in lib.rs invokes SingleInstanceGuard::acquire()
    let secondary_attempt = SingleInstanceGuard::acquire()
        .expect("Secondary instance lock query should succeed without OS error");
    assert!(
        secondary_attempt.is_none(),
        "Secondary instance must detect existing lock and return None (blocking duplicate launch)"
    );

    // 3. Duplicate handling path in lib.rs executes focus_existing_window("Sona")
    // This must execute cleanly without panic in both headless and GUI environments
    let focused = sona_desktop_lib::platform::single_instance::focus_existing_window("Sona");
    println!("focus_existing_window('Sona') executed cleanly, result: {focused}");

    // 4. Dropping primary instance releases OS mutex
    drop(primary_guard);

    // 5. Subsequent instance can now acquire lock
    let third_attempt = SingleInstanceGuard::acquire()
        .expect("Subsequent acquisition query should succeed");
    assert!(
        third_attempt.is_some(),
        "Subsequent instance must successfully acquire lock after primary dropped"
    );
}

// ============================================================================
// 3. Decoupled Core Direct Linking Multithreaded Calling Tests
// ============================================================================

#[tokio::test]
async fn test_decoupled_core_concurrent_multithreaded_services() {
    let harness = TestHarness::new();
    let services = Arc::new(harness.services);
    let event_bus = harness.event_bus.clone();

    let mut handles = Vec::new();

    // 1. Projects tasks (creating and listing)
    for p_idx in 0..5 {
        let s = services.clone();
        handles.push(tokio::spawn(async move {
            let proj_name = format!("Stress Project {p_idx}");
            let created = s
                .projects
                .create(sona_core::project::ProjectCreateInput {
                    name: proj_name.clone(),
                    description: None,
                    icon: None,
                    color: None,
                    pipeline: None,
                })
                .await
                .unwrap();
            assert_eq!(created.name, proj_name);

            let list = s.projects.list().await.unwrap();
            assert!(list.iter().any(|p| p.id == created.id));
        }));
    }

    // 2. Config tasks (saving and loading)
    for c_idx in 0..5 {
        let s = services.clone();
        handles.push(tokio::spawn(async move {
            let cfg = json!({
                "theme": if c_idx % 2 == 0 { "dark" } else { "light" },
                "iteration": c_idx
            });
            s.config.save(&cfg).unwrap();
            let loaded = s.config.load().unwrap();
            assert!(loaded.is_some());
        }));
    }

    // 3. Task ledger tasks (upserting and listing)
    for t_idx in 0..5 {
        let s = services.clone();
        handles.push(tokio::spawn(async move {
            let record = sona_core::task_ledger::types::TaskLedgerRecord {
                id: format!("stress-task-{t_idx}"),
                kind: sona_core::task_ledger::types::TaskLedgerKind::BatchImport,
                status: sona_core::task_ledger::types::TaskLedgerStatus::Pending,
                title: format!("Stress Task {t_idx}"),
                progress: (t_idx as f64) * 10.0,
                created_at: 1700000000,
                updated_at: 1700000000,
                retryable: false,
                cancelable: true,
                recoverable: false,
                stage: None,
                history_id: None,
                tag_ids: Vec::new(),
                file_path: None,
                automation_rule_id: None,
                tag_automation_rule_id: None,
                automation_profile_id: None,
                automation_profile_source: None,
                source_fingerprint: None,
                error_message: None,
                template_id: None,
                target_language: None,
            };
            s.task_ledger.upsert_task(record).await.unwrap();
            let tasks = s.task_ledger.list_tasks().await.unwrap();
            assert!(tasks.iter().any(|t| t.id == format!("stress-task-{t_idx}")));
        }));
    }

    // 4. Concurrent Event streaming
    let mut rx = event_bus.subscribe();
    let events_received = Arc::new(AtomicUsize::new(0));
    let er_clone = events_received.clone();
    let stream_handle = tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if event.name == "stress:core_ping" {
                if er_clone.fetch_add(1, Ordering::SeqCst) + 1 >= 5 {
                    break;
                }
            }
        }
    });

    for e_idx in 0..5 {
        let bus = event_bus.clone();
        handles.push(tokio::spawn(async move {
            bus.emit("stress:core_ping", json!({ "seq": e_idx })).unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    let _ = tokio::time::timeout(Duration::from_secs(3), stream_handle).await;
    assert_eq!(
        events_received.load(Ordering::SeqCst),
        5,
        "All core event bus pings must be received concurrently"
    );
}

#[tokio::test]
async fn test_global_desktop_services_and_event_bus_deref() {
    let harness = TestHarness::new();
    let services = Arc::new(harness.services);
    let event_bus = Arc::new(AppEventBus::default());

    let global_services = GlobalDesktopServices(Arc::clone(&services));
    let global_bus = GlobalEventBus(Arc::clone(&event_bus));

    // Verify deref to DesktopServices
    assert!(global_services.projects.list().await.is_ok());

    // Verify deref to AppEventBus
    assert_eq!(global_bus.receiver_count(), 0);
    let _rx = global_bus.subscribe();
    assert_eq!(global_bus.receiver_count(), 1);
}
