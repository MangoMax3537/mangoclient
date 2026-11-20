use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, RuntimeError>;

/// Errors from spawning, stopping and following a game process.
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("could not prepare the game log {}: {source}", path.display())]
    LogFile {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("could not start the game ({}): {source}", program.display())]
    Spawn {
        program: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("process {0} is not a Java process; refusing to signal it")]
    NotJava(u32),
    #[error("could not stop process {pid}: {reason}")]
    Stop { pid: u32, reason: String },
    #[error("session {0} is no longer running")]
    NotAlive(u32),
}

impl RuntimeError {
    /// Stable code for the UI.
    pub fn code(&self) -> &'static str {
        match self {
            RuntimeError::LogFile { .. } => "log_file",
            RuntimeError::Spawn { .. } => "spawn_failed",
            RuntimeError::NotJava(_) => "not_java",
            RuntimeError::Stop { .. } => "stop_failed",
            RuntimeError::NotAlive(_) => "not_alive",
        }
    }
}
