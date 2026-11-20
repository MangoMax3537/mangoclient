use std::path::PathBuf;
use std::sync::Arc;

use mango_archive::ArchiveError;
use mango_net::NetError;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors of the Modrinth layer. `code()` is the stable code for the UI.
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Net(Arc<NetError>),
    #[error(transparent)]
    Archive(Arc<ArchiveError>),
    #[error(transparent)]
    Core(Arc<mango_core::Error>),
    #[error("I/O error at {}: {message}", path.display())]
    Io { path: PathBuf, message: String },
    #[error("Modrinth returned an invalid filename")]
    InvalidFilename,
    #[error("{title} has no build for {loader} {mc_version}")]
    NoCompatibleVersion {
        title: String,
        loader: String,
        mc_version: String,
    },
    #[error("Required dependency {0} is not installed")]
    DependencyNotInstalled(String),
    #[error("{title} requires a different version of {dependency}. Run Check for updates before enabling it.")]
    DependencyVersionMismatch { title: String, dependency: String },
    #[error(
        "Mod dependencies could not be verified. Check your internet connection and try again."
    )]
    MetadataIncomplete,
    #[error("Invalid .mrpack: {0}")]
    InvalidModpack(String),
    #[error("No download for {0}")]
    NoDownload(String),
    #[error("Unsafe override target: {0}")]
    UnsafeOverride(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Error::Net(e) => e.code(),
            Error::Archive(_) => "modrinth.archive",
            Error::Core(e) => e.code(),
            Error::Io { .. } => "modrinth.io",
            Error::InvalidFilename => "modrinth.invalid_filename",
            Error::NoCompatibleVersion { .. } => "modrinth.no_compatible_version",
            Error::DependencyNotInstalled(_) => "modrinth.dependency_not_installed",
            Error::DependencyVersionMismatch { .. } => "modrinth.dependency_version_mismatch",
            Error::MetadataIncomplete => "modrinth.metadata_incomplete",
            Error::InvalidModpack(_) => "modrinth.invalid_modpack",
            Error::NoDownload(_) => "modrinth.no_download",
            Error::UnsafeOverride(_) => "modrinth.unsafe_override",
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
impl From<mango_core::Error> for Error {
    fn from(e: mango_core::Error) -> Self {
        Error::Core(Arc::new(e))
    }
}
