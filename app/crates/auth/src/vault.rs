//! Envelope logic of `credentials.js`, working on the `accounts.json` account
//! objects (JSON maps) so unknown fields survive untouched.
//!
//! With a secure store the tokens live in the store and the account carries
//! `encryptedCredentials = "keyring:v1:<uuid>"`. Without one they stay inline
//! (the accounts file is 0600). An envelope that cannot be read because the
//! store is locked is kept verbatim with `credentialsLocked: true`.
//! Envelopes written by the Electron launcher (Chromium OSCrypt) are not
//! readable and yield [`AuthError::LegacyEnvelope`] so the caller can force
//! one re-login.

use crate::account::Account;
use crate::error::{AuthError, StoreError};
use crate::store::CredentialStore;
use serde_json::{Map, Value};

pub const SECRET_FIELDS: [&str; 2] = ["accessToken", "msaRefreshToken"];
pub const ENVELOPE_PREFIX: &str = "keyring:v1:";
const ENVELOPE_KEY: &str = "encryptedCredentials";
const LOCKED_KEY: &str = "credentialsLocked";

fn is_secret(v: &Value) -> bool {
    matches!(v, Value::String(s) if !s.is_empty() && s != "0")
}

pub fn has_plaintext_credentials(account: &Map<String, Value>) -> bool {
    SECRET_FIELDS
        .iter()
        .any(|k| account.get(*k).is_some_and(is_secret))
}

/// Runtime account -> storable account (secrets moved into the store when it
/// is secure).
pub fn serialize_account(
    account: &Map<String, Value>,
    store: &dyn CredentialStore,
) -> Result<Map<String, Value>, AuthError> {
    let mut out = account.clone();
    let preserved = out.remove(ENVELOPE_KEY);
    out.remove(LOCKED_KEY);
    let mut secrets = Map::new();
    for key in SECRET_FIELDS {
        if let Some(v) = out.remove(key) {
            if is_secret(&v) {
                secrets.insert(key.to_string(), v);
            }
        }
    }
    if secrets.is_empty() {
        if let Some(env @ Value::String(_)) = preserved {
            out.insert(ENVELOPE_KEY.into(), env);
        }
        return Ok(out);
    }
    if !store.is_secure() {
        out.extend(secrets);
        return Ok(out);
    }
    let uuid = out
        .get("uuid")
        .and_then(Value::as_str)
        .filter(|u| !u.is_empty())
        .ok_or(AuthError::InvalidAccount)?
        .to_string();
    store.set(&uuid, &serde_json::to_string(&secrets)?)?;
    out.insert(
        ENVELOPE_KEY.into(),
        Value::String(format!("{ENVELOPE_PREFIX}{uuid}")),
    );
    Ok(out)
}

/// Stored account -> runtime account with tokens filled in.
pub fn deserialize_account(
    stored: &Map<String, Value>,
    store: &dyn CredentialStore,
) -> Result<Map<String, Value>, AuthError> {
    let mut out = stored.clone();
    let Some(envelope) = out.remove(ENVELOPE_KEY) else {
        return Ok(out);
    };
    let Some(key) = envelope
        .as_str()
        .and_then(|e| e.strip_prefix(ENVELOPE_PREFIX))
    else {
        return Err(AuthError::LegacyEnvelope);
    };
    match store.get(key) {
        Ok(Some(blob)) => {
            if let Value::Object(secrets) = serde_json::from_str::<Value>(&blob)? {
                for k in SECRET_FIELDS {
                    if let Some(v @ Value::String(_)) = secrets.get(k) {
                        out.insert(k.to_string(), v.clone());
                    }
                }
            }
            Ok(out)
        }
        // Store reachable but the entry is gone: the account loads without
        // tokens and will need a fresh sign-in.
        Ok(None) => Ok(out),
        Err(StoreError::Locked | StoreError::Unavailable(_)) => {
            out.insert(ENVELOPE_KEY.into(), envelope);
            out.insert(LOCKED_KEY.into(), Value::Bool(true));
            Ok(out)
        }
        Err(e) => Err(e.into()),
    }
}

/// Remove an account's secrets from the store (when the account is deleted).
/// Foreign or legacy envelopes are ignored.
pub fn delete_credentials(
    stored: &Map<String, Value>,
    store: &dyn CredentialStore,
) -> Result<(), AuthError> {
    if let Some(key) = stored
        .get(ENVELOPE_KEY)
        .and_then(Value::as_str)
        .and_then(|e| e.strip_prefix(ENVELOPE_PREFIX))
    {
        store.delete(key)?;
    }
    Ok(())
}

impl Account {
    /// Load from a stored `accounts.json` entry.
    pub fn from_stored(
        stored: &Map<String, Value>,
        store: &dyn CredentialStore,
    ) -> Result<Self, AuthError> {
        let runtime = deserialize_account(stored, store)?;
        Ok(serde_json::from_value(Value::Object(runtime))?)
    }

    /// Storable form with secrets moved into the store when it is secure.
    pub fn to_stored(&self, store: &dyn CredentialStore) -> Result<Map<String, Value>, AuthError> {
        match serde_json::to_value(self)? {
            Value::Object(m) => serialize_account(&m, store),
            _ => Err(AuthError::InvalidAccount),
        }
    }
}
