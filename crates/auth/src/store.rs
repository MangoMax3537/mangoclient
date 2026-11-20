use crate::error::StoreError;

/// Legacy v1 backend: per-account secret blobs (a JSON string) keyed by account
/// uuid. Only used to migrate `keyring:v1:<uuid>` envelopes to v2 and delete
/// the old entries; secrets are never written here (read and delete only).
pub trait CredentialStore: Send + Sync {
    /// `Ok(None)` when nothing is stored; `Err(Locked)` when the store exists
    /// but cannot be read right now.
    fn get(&self, key: &str) -> Result<Option<String>, StoreError>;
    fn delete(&self, key: &str) -> Result<(), StoreError>;
}

/// OS keychain (macOS Keychain, Windows Credential Manager, Secret Service).
#[cfg(feature = "keyring")]
#[derive(Debug, Clone, Default)]
pub struct KeyringStore;

#[cfg(feature = "keyring")]
const SERVICE: &str = "dev.mangomax.mangoclient";

#[cfg(feature = "keyring")]
fn map_err(e: keyring::Error) -> StoreError {
    match e {
        keyring::Error::NoStorageAccess(_) => StoreError::Locked,
        other => StoreError::Unavailable(other.to_string()),
    }
}

#[cfg(feature = "keyring")]
impl CredentialStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        let entry = keyring::Entry::new(SERVICE, key).map_err(map_err)?;
        match entry.get_password() {
            Ok(v) => Ok(Some(v)),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(map_err(e)),
        }
    }

    fn delete(&self, key: &str) -> Result<(), StoreError> {
        match keyring::Entry::new(SERVICE, key).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }
}
