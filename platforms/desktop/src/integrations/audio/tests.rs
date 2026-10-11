use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::channel;

use sona_application::live_transcription::LiveSourceEpoch;

use super::AudioState;
use super::coordinator::wait_for_device_teardown;
use super::recorder::{RecorderCommand, queue_recording_start, resolve_recording_output_path};
use super::registry::{CaptureRegistry, SharedCaptureState};
use super::types::{AudioStopSignal, CaptureKey, CaptureKind};

fn running_capture(
    owner: &str,
    source_generation: u64,
) -> (
    SharedCaptureState,
    std::sync::mpsc::Receiver<()>,
    tokio::sync::mpsc::Receiver<RecorderCommand>,
) {
    let (stop_tx, stop_rx) = channel::<()>();
    let (recorder_tx, recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(4);
    let mut capture = SharedCaptureState::default();
    capture.commit_start_with_source(
        owner.to_string(),
        true,
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
        LiveSourceEpoch::new(
            format!("desktop-source-{source_generation}"),
            source_generation,
        ),
        Arc::new(AtomicU64::new(0)),
    );
    (capture, stop_rx, recorder_rx)
}

#[test]
fn stopping_marker_is_cleared_only_by_matching_source_epoch() {
    let key = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "test-device".to_string(),
    };
    let old_source = LiveSourceEpoch::new("source-old", 1);
    let new_source = LiveSourceEpoch::new("source-new", 2);
    let mut registry = CaptureRegistry::default();
    let _rx = registry.mark_stopping(key.clone(), new_source.clone());

    assert!(!registry.clear_stopping(&key, &old_source));
    assert!(registry.stopping_keys.contains_key(&key));
    assert!(registry.clear_stopping(&key, &new_source));
    assert!(!registry.stopping_keys.contains_key(&key));
}

#[test]
fn shared_capture_state_only_becomes_running_after_commit() {
    let mut capture = SharedCaptureState::default();
    assert!(!capture.is_running());
    assert!(capture.owners().is_empty());
    assert!(capture.active_instances().is_empty());
    assert!(capture.recorder_tx.is_none());

    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    let owners = capture.commit_start(
        "record".to_string(),
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
    );

    assert!(capture.is_running());
    assert_eq!(owners, vec!["record".to_string()]);
    assert_eq!(capture.active_instances(), vec!["record".to_string()]);
    assert!(capture.recorder_tx.is_some());
}

#[test]
fn shared_capture_state_failed_start_leaves_no_runtime_state() {
    let capture = SharedCaptureState::default();

    assert!(!capture.is_running());
    assert!(capture.owners().is_empty());
    assert!(capture.active_instances().is_empty());
    assert!(capture.recorder_tx.is_none());
}

#[test]
fn shared_capture_state_attach_adds_owner_to_running_capture() {
    let mut capture = SharedCaptureState::default();
    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    capture.commit_start(
        "voice-typing".to_string(),
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
    );

    let owners = capture.attach_instance("record".to_string(), true);

    assert_eq!(
        owners,
        vec!["record".to_string(), "voice-typing".to_string()]
    );
    assert_eq!(
        capture.active_instances(),
        vec!["record".to_string(), "voice-typing".to_string()]
    );
    assert!(capture.is_running());
}

#[test]
fn shared_capture_state_detach_last_owner_clears_runtime_state() {
    let mut capture = SharedCaptureState::default();
    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    capture.commit_start(
        "record".to_string(),
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
    );

    let detach_result = capture.detach_instance("record");

    assert!(detach_result.should_stop_hardware);
    assert!(detach_result.stop_signal.is_some());
    assert!(detach_result.recorder_tx.is_some());
    assert!(!capture.is_running());
    assert!(capture.owners().is_empty());
    assert!(capture.active_instances().is_empty());
    assert!(capture.recorder_tx.is_none());
}

#[test]
fn shared_capture_state_pause_filters_active_instances_without_detaching_owner() {
    let mut capture = SharedCaptureState::default();
    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    capture.commit_start(
        "voice-typing".to_string(),
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
    );
    capture.attach_instance("record".to_string(), true);

    let active_instances = capture.set_instance_paused("record", true).unwrap();

    assert_eq!(
        capture.owners(),
        vec!["record".to_string(), "voice-typing".to_string()]
    );
    assert_eq!(active_instances, vec!["voice-typing".to_string()]);
    assert_eq!(capture.active_instances(), vec!["voice-typing".to_string()]);
}

#[test]
fn shared_capture_state_detaches_preview_without_stopping_persistent_owner() {
    let mut capture = SharedCaptureState::default();
    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    capture.commit_start(
        "voice-typing".to_string(),
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
    );
    capture.attach_instance("test_mic".to_string(), false);

    let preview_detach = capture.detach_instance("test_mic");

    assert!(!preview_detach.should_stop_hardware);
    assert!(preview_detach.stop_signal.is_none());
    assert_eq!(capture.owners(), vec!["voice-typing".to_string()]);
    assert!(capture.is_running());

    let persistent_detach = capture.detach_instance("voice-typing");

    assert!(persistent_detach.should_stop_hardware);
    assert!(persistent_detach.stop_signal.is_some());
    assert!(!capture.is_running());
}

