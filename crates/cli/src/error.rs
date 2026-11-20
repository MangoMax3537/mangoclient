use std::fmt::Display;

/// Everything the CLI can fail with. Messages are printed to stderr as is,
/// so they must never contain tokens.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Bad command line: exit code 2.
    #[error("{0}")]
    Usage(String),
    #[error("{0}")]
    Failure(String),
    /// The game started but crashed; the RESULT line is already printed.
    #[error("the game crashed (exit code {0})")]
    Crashed(String),
    #[error("{0}")]
    Timeout(String),
}

impl CliError {
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::Usage(_) => 2,
            _ => 1,
        }
    }
}

pub type Res<T> = Result<T, CliError>;

pub fn usage<T>(msg: impl Into<String>) -> Res<T> {
    Err(CliError::Usage(msg.into()))
}

/// `.map_err(fail("loading profiles"))`
pub fn fail<E: Display>(context: &'static str) -> impl FnOnce(E) -> CliError {
    move |e| CliError::Failure(format!("{context}: {e}"))
}
