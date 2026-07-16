use serde_json::json;
use sona_core::sync::SyncPresetV1;
use sona_sync::{
    CreatedVault, change_master_password, create_vault, open_json, seal_json,
    unlock_with_master_password, unlock_with_recovery_key,
};

const MASTER_PASSWORD: &str = "correct horse battery staple";

#[test]
fn master_password_and_optional_recovery_key_unlock_the_same_vault_key() {
    let CreatedVault {
        header,
        vault_key,
        recovery_key,
    } = create_vault("vault-a", SyncPresetV1::Standard, MASTER_PASSWORD, true).unwrap();

    let password_key = unlock_with_master_password(&header, MASTER_PASSWORD).unwrap();
    let recovery_key = recovery_key.expect("recovery key should be created");
    let recovered_key = unlock_with_recovery_key(&header, &recovery_key).unwrap();

    assert_eq!(password_key.as_slice(), vault_key.as_slice());
    assert_eq!(recovered_key.as_slice(), vault_key.as_slice());
    assert!(unlock_with_master_password(&header, "wrong password").is_err());
}

#[test]
fn changing_master_password_rewraps_without_changing_the_vault_key() {
    let created = create_vault("vault-a", SyncPresetV1::Content, MASTER_PASSWORD, false).unwrap();

    let changed = change_master_password(
        &created.header,
        MASTER_PASSWORD,
        "a different strong master password",
    )
    .unwrap();

    assert!(unlock_with_master_password(&changed, MASTER_PASSWORD).is_err());
    assert_eq!(
        unlock_with_master_password(&changed, "a different strong master password")
            .unwrap()
            .as_slice(),
        created.vault_key.as_slice()
    );
}

#[test]
fn master_passwords_only_need_to_be_non_empty() {
    let created = create_vault("vault-a", SyncPresetV1::Standard, "x", false).unwrap();
    assert_eq!(
        unlock_with_master_password(&created.header, "x")
            .unwrap()
            .as_slice(),
        created.vault_key.as_slice()
    );

    let changed = change_master_password(&created.header, "x", "y").unwrap();
    assert!(unlock_with_master_password(&changed, "x").is_err());
    assert_eq!(
        unlock_with_master_password(&changed, "y")
            .unwrap()
            .as_slice(),
        created.vault_key.as_slice()
    );
    assert!(create_vault("vault-a", SyncPresetV1::Standard, "", false).is_err());
}

#[test]
fn encrypted_json_round_trips_and_rejects_tampering() {
    let created = create_vault("vault-a", SyncPresetV1::Standard, MASTER_PASSWORD, false).unwrap();
    let plaintext = json!({"transcript": "private words", "count": 2});

    let sealed = seal_json(&created.vault_key, b"segment-key", &plaintext).unwrap();

    assert!(!String::from_utf8_lossy(&sealed).contains("private words"));
    assert_eq!(
        open_json::<serde_json::Value>(&created.vault_key, b"segment-key", &sealed).unwrap(),
        plaintext
    );

    let mut tampered = sealed;
    let last = tampered.last_mut().unwrap();
    *last ^= 0x01;
    assert!(open_json::<serde_json::Value>(&created.vault_key, b"segment-key", &tampered).is_err());
}
