use crate::account::{Account, AccountType};
use crate::error::AuthError;
use md5::{Digest, Md5};
use serde_json::Map;

/// Offline account with the same UUID scheme vanilla servers use.
pub fn offline_account(name: &str) -> Result<Account, AuthError> {
    let clean = name.trim();
    let valid = (3..=16).contains(&clean.len())
        && clean
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_');
    if !valid {
        return Err(AuthError::InvalidUsername);
    }
    let mut md5: [u8; 16] = Md5::digest(format!("OfflinePlayer:{clean}").as_bytes()).into();
    md5[6] = (md5[6] & 0x0f) | 0x30; // version 3
    md5[8] = (md5[8] & 0x3f) | 0x80; // RFC 4122 variant
    let uuid: String = md5.iter().map(|b| format!("{b:02x}")).collect();
    Ok(Account {
        kind: AccountType::Offline,
        uuid,
        name: clean.to_string(),
        access_token: "0".into(),
        msa_refresh_token: None,
        expires_at: None,
        skins: vec![],
        capes: vec![],
        credentials_locked: false,
        encrypted_credentials: None,
        memory_only: false,
        legacy_keyring_key: None,
        extra: Map::new(),
    })
}
