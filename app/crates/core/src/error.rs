use std::path::PathBuf;

use crate::validation::ValidationError;

pub type Result<T> = std::result::Result<T, Error>;

/// Errors surfaced by `mango-core`. `code()` is the stable code for the UI.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid profile ID")]
    InvalidProfileId,
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error("I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Profile not found")]
    ProfileNotFound,
    #[error("Unsafe mod record: {0}")]
    UnsafeMod(String),
}

impl Error {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Error::Io {
            path: path.into(),
            source,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            Error::InvalidProfileId => "invalid_profile_id",
            Error::Validation(_) => "validation",
            Error::Io { .. } => "io",
            Error::Json(_) => "json",
            Error::ProfileNotFound => "profile_not_found",
            Error::UnsafeMod(_) => "unsafe_mod",
        }
    }
}
