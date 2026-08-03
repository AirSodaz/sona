use serde_json::json;
use sona_core::sync::{
    HybridLogicalClock, SyncCausalContext, SyncEntityKey, SyncEntityKind, SyncOperation,
    SyncOperationKind, SyncVersion,
};
use sona_sync::{MAX_SEGMENT_OPERATIONS, SyncSegmentV1, checkpoint_object_key, segment_object_key};

fn operation(index: u64) -> SyncOperation {
    SyncOperation {
        operation_id: format!("op-{index}"),
        source_device_id: "device-a".to_string(),
        source_sequence: index,
        causal_context: SyncCausalContext::default(),
        version: SyncVersion {
            clock: HybridLogicalClock {
                physical_ms: index,
                logical: 0,
            },
            device_id: "device-a".to_string(),
            operation_id: format!("op-{index}"),
        },
        entity: SyncEntityKey {
            kind: SyncEntityKind::Project,
            id: "project-1".to_string(),
        },
        kind: SyncOperationKind::SetField {
            field: "name".to_string(),
            value: json!(format!("Project {index}")),
        },
    }
}

#[test]
fn object_layout_is_stable_and_validated_by_core() {
    let segment = segment_object_key("vault-a", "device-a", 42, "deadbeef").unwrap();
    let checkpoint = checkpoint_object_key("vault-a", "device-a", 42, "cafebabe").unwrap();

    assert_eq!(
        segment.as_str(),
        "sona-sync/v2/vault-a/devices/device-a/segments/00000000000000000042-deadbeef.sync"
    );
    assert_eq!(
        checkpoint.as_str(),
        "sona-sync/v2/vault-a/devices/device-a/checkpoints/00000000000000000042-cafebabe.sync"
    );
    assert!(segment_object_key("../vault", "device-a", 1, "hash").is_err());
}

#[test]
fn segment_validation_rejects_wrong_version_and_oversized_batches() {
    let valid = SyncSegmentV1 {
        protocol_version: sona_core::sync::SYNC_PROTOCOL_VERSION,
        vault_id: "vault-a".to_string(),
        device_id: "device-a".to_string(),
        sequence: 1,
        previous_cipher_hash: None,
        created_at_ms: 123,
        operations: vec![operation(1)],
    };
    valid.validate().unwrap();

    let mut future = valid.clone();
    future.protocol_version = 1;
    assert!(
        future
            .validate()
            .unwrap_err()
            .to_string()
            .contains("version")
    );

    let mut oversized = valid;
    oversized.operations = (0..=MAX_SEGMENT_OPERATIONS as u64).map(operation).collect();
    assert!(
        oversized
            .validate()
            .unwrap_err()
            .to_string()
            .contains("operation limit")
    );
}
