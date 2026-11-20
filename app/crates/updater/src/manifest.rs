//! `latest.json` model and strict validation.

use crate::error::{Result, UpdateError};
use base64::Engine;
use semver::Version;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

/// Largest manifest we are willing to parse.
pub const MAX_MANIFEST_BYTES: usize = 1024 * 1024;
/// Bounds for an installer size (bytes).
pub const MIN_UPDATE_SIZE: u64 = 1024;
pub const MAX_UPDATE_SIZE: u64 = 1024 * 1024 * 1024;

/// Hosts GitHub serves release assets from.
pub const DEFAULT_ALLOWED_HOSTS: &[&str] = &[
    "github.com",
    "objects.githubusercontent.com",
    "release-assets.githubusercontent.com",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformEntry {
    pub url: String,
    /// Lowercase hex SHA-256 of the file.
    pub sha256: String,
    pub size: u64,
    /// Base64 detached Ed25519 signature over the file bytes.
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: Version,
    #[serde(default)]
    pub pub_date: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
    /// Keys: `windows-x86_64`, `darwin-aarch64`, `darwin-x86_64`,
    /// `linux-x86_64`, `linux-x86_64-appimage`.
    pub platforms: BTreeMap<String, PlatformEntry>,
}

/// Which URLs an update may be fetched from. Injectable for tests/mirrors.
#[derive(Debug, Clone)]
pub struct UrlPolicy {
    allowed_hosts: Vec<String>,
    allow_insecure: bool,
}

impl Default for UrlPolicy {
    fn default() -> Self {
        Self::with_hosts(DEFAULT_ALLOWED_HOSTS.iter().map(|h| h.to_string()))
    }
}

impl UrlPolicy {
    /// HTTPS only, exact host match against `hosts`.
    pub fn with_hosts(hosts: impl IntoIterator<Item = String>) -> Self {
        Self {
            allowed_hosts: hosts.into_iter().map(|h| h.to_lowercase()).collect(),
            allow_insecure: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn insecure_for_tests(hosts: &[&str]) -> Self {
        Self {
            allowed_hosts: hosts.iter().map(|h| h.to_string()).collect(),
            allow_insecure: true,
        }
    }

    pub fn check(&self, raw: &str) -> Result<Url> {
        let rej = |why: &str| UpdateError::UrlRejected(format!("{why}: {raw}"));
        let url = Url::parse(raw).map_err(|_| rej("unparseable"))?;
        match url.scheme() {
            "https" => {}
            "http" if self.allow_insecure => {}
            _ => return Err(rej("not https")),
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(rej("credentials in URL"));
        }
        if !self.allow_insecure && url.port().is_some() {
            return Err(rej("explicit port"));
        }
        let host = url.host_str().ok_or_else(|| rej("no host"))?.to_lowercase();
        if !self.allowed_hosts.contains(&host) {
            return Err(rej("host not allowed"));
        }
        Ok(url)
    }
}

impl PlatformEntry {
    fn validate(&self, key: &str, policy: &UrlPolicy) -> Result<()> {
        let bad = |why: &str| UpdateError::ManifestInvalid(format!("{key}: {why}"));
        policy.check(&self.url)?;
        match hex::decode(&self.sha256) {
            Ok(b) if b.len() == 32 => {}
            _ => return Err(bad("sha256 must be 64 hex chars")),
        }
        if !(MIN_UPDATE_SIZE..=MAX_UPDATE_SIZE).contains(&self.size) {
            return Err(bad("size out of bounds"));
        }
        match base64::engine::general_purpose::STANDARD.decode(&self.signature) {
            Ok(b) if b.len() == 64 => {}
            _ => return Err(bad("signature must be base64 of 64 bytes")),
        }
        Ok(())
    }
}

impl UpdateManifest {
    /// Parse and strictly validate every entry (a bad foreign entry also
    /// invalidates the manifest: fail closed).
    pub fn parse(bytes: &[u8], policy: &UrlPolicy) -> Result<Self> {
        if bytes.len() > MAX_MANIFEST_BYTES {
            return Err(UpdateError::ManifestInvalid("manifest too large".into()));
        }
        let m: Self = serde_json::from_slice(bytes)
            .map_err(|e| UpdateError::ManifestInvalid(e.to_string()))?;
        if m.platforms.is_empty() {
            return Err(UpdateError::ManifestInvalid("no platforms".into()));
        }
        for (key, entry) in &m.platforms {
            entry.validate(key, policy)?;
        }
        Ok(m)
    }
}
