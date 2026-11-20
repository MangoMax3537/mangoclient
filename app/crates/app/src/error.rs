use serde::Serialize;

pub type Res<T> = Result<T, AppError>;

/// The error every command returns. `code` is the stable key the UI localizes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, thiserror::Error)]
#[error("{code}: {message}")]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub needs_relogin: bool,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        let code = code.into();
        AppError {
            needs_relogin: code == "auth.needs_relogin",
            code,
            message: message.into(),
        }
    }

    pub fn not_implemented(what: &str) -> Self {
        AppError::new("not_implemented", format!("{what} is not implemented yet"))
    }

    pub fn profile_not_found() -> Self {
        AppError::new("profile_not_found", "Profile not found")
    }

    pub fn already_running() -> Self {
        AppError::new("already_running", "This profile is already running")
    }
}

macro_rules! from_coded {
    ($($ty:ty),* $(,)?) => {$(
        impl From<$ty> for AppError {
            fn from(e: $ty) -> Self {
                AppError::new(e.code(), e.to_string())
            }
        }
    )*};
}

from_coded!(
    mango_core::Error,
    mango_java::JavaError,
    mango_minecraft::MinecraftError,
    mango_loaders::LoaderError,
    mango_content::ContentError,
    mango_auth::AuthError,
    mango_runtime::RuntimeError,
    mango_net::NetError,
    mango_mango::Error,
);

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        AppError::new("io", e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        AppError::new("json", e.to_string())
    }
}

impl From<tokio::task::JoinError> for AppError {
    fn from(e: tokio::task::JoinError) -> Self {
        AppError::new("task_failed", e.to_string())
    }
}
