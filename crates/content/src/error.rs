use std::path::PathBuf;

/// Errors surfaced by `mango-content`. `code()` is the stable code for the UI.
#[derive(Debug, thiserror::Error)]
pub enum ContentError {
    #[error(transparent)]
    Core(#[from] mango_core::error::Error),
    #[error("I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("not a local mod")]
    NotLocal,
    #[error("mod has no file")]
    NoFile,
    #[error("lookup failed: {0}")]
    Lookup(String),
}

impl ContentError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        ContentError::Io {
            path: path.into(),
            source,
        }
    }

    pub fn code(&self) -> &'static str {
        match self {
            ContentError::Core(e) => e.code(),
            ContentError::Io { .. } => "io",
            ContentError::NotLocal => "not_local",
            ContentError::NoFile => "no_file",
            ContentError::Lookup(_) => "lookup",
        }
    }
}
