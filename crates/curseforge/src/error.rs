use std::path::PathBuf;
use std::sync::Arc;

use mango_archive::ArchiveError;
use mango_net::NetError;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors of the CurseForge layer. `code()` is the stable code for the UI.
/// No variant ever carries the API key.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(Arc<NetError>),
    #[error(transparent)]
    Archive(Arc<ArchiveError>),
    #[error("CurseForge request failed: {0}")]
    Request(String),
    #[error("CurseForge answered HTTP {status} for {path}")]
    Status { status: u16, path: String },
    #[error("CurseForge rejected the API key")]
    Unauthorized,
    #[error("CurseForge returned an unreadable response for {path}: {message}")]
    BadResponse { path: String, message: String },
    #[error("The CurseForge API key is malformed")]
    InvalidApiKey,
    #[error("I/O error at {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("CurseForge returned an invalid filename")]
    InvalidFilename,
    #[error("Invalid CurseForge modpack: {0}")]
    InvalidModpack(String),
    #[error("Unsafe override target: {0}")]
    UnsafeOverride(String),
    #[error("{0} may not be distributed by third-party launchers")]
    DistributionBlocked(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Net(e) => e.code(),
            Error::Archive(_) => "curseforge.archive",
            Error::Request(_) => "curseforge.request",
            Error::Status { .. } => "curseforge.http_status",
            Error::Unauthorized => "curseforge.unauthorized",
            Error::BadResponse { .. } => "curseforge.bad_response",
            Error::InvalidApiKey => "curseforge.invalid_api_key",
            Error::Io { .. } => "curseforge.io",
            Error::InvalidFilename => "curseforge.invalid_filename",
            Error::InvalidModpack(_) => "curseforge.invalid_modpack",
            Error::UnsafeOverride(_) => "curseforge.unsafe_override",
            Error::DistributionBlocked(_) => "curseforge.distribution_blocked",
        }
    }

    pub(crate) fn io(path: impl Into<PathBuf>, e: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            message: e.to_string(),
        }
    }
}

impl From<NetError> for Error {
    fn from(e: NetError) -> Self {
        Error::Net(Arc::new(e))
    }
}

impl From<ArchiveError> for Error {
    fn from(e: ArchiveError) -> Self {
        Error::Archive(Arc::new(e))
    }
}
