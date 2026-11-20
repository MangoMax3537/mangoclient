use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum JavaError {
    #[error("Configured Java path is not runnable: {}", .0.display())]
    NotRunnable(PathBuf),
    #[error("Minecraft needs Java {required}, but the configured Java is {found}")]
    TooOld { required: u32, found: u32 },
    #[error("No Java {major} runtime is published for {os}/{arch}")]
    NoRuntime {
        major: u32,
        os: &'static str,
        arch: &'static str,
    },
    #[error("Java API returned an invalid filename: {0:?}")]
    InvalidPackageName(String),
    #[error("Java runtime extracted but no executable was found")]
    NoExecutable,
    #[error(transparent)]
    Net(#[from] mango_net::NetError),
    #[error(transparent)]
    Archive(#[from] mango_archive::ArchiveError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("worker task failed: {0}")]
    Join(String),
}

impl JavaError {
    /// Stable identifier for the UI to localize.
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotRunnable(_) => "java.not_runnable",
            Self::TooOld { .. } => "java.too_old",
            Self::NoRuntime { .. } => "java.no_runtime",
            Self::InvalidPackageName(_) => "java.invalid_package_name",
            Self::NoExecutable => "java.no_executable",
            Self::Net(e) => e.code(),
            Self::Archive(_) => "java.archive",
            Self::Io(_) => "java.io",
            Self::Join(_) => "java.join",
        }
    }
}
