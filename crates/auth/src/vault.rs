//! Account persistence: encrypted envelopes in `accounts.json`.
//!
//! # Threat model
//!
//! Only the Microsoft refresh token is persisted, as an envelope v2
//! (`mc2:<base64(nonce || ciphertext)>`, XChaCha20-Poly1305, AAD binding the
//! account uuid and format version) inside the account object of
//! `accounts.json`. The 256-bit key is random per device, lives only in the OS
//! keystore and is never written to disk. Access tokens are never persisted:
//! they are empty after load and derived from the refresh token by
//! [`crate::ensure_fresh_account`].
//!
//! Protects against: theft or backup leaks of `accounts.json`, copied
//! profiles/data dirs, other OS users, disk forensics. There is NO plaintext
//! fallback: with the keystore locked or unavailable, accounts load with
//! `credentials_locked = true`, the existing envelope is preserved
//! byte-for-byte on save, and tokens obtained meanwhile are memory-only
//! ([`StoredAccount::persisted`] is false).
//!
//! Honest limit: malware running as the same user in the same login session
//! can often call the same OS keystore APIs (or read process memory), so this
//! does not stop it. Short token lifetimes and no tokens in logs/UI limit the
//! damage.
//!
//! Migration: legacy plaintext `accessToken`/`msaRefreshToken` fields and v1
//! envelopes (`keyring:v1:<uuid>`, one keyring entry per account) are
//! converted to v2 on load. The next [`Vault::store_account`] writes no
//! plaintext and deletes the old keyring entry. Electron (Chromium OSCrypt)
//! blobs are unreadable and yield [`AuthError::LegacyEnvelope`].

use crate::account::{Account, AccountType};
use crate::envelope;
use crate::error::{AuthError, StoreError};
use crate::keystore::DeviceKeyStore;
use crate::store::CredentialStore;
use serde_json::{Map, Value};
use std::sync::Arc;
use zeroize::Zeroize;

/// Legacy plaintext field names (also stripped defensively on save).
pub const SECRET_FIELDS: [&str; 2] = ["accessToken", "msaRefreshToken"];
/// v1 envelope prefix (`keyring:v1:<uuid>`), migrated away on load.
pub const ENVELOPE_PREFIX: &str = "keyring:v1:";
const ENVELOPE_KEY: &str = "encryptedCredentials";
const LOCKED_KEY: &str = "credentialsLocked";

/// A legacy v1 keyring entry is named by an account uuid: 32 hex digits, or
/// the dashed 8-4-4-4-12 form. Anything else (notably `device-key-v1`, which
/// lives in the same keyring service) is never read or deleted.
pub fn is_legacy_key_name(key: &str) -> bool {
    let hex = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_hexdigit());
    if key.len() == 32 {
        return hex(key);
    }
    let parts: Vec<&str> = key.split('-').collect();
    key.len() == 36
        && parts.len() == 5
        && parts
            .iter()
            .zip([8usize, 4, 4, 4, 12])
            .all(|(p, n)| p.len() == n && hex(p))
}

fn is_secret(v: &Value) -> bool {
    matches!(v, Value::String(s) if !s.is_empty() && s != "0")
}

/// True if an `accounts.json` entry still carries plaintext tokens.
pub fn has_plaintext_credentials(account: &Map<String, Value>) -> bool {
    SECRET_FIELDS
        .iter()
        .any(|k| account.get(*k).is_some_and(is_secret))
}

/// True if loading and re-saving this entry changes its on-disk form
/// (plaintext tokens or a v1 envelope); callers should save after load.
pub fn needs_migration(account: &Map<String, Value>) -> bool {
    has_plaintext_credentials(account)
        || account
            .get(ENVELOPE_KEY)
            .and_then(Value::as_str)
            .is_some_and(|e| e.starts_with(ENVELOPE_PREFIX))
}

/// An account in its `accounts.json` form.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredAccount {
    pub value: Map<String, Value>,
    /// False when the account holds tokens that are not in `value` (keystore
    /// locked or unavailable): show a "not saved" notice.
    pub persisted: bool,
}

/// Encrypts and decrypts account secrets with the device key.
pub struct Vault {
    keys: Arc<dyn DeviceKeyStore>,
    legacy: Option<Arc<dyn CredentialStore>>,
}

impl Vault {
    /// `legacy` is only read/deleted to migrate v1 envelopes.
    pub fn new(keys: Arc<dyn DeviceKeyStore>, legacy: Option<Arc<dyn CredentialStore>>) -> Self {
        Self { keys, legacy }
    }

    /// OS keystore for the device key and the v1 keyring entries.
    #[cfg(feature = "keyring")]
    pub fn system() -> Self {
        Self::new(
            Arc::new(crate::keystore::KeyringDeviceKey::new()),
            Some(Arc::new(crate::store::KeyringStore)),
        )
    }

