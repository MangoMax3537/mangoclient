use crate::error::StoreError;
use serde_json::{Map, Value};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Backend for per-account secret blobs (a JSON string), keyed by account uuid.
pub trait CredentialStore: Send + Sync {
    /// `Ok(None)` when nothing is stored; `Err(Locked)` when the store exists
    /// but cannot be read right now.
    fn get(&self, key: &str) -> Result<Option<String>, StoreError>;
    fn set(&self, key: &str, value: &str) -> Result<(), StoreError>;
    fn delete(&self, key: &str) -> Result<(), StoreError>;
    /// True only for a real OS keychain. Insecure stores make the vault keep
    /// tokens in the 0600 accounts file instead of an envelope.
    fn is_secure(&self) -> bool;
}

/// 0600 JSON file holding key -> blob. Not a secure store: it is the honest
/// fallback when no keychain exists (e.g. Linux without a secret service).
#[derive(Debug, Clone)]
pub struct FileFallbackStore {
    path: PathBuf,
}

impl FileFallbackStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn read(&self) -> Result<Map<String, Value>, StoreError> {
        match std::fs::read(&self.path) {
            Ok(bytes) => match serde_json::from_slice::<Value>(&bytes) {
                Ok(Value::Object(m)) => Ok(m),
                _ => Err(StoreError::Corrupt(self.path.display().to_string())),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Map::new()),
            Err(e) => Err(e.into()),
        }
    }

    fn write(&self, map: &Map<String, Value>) -> Result<(), StoreError> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("tmp");
        let mut opts = std::fs::OpenOptions::new();
        opts.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        let mut f = opts.open(&tmp)?;
        let data =
            serde_json::to_vec_pretty(map).map_err(|e| StoreError::Corrupt(e.to_string()))?;
        f.write_all(&data)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

impl CredentialStore for FileFallbackStore {
    fn get(&self, key: &str) -> Result<Option<String>, StoreError> {
        Ok(self
            .read()?
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string))
    }

    fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        let mut map = self.read()?;
        map.insert(key.to_string(), Value::String(value.to_string()));
        self.write(&map)
    }

    fn delete(&self, key: &str) -> Result<(), StoreError> {
        let mut map = self.read()?;
        if map.remove(key).is_some() {
            self.write(&map)?;
        }
        Ok(())
    }

    fn is_secure(&self) -> bool {
        false
    }
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

    fn set(&self, key: &str, value: &str) -> Result<(), StoreError> {
        keyring::Entry::new(SERVICE, key)
            .and_then(|e| e.set_password(value))
            .map_err(map_err)
    }

    fn delete(&self, key: &str) -> Result<(), StoreError> {
        match keyring::Entry::new(SERVICE, key).and_then(|e| e.delete_credential()) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(map_err(e)),
        }
    }

    /// Probes the backend: a missing entry means it works, any other error
    /// (no secret service, locked, denied) means it does not.
    fn is_secure(&self) -> bool {
        match keyring::Entry::new(SERVICE, "__probe__").and_then(|e| e.get_password()) {
            Ok(_) | Err(keyring::Error::NoEntry) => true,
            Err(_) => false,
        }
    }
}
