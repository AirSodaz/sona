use sona_core::sync::SyncObjectKey;

#[test]
fn accepts_normalized_relative_object_keys() {
    let key = SyncObjectKey::parse(
        "sona-sync/v1/vault/devices/device-a/segments/00000000000000000001-deadbeef.sync",
    )
    .unwrap();

    assert_eq!(
        key.as_str(),
        "sona-sync/v1/vault/devices/device-a/segments/00000000000000000001-deadbeef.sync"
    );
}

#[test]
fn rejects_unsafe_or_non_normalized_object_keys() {
    for invalid in [
        "",
        "/absolute",
        "C:/absolute",
        "sona-sync\\segment",
        "sona-sync//segment",
        "sona-sync/./segment",
        "sona-sync/../segment",
        "sona-sync/segment/",
    ] {
        assert!(
            SyncObjectKey::parse(invalid).is_err(),
            "expected key to be rejected: {invalid}"
        );
    }
}