    /// Stored `accounts.json` entry -> runtime account. Access token is empty
    /// (derived at launch; offline accounts keep `"0"`).
    pub fn load_account(&self, stored: &Map<String, Value>) -> Result<Account, AuthError> {
        // Typed errors from serde can quote values: never forward them.
        let mut acc: Account = serde_json::from_value(Value::Object(stored.clone()))
            .map_err(|_| AuthError::InvalidAccount)?;
        acc.access_token.zeroize();
        acc.credentials_locked = false;
        acc.memory_only = false;
        acc.legacy_keyring_key = None;
        if acc.kind == AccountType::Offline {
            acc.access_token = "0".into();
            acc.msa_refresh_token = None;
            acc.encrypted_credentials = None;
            return Ok(acc);
        }
        if acc.msa_refresh_token.as_deref().is_some_and(|t| t == "0") {
            acc.msa_refresh_token = None;
        }
        let Some(env) = acc.encrypted_credentials.clone() else {
            return Ok(acc); // plaintext (if any) is migrated on next save
        };
        if envelope::is_v2(&env) {
            match self.keys.get_or_create() {
                Ok(key) => match envelope::open(&key, &acc.uuid, &env) {
                    Ok(rt) => acc.msa_refresh_token = Some(rt.to_string()),
                    Err(_) => {
                        tracing::warn!("credential envelope could not be decrypted");
                        acc.credentials_locked = true;
                    }
                },
                Err(_) => acc.credentials_locked = true,
            }
        } else if let Some(key) = env.strip_prefix(ENVELOPE_PREFIX) {
            if !is_legacy_key_name(key) {
                // A forged name could point the cleanup at another keyring
                // entry (the device key!): treat the envelope as unreadable.
                return Err(AuthError::LegacyEnvelope);
            }
            match self.legacy.as_ref().map(|s| s.get(key)) {
                Some(Ok(Some(mut blob))) => {
                    acc.msa_refresh_token = refresh_from_v1(&blob);
                    blob.zeroize();
                    acc.legacy_keyring_key = Some(key.to_string());
                }
                Some(Ok(None)) => {
                    // Entry gone: needs a fresh sign-in.
                    acc.encrypted_credentials = None;
                }
                Some(Err(_)) | None => acc.credentials_locked = true,
            }
        } else {
            return Err(AuthError::LegacyEnvelope);
        }
        Ok(acc)
    }

    /// Runtime account -> `accounts.json` entry. Never contains plaintext
    /// tokens. See the module docs for the locked/unavailable behavior.
    pub fn store_account(&self, account: &Account) -> Result<StoredAccount, AuthError> {
        let Value::Object(mut out) =
            serde_json::to_value(account).map_err(|_| AuthError::InvalidAccount)?
        else {
            return Err(AuthError::InvalidAccount);
        };
        out.remove(ENVELOPE_KEY);
        out.remove(LOCKED_KEY);
        for k in SECRET_FIELDS {
            out.remove(k);
        }
        if account.kind == AccountType::Offline {
            return Ok(StoredAccount {
                value: out,
                persisted: true,
            });
        }
        let refresh = account
            .msa_refresh_token
            .as_deref()
            .filter(|t| !t.is_empty());
        let existing = account.encrypted_credentials.clone();
        let (envelope, persisted) = if account.credentials_locked {
            // Unreadable existing envelope: keep it exactly as it was.
            (existing, refresh.is_none())
        } else if let Some(rt) = refresh {
            match self.keys.get_or_create() {
                Ok(key) => {
                    if account.uuid.is_empty() {
                        return Err(AuthError::InvalidAccount);
                    }
                    let env = envelope::seal(&key, &account.uuid, rt)?;
                    self.delete_legacy(account.legacy_keyring_key.as_deref());
                    (Some(env), true)
                }
                Err(StoreError::Locked | StoreError::Unavailable(_)) => (existing, false),
                Err(e) => return Err(e.into()),
            }
        } else {
            // Signed out / nothing to keep.
            self.delete_legacy(account.legacy_keyring_key.as_deref());
            (None, true)
        };
        if let Some(env) = envelope {
            out.insert(ENVELOPE_KEY.into(), Value::String(env));
        }
        Ok(StoredAccount {
            value: out,
            persisted,
        })
    }

    /// Cleanup when an account is deleted: removes a leftover v1 keyring entry.
    /// v2 envelopes live in the account object itself, nothing else to delete.
    pub fn delete_credentials(&self, stored: &Map<String, Value>) -> Result<(), AuthError> {
        if let (Some(store), Some(key)) = (
            self.legacy.as_ref(),
            stored
                .get(ENVELOPE_KEY)
                .and_then(Value::as_str)
                .and_then(|e| e.strip_prefix(ENVELOPE_PREFIX))
                .filter(|k| is_legacy_key_name(k)),
        ) {
            store.delete(key)?;
        }
        Ok(())
    }

    fn delete_legacy(&self, key: Option<&str>) {
        if let (Some(store), Some(key)) =
            (self.legacy.as_ref(), key.filter(|k| is_legacy_key_name(k)))
        {
            if store.delete(key).is_err() {
                tracing::warn!("could not delete legacy credential entry");
            }
        }
    }
}

fn refresh_from_v1(blob: &str) -> Option<String> {
    #[derive(serde::Deserialize)]
    struct V1 {
        #[serde(rename = "msaRefreshToken")]
        msa_refresh_token: Option<String>,
    }
    serde_json::from_str::<V1>(blob)
        .ok()
        .and_then(|v| v.msa_refresh_token)
        .filter(|t| !t.is_empty())
}

impl Account {
    /// Load from a stored `accounts.json` entry.
    pub fn from_stored(stored: &Map<String, Value>, vault: &Vault) -> Result<Self, AuthError> {
        vault.load_account(stored)
    }

    /// Storable form (envelope, no plaintext tokens).
    pub fn to_stored(&self, vault: &Vault) -> Result<Map<String, Value>, AuthError> {
        Ok(vault.store_account(self)?.value)
    }
}
