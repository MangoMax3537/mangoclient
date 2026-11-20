use mango_net::NetError;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Coarse error class shown to the player (mirrors the legacy JS codes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ErrorCode {
    NoRelease,
    Offline,
    Other,
}

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("invalid update manifest: {0}")]
    ManifestInvalid(String),
    #[error("update URL rejected: {0}")]
    UrlRejected(String),
    #[error("the manifest has no build for platform {0}")]
    NoPlatformBuild(String),
    #[error("the compiled-in update public key is missing or malformed")]
    BadPublicKey,
    #[error("update signature is malformed")]
    SignatureEncoding,
    #[error("update signature does not match the downloaded file")]
    SignatureInvalid,
    #[error("network error: {0}")]
    Net(#[from] NetError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("install step failed: {0}")]
    Install(String),
}

impl UpdateError {
    /// Stable identifier for the UI to localize.
    pub fn code(&self) -> &'static str {
        match self {
            Self::ManifestInvalid(_) => "updater.manifest_invalid",
            Self::UrlRejected(_) => "updater.url_rejected",
            Self::NoPlatformBuild(_) => "updater.no_platform_build",
            Self::BadPublicKey => "updater.bad_public_key",
            Self::SignatureEncoding => "updater.signature_encoding",
            Self::SignatureInvalid => "updater.signature_invalid",
            Self::Net(_) => "updater.net",
            Self::Io(_) => "updater.io",
            Self::Install(_) => "updater.install",
        }
    }

    /// Player-facing class: a missing feed, no connectivity, or anything else.
    pub fn kind(&self) -> ErrorCode {
        match self {
            Self::Net(NetError::Status { status: 404, .. }) => ErrorCode::NoRelease,
            Self::Net(NetError::Request { .. } | NetError::Timeout { .. }) => ErrorCode::Offline,
            _ => ErrorCode::Other,
        }
    }
}

pub type Result<T> = std::result::Result<T, UpdateError>;
