use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("invalid Mango API URL")]
    ApiUrlInvalid,
    #[error("Mango API URL must be an origin")]
    ApiUrlNotOrigin,
    #[error("Mango API URL must use HTTPS")]
    ApiUrlNotHttps,
    #[error("invalid profile id")]
    InvalidProfileId,
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("persistence error: {0}")]
    Core(#[from] mango_core::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

impl Error {
    pub fn io(path: &Path, source: std::io::Error) -> Self {
        Error::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    /// Stable code for the UI layer.
    pub fn code(&self) -> &'static str {
        match self {
            Error::ApiUrlInvalid => "mango.api_url_invalid",
            Error::ApiUrlNotOrigin => "mango.api_url_not_origin",
            Error::ApiUrlNotHttps => "mango.api_url_not_https",
            Error::InvalidProfileId => "mango.invalid_profile_id",
            Error::Io { .. } => "mango.io",
            Error::Core(_) => "mango.persistence",
            Error::Json(_) => "mango.json",
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_stable() {
        assert_eq!(Error::ApiUrlNotHttps.code(), "mango.api_url_not_https");
        assert_eq!(Error::InvalidProfileId.code(), "mango.invalid_profile_id");
    }
}