#[test]
fn shared_capture_state_detach_clears_paused_instance_state() {
    let mut capture = SharedCaptureState::default();
    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    capture.commit_start(
        "record".to_string(),
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
    );
    capture.set_instance_paused("record", true).unwrap();

    let detach_result = capture.detach_instance("record");

    assert!(detach_result.should_stop_hardware);
    assert!(capture.paused_instances.is_empty());
    assert!(capture.active_instances().is_empty());
}

#[test]
fn capture_registry_shares_same_device_and_keeps_different_devices_independent() {
    let microphone_a = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "device-a".to_string(),
    };
    let microphone_b = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "device-b".to_string(),
    };
    let (capture_a, _stop_a, _recorder_a) = running_capture("record", 11);
    let (capture_b, _stop_b, _recorder_b) = running_capture("voice-typing", 12);
    let mut registry = CaptureRegistry::default();
    registry.captures.insert(microphone_a.clone(), capture_a);
    registry.captures.insert(microphone_b.clone(), capture_b);
    registry.instance_keys.insert(
        (CaptureKind::Microphone, "record".to_string()),
        microphone_a.clone(),
    );
    registry.instance_keys.insert(
        (CaptureKind::Microphone, "voice-typing".to_string()),
        microphone_b.clone(),
    );

    let (shared_lease, _) = registry
        .attach_running(&microphone_a, CaptureKind::Microphone, "caption", false)
        .unwrap()
        .unwrap();

    assert_eq!(registry.captures.len(), 2);
    assert_eq!(shared_lease.source_generation, 11);
    assert_eq!(
        registry.captures[&microphone_a].owners(),
        vec!["caption".to_string(), "record".to_string()]
    );
    assert_eq!(
        registry.captures[&microphone_b].owners(),
        vec!["voice-typing".to_string()]
    );
}

#[test]
fn capture_registry_rollback_detaches_only_target_and_stops_last_owner() {
    let key = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "device-a".to_string(),
    };
    let (mut capture, stop_rx, _recorder_rx) = running_capture("record", 13);
    capture.attach_instance("caption".to_string(), false);
    let mut registry = CaptureRegistry::default();
    registry.captures.insert(key.clone(), capture);
    for owner in ["record", "caption"] {
        registry
            .instance_keys
            .insert((CaptureKind::Microphone, owner.to_string()), key.clone());
    }

    assert!(
        registry
            .rollback_attachment(CaptureKind::Microphone, "caption", &key)
            .is_none()
    );
    assert_eq!(registry.captures[&key].owners(), vec!["record".to_string()]);
    let stop_signal = registry
        .rollback_attachment(CaptureKind::Microphone, "record", &key)
        .unwrap();
    stop_signal.stop();

    assert!(registry.captures.is_empty());
    assert!(registry.instance_keys.is_empty());
    assert!(stop_rx.recv().is_ok());
}

#[tokio::test]
async fn recording_start_propagates_writer_creation_failure() {
    let (recorder_tx, mut recorder_rx) = tokio::sync::mpsc::channel(1);
    let worker = tokio::spawn(async move {
        let RecorderCommand::Start { completed, .. } = recorder_rx.recv().await.unwrap() else {
            panic!("expected recorder start command");
        };
        completed
            .send(Err("writer creation failed".to_string()))
            .unwrap();
    });

    let error = queue_recording_start(
        Some(&recorder_tx),
        true,
        "Microphone",
        "record",
        Some("recording.wav".to_string()),
        || Err("fallback must not run".to_string()),
    )
    .await
    .unwrap_err();
    worker.await.unwrap();

    assert_eq!(error, "writer creation failed");
}

#[test]
fn resolve_recording_output_path_prefers_explicit_output_path() {
    let resolved = resolve_recording_output_path(Some("C:/tmp/custom.wav".to_string()), || {
        Err("fallback should not run".to_string())
    })
    .unwrap();

    assert_eq!(resolved, "C:/tmp/custom.wav");
}

#[test]
fn resolve_recording_output_path_uses_fallback_when_missing() {
    let resolved = resolve_recording_output_path(None, || Ok("generated.wav".to_string())).unwrap();

    assert_eq!(resolved, "generated.wav");
}

#[test]
fn resolve_recording_output_path_rejects_parent_traversal() {
    let error = resolve_recording_output_path(Some("../evil.wav".to_string()), || {
        Ok("generated.wav".to_string())
    })
    .unwrap_err();

    assert!(error.contains("parent directory traversal"));
}
#[test]
fn resolve_recording_output_path_rejects_fallback_parent_traversal() {
    let error =
        resolve_recording_output_path(None, || Ok("../evil_fallback.wav".to_string())).unwrap_err();

    assert!(error.contains("parent directory traversal"));
}

