use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccountType {
    #[default]
    Microsoft,
    Offline,
}

/// A skin or cape entry from the Minecraft profile API.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct Texture {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl Texture {
    pub fn is_active(&self) -> bool {
        self.state.as_deref() == Some("ACTIVE")
    }
}

/// Runtime account including secrets. Serialized form is the `accounts.json`
/// shape (camelCase); go through [`crate::serialize_account`] to persist it so
/// tokens end up in the vault.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    #[serde(rename = "type", default)]
    pub kind: AccountType,
    #[serde(default)]
    pub uuid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msa_refresh_token: Option<String>,
    /// Unix milliseconds; `None` for offline accounts.
    #[serde(default)]
    pub expires_at: Option<i64>,
    #[serde(default)]
    pub skins: Vec<Texture>,
    #[serde(default)]
    pub capes: Vec<Texture>,
    /// Secrets exist but the store is locked right now.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub credentials_locked: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub encrypted_credentials: Option<String>,
    #[serde(flatten)]
    pub extra: Map<String, Value>,
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account")
            .field("kind", &self.kind)
            .field("uuid", &self.uuid)
            .field("name", &self.name)
            .field("access_token", &"<redacted>")
            .field(
                "msa_refresh_token",
                &self.msa_refresh_token.as_ref().map(|_| "<redacted>"),
            )
            .field("expires_at", &self.expires_at)
            .field("credentials_locked", &self.credentials_locked)
            .finish_non_exhaustive()
    }
}

/// What the UI may see. Never contains tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAccount {
    pub uuid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: AccountType,
    pub expires_at: Option<i64>,
    pub expired: bool,
    pub has_cape: bool,
}

impl Account {
    pub fn to_public(&self) -> PublicAccount {
        self.to_public_at(now_ms())
    }

    pub fn to_public_at(&self, now_ms: i64) -> PublicAccount {
        let expired = match (self.kind, self.expires_at) {
            (AccountType::Offline, _) | (_, None) | (_, Some(0)) => false,
            (_, Some(e)) => e < now_ms,
        };
        PublicAccount {
            uuid: self.uuid.clone(),
            name: self.name.clone(),
            kind: self.kind,
            expires_at: self.expires_at,
            expired,
            has_cape: self.capes.iter().any(Texture::is_active),
        }
    }
}

pub(crate) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
