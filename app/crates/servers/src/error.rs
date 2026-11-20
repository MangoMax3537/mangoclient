use std::io;

/// Errors surfaced by the servers crate.
#[derive(Debug, thiserror::Error)]
pub enum ServersError {
    #[error("i/o error: {0}")]
    Io(#[from] io::Error),
    #[error("truncated NBT data")]
    Truncated,
    #[error("invalid NBT collection length")]
    InvalidLength,
    #[error("unsupported NBT tag {0}")]
    UnsupportedTag(u8),
    #[error("invalid modified UTF-8")]
    InvalidModifiedUtf8,
    #[error("NBT string is too long")]
    StringTooLong,
    #[error("NBT nesting is too deep")]
    TooDeep,
    #[error("invalid NBT: {0}")]
    InvalidNbt(&'static str),
    #[error("invalid Minecraft servers list")]
    InvalidServerList,
    #[error("background task failed")]
    Task,
}

impl ServersError {
    /// Stable machine-readable code for the UI layer.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io(_) => "servers.io",
            Self::Truncated
            | Self::InvalidLength
            | Self::UnsupportedTag(_)
            | Self::InvalidModifiedUtf8
            | Self::StringTooLong
            | Self::TooDeep
            | Self::InvalidNbt(_) => "servers.nbt_invalid",
            Self::InvalidServerList => "servers.list_invalid",
            Self::Task => "servers.task_failed",
        }
    }
}

pub type Result<T> = std::result::Result<T, ServersError>;