#[test]
fn resolve_recording_output_path_rejects_fallback_non_wav() {
    let error = resolve_recording_output_path(None, || Ok("fallback.mp3".to_string())).unwrap_err();

    assert!(error.contains(".wav extension"));
}

#[test]
fn resolve_recording_output_path_creates_nested_directories() {
    let temp_dir = std::env::temp_dir().join(format!("sona_test_dir_{}", uuid::Uuid::new_v4()));
    let nested_path = temp_dir.join("sub").join("nested").join("output.wav");
    let path_str = nested_path.to_string_lossy().to_string();

    let resolved = resolve_recording_output_path(Some(path_str.clone()), || {
        Err("fallback should not run".to_string())
    })
    .unwrap();

    assert_eq!(resolved, path_str);
    assert!(nested_path.parent().unwrap().exists());
    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[test]
fn shared_capture_state_tracks_recording_instances_explicitly() {
    let mut capture = SharedCaptureState::default();
    let (stop_tx, _stop_rx) = channel::<()>();
    let (recorder_tx, _recorder_rx) = tokio::sync::mpsc::channel::<RecorderCommand>(1);
    capture.commit_start_with_source(
        "inst-1".to_string(),
        true,
        AudioStopSignal::from_sender(stop_tx),
        recorder_tx,
        LiveSourceEpoch::default(),
        Arc::new(AtomicU64::new(0)),
    );
    assert!(capture.recording_instances.contains("inst-1"));

    capture.attach_instance("inst-2".to_string(), false);
    assert!(!capture.recording_instances.contains("inst-2"));

    let detach_2 = capture.detach_instance("inst-2");
    assert!(!detach_2.was_recording);

    let detach_1 = capture.detach_instance("inst-1");
    assert!(detach_1.was_recording);
    assert!(!capture.recording_instances.contains("inst-1"));
}

#[tokio::test]
async fn teardown_watch_channel_unblocks_pending_start_asynchronously() {
    let state = AudioState::new();
    let key = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "test-device".to_string(),
    };
    let source = LiveSourceEpoch::new("desktop-source-1", 1);
    {
        let mut registry = state.registry.lock().unwrap();
        let _rx = registry.mark_stopping(key.clone(), source.clone());
    }

    let state_clone = state.clone();
    let key_clone = key.clone();
    let waiter = tokio::spawn(async move {
        wait_for_device_teardown(&state_clone, &key_clone, std::time::Duration::from_secs(2)).await
    });

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert!(!waiter.is_finished());

    {
        let mut registry = state.registry.lock().unwrap();
        assert!(registry.clear_stopping(&key, &source));
    }

    let result = waiter.await.unwrap();
    assert!(result.is_ok());
}

#[test]
fn different_devices_do_not_share_start_locks() {
    let state = AudioState::new();
    let key_mic = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "mic-1".to_string(),
    };
    let key_sys = CaptureKey {
        kind: CaptureKind::System,
        device_name: "sys-1".to_string(),
    };
    let lock_mic = state.get_device_start_lock(&key_mic);
    let lock_sys = state.get_device_start_lock(&key_sys);
    assert!(!Arc::ptr_eq(&lock_mic, &lock_sys));

    let lock_mic_2 = state.get_device_start_lock(&key_mic);
    assert!(Arc::ptr_eq(&lock_mic, &lock_mic_2));
}

#[tokio::test]
async fn teardown_waiter_times_out_with_explicit_error() {
    let state = AudioState::new();
    let key = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "test-mic".to_string(),
    };
    let source = LiveSourceEpoch::new("desktop-source-1", 1);
    {
        let mut registry = state.registry.lock().unwrap();
        let _rx = registry.mark_stopping(key.clone(), source);
    }

    let result = wait_for_device_teardown(&state, &key, std::time::Duration::from_millis(50)).await;
    assert_eq!(
        result.unwrap_err(),
        "Timed out waiting for previous Microphone capture on 'test-mic' to stop"
    );
}

#[tokio::test]
async fn stop_shared_capture_retrieves_saved_recording_path() {
    let state = AudioState::new();
    let key = CaptureKey {
        kind: CaptureKind::Microphone,
        device_name: "test-mic".to_string(),
    };
    let (mut capture, _stop_rx, mut recorder_rx) = running_capture("record-owner", 1);
    capture
        .recording_instances
        .insert("record-owner".to_string());

    {
        let mut reg = state.registry.lock().unwrap();
        reg.captures.insert(key.clone(), capture);
        reg.instance_keys.insert(
            (CaptureKind::Microphone, "record-owner".to_string()),
            key.clone(),
        );
    }

    let recorder_task = tokio::spawn(async move {
        if let Some(RecorderCommand::Stop { completed, .. }) = recorder_rx.recv().await {
            let _ = completed.send("path/to/recorded.wav".to_string());
        }
    });

    let path = super::coordinator::stop_shared_capture(
        &state,
        CaptureKind::Microphone,
        "record-owner".to_string(),
    )
    .await
    .unwrap();

    recorder_task.await.unwrap();
    assert_eq!(path, "path/to/recorded.wav");
}
