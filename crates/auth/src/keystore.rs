//! The per-device 256-bit key that encrypts account secrets.
//!
//! The key is generated once with the OS CSPRNG and lives only in the OS
//! keystore (Windows Credential Manager, macOS Keychain, Linux Secret
//! Service). It is never written to disk and never logged; in memory it sits
//! in a zeroizing buffer.

use crate::error::StoreError;
use std::fmt;
use std::sync::{Arc, Mutex};
use zeroize::Zeroizing;

pub const KEY_LEN: usize = 32;

/// 256-bit device key. Cheap to clone (clones share one zeroizing buffer),
/// redacted in `Debug`.
#[derive(Clone)]
pub struct DeviceKey(Arc<Zeroizing<[u8; KEY_LEN]>>);

impl DeviceKey {
    /// Fresh random key from the OS CSPRNG.
    pub fn generate() -> Result<Self, StoreError> {
        let mut bytes = Zeroizing::new([0u8; KEY_LEN]);
        getrandom::getrandom(bytes.as_mut_slice())
            .map_err(|_| StoreError::Unavailable("no secure random source".into()))?;
        Ok(Self(Arc::new(bytes)))
    }

    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(Arc::new(Zeroizing::new(bytes)))
    }

    pub(crate) fn as_bytes(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

impl fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceKey(<redacted>)")
    }
}

/// Source of the device key.
pub trait DeviceKeyStore: Send + Sync {
    /// Returns the existing key or creates and stores a new one.
    /// `Err(Locked | Unavailable)` when the keystore cannot be used; callers
    /// must then treat tokens as memory-only.
    fn get_or_create(&self) -> Result<DeviceKey, StoreError>;
}

/// In-memory key store for tests and headless tooling. Can be locked to
/// simulate an unavailable keystore.
#[derive(Default)]
pub struct MemoryDeviceKey {
    state: Mutex<MemState>,
}

#[derive(Default)]
struct MemState {
    key: Option<DeviceKey>,
    locked: bool,
}

impl MemoryDeviceKey {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_key(key: DeviceKey) -> Self {
        Self {
            state: Mutex::new(MemState {
                key: Some(key),
                locked: false,
            }),
        }
    }

    pub fn set_locked(&self, locked: bool) {
        self.lock().locked = locked;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, MemState> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
}

impl DeviceKeyStore for MemoryDeviceKey {
    fn get_or_create(&self) -> Result<DeviceKey, StoreError> {
        let mut st = self.lock();
        if st.locked {
            return Err(StoreError::Locked);
        }
        if let Some(k) = &st.key {
            return Ok(k.clone());
        }
        let k = DeviceKey::generate()?;
        st.key = Some(k.clone());
        Ok(k)
    }
}

/// OS keystore backed device key (service `dev.mangomax.mangoclient`, user
/// `device-key-v1`, value base64). Cached in memory after the first access.
#[cfg(feature = "keyring")]
#[derive(Default)]
pub struct KeyringDeviceKey {
    cache: Mutex<Option<DeviceKey>>,
}

#[cfg(feature = "keyring")]
impl KeyringDeviceKey {
    pub const SERVICE: &'static str = "dev.mangomax.mangoclient";
    pub const USER: &'static str = "device-key-v1";

    pub fn new() -> Self {
        Self::default()
    }
}

#[cfg(feature = "keyring")]
fn map_kr(e: keyring::Error) -> StoreError {
    match e {
        keyring::Error::NoStorageAccess(_) => StoreError::Locked,
        // Details can name platform internals but never secret values.
        other => StoreError::Unavailable(other.to_string()),
    }
}

/// Runs `f` on a fresh plain thread and waits for it.
///
/// The Secret Service backend of the `keyring` crate blocks on its own
/// executor, which panics ("Cannot start a runtime from within a runtime") when
/// it is reached from a thread that already drives a tokio runtime. Every
/// keyring call goes through here, so it works from async code and from sync
/// code alike. The calls are short, blocking the caller briefly is fine.
#[cfg(feature = "keyring")]
pub(crate) fn off_runtime<T, F>(f: F) -> Result<T, StoreError>
where
    T: Send,
    F: FnOnce() -> Result<T, StoreError> + Send,
{
    std::thread::scope(|scope| {
        scope
            .spawn(f)
            .join()
            .unwrap_or_else(|_| Err(StoreError::Unavailable("keystore call failed".into())))
    })
}

#[cfg(feature = "keyring")]
fn load_or_create_in_keyring() -> Result<DeviceKey, StoreError> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    let entry =
        keyring::Entry::new(KeyringDeviceKey::SERVICE, KeyringDeviceKey::USER).map_err(map_kr)?;
    match entry.get_password() {
        Ok(text) => {
            let text = Zeroizing::new(text);
            let mut raw = Zeroizing::new([0u8; KEY_LEN + 3]);
            let n = STANDARD
                .decode_slice(text.trim().as_bytes(), raw.as_mut_slice())
                .map_err(|_| StoreError::Corrupt("device key".into()))?;
            if n != KEY_LEN {
                // Never overwrite an unreadable key: that would orphan
                // every existing envelope.
                return Err(StoreError::Corrupt("device key".into()));
            }
            let mut bytes = [0u8; KEY_LEN];
            bytes.copy_from_slice(&raw[..KEY_LEN]);
            let k = DeviceKey::from_bytes(bytes);
            zeroize::Zeroize::zeroize(&mut bytes);
            Ok(k)
        }
        Err(keyring::Error::NoEntry) => {
            let k = DeviceKey::generate()?;
            let text = Zeroizing::new(STANDARD.encode(k.as_bytes()));
            entry.set_password(&text).map_err(map_kr)?;
            Ok(k)
        }
        Err(e) => Err(map_kr(e)),
    }
}

#[cfg(feature = "keyring")]
impl DeviceKeyStore for KeyringDeviceKey {
    fn get_or_create(&self) -> Result<DeviceKey, StoreError> {
        let mut cache = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(k) = cache.as_ref() {
            return Ok(k.clone());
        }
        let key = off_runtime(load_or_create_in_keyring)?;
        *cache = Some(key.clone());
        Ok(key)
    }
}
