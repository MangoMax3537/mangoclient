use thiserror::Error;

/// Errors from version resolution, installation and command building.
/// `code()` is the stable identifier the UI localizes.
#[derive(Debug, Error)]
pub enum MinecraftError {
    #[error(transparent)]
    Net(#[from] mango_net::NetError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON in {what}: {source}")]
    Json {
        what: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("unknown Minecraft version: {0}")]
    UnknownVersion(String),
    #[error("circular version inheritance at {0}")]
    CircularInheritance(String),
    #[error("unsafe version id: {0}")]
    InvalidVersionId(String),
    #[error("worker task failed: {0}")]
    Join(String),
}

impl MinecraftError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Net(_) => "minecraft.net",
            Self::Io(_) => "minecraft.io",
            Self::Json { .. } => "minecraft.json",
            Self::UnknownVersion(_) => "minecraft.unknown_version",
            Self::CircularInheritance(_) => "minecraft.circular_inheritance",
            Self::InvalidVersionId(_) => "minecraft.invalid_version_id",
            Self::Join(_) => "minecraft.join",
        }
    }
}

pub type Result<T> = std::result::Result<T, MinecraftError>;
