use mango_net::NetError;
use thiserror::Error;

/// Failure talking to a credential store.
#[derive(Debug, Error)]
pub enum StoreError {
    /// The store exists but is locked or access was denied; may work later.
    #[error("credential store is locked")]
    Locked,
    /// No usable backend on this system.
    #[error("credential store unavailable: {0}")]
    Unavailable(String),
    #[error("credential store I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("credential store data is corrupt: {0}")]
    Corrupt(String),
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("username must be 3-16 characters (letters, digits, underscore)")]
    InvalidUsername,
    #[error("session expired, sign in again")]
    NeedsRelogin,
    #[error("credentials were stored by the Electron launcher and cannot be read")]
    LegacyEnvelope,
    #[error("account has no usable uuid")]
    InvalidAccount,
    #[error("Microsoft sign-in failed: {0}")]
    LoginFailed(String),
    #[error("this Microsoft account has no Xbox profile")]
    XboxNoProfile,
    #[error("Xbox Live is not available in this account's country")]
    XboxCountry,
    #[error("this account needs adult verification")]
    XboxAdultVerification,
    #[error("this is a child account and must be added to a family first")]
    XboxChildAccount,
    #[error("this account owns Minecraft but has no profile yet")]
    NoProfile,
    #[error("this Microsoft account does not own Minecraft: Java Edition")]
    NotOwned,
    #[error("could not load the Minecraft profile (HTTP {0})")]
    ProfileHttp(u16),
    #[error("HTTP {status}: {message}")]
    Http {
        status: u16,
        message: String,
        /// Xbox `XErr` code when present.
        xerr: Option<String>,
    },
    #[error("offline accounts cannot change their skin")]
    OfflineSkin,
    #[error("skin upload failed (HTTP {0})")]
    SkinUpload(u16),
    #[error("request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("request timed out")]
    Timeout,
    #[error(transparent)]
    Net(#[from] NetError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl AuthError {
    /// Stable identifier for UI localization.
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidUsername => "auth.invalid_username",
            Self::NeedsRelogin => "auth.needs_relogin",
            Self::LegacyEnvelope => "auth.legacy_envelope",
            Self::InvalidAccount => "auth.invalid_account",
            Self::LoginFailed(_) => "auth.login_failed",
            Self::XboxNoProfile => "auth.xbox.no_profile",
            Self::XboxCountry => "auth.xbox.country",
            Self::XboxAdultVerification => "auth.xbox.adult_verification",
            Self::XboxChildAccount => "auth.xbox.child_account",
            Self::NoProfile => "auth.no_profile",
            Self::NotOwned => "auth.not_owned",
            Self::ProfileHttp(_) => "auth.profile_http",
            Self::Http { .. } => "auth.http",
            Self::OfflineSkin => "auth.offline_skin",
            Self::SkinUpload(_) => "auth.skin_upload",
            Self::Request(_) => "auth.request",
            Self::Timeout => "auth.timeout",
            Self::Net(e) => e.code(),
            Self::Store(StoreError::Locked) => "auth.store_locked",
            Self::Store(_) => "auth.store",
            Self::Json(_) => "auth.json",
            Self::Io(_) => "auth.io",
        }
    }

    /// True when the user has to sign in again.
    pub fn needs_relogin(&self) -> bool {
        matches!(self, Self::NeedsRelogin | Self::LegacyEnvelope)
    }
}
