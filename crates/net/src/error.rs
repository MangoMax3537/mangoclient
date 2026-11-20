use thiserror::Error;

/// Errors surfaced by the network layer. `code()` gives a stable identifier
/// for the UI to localize.
#[derive(Debug, Error)]
pub enum NetError {
    #[error("HTTP {status} for {url}")]
    Status { status: u16, url: String },
    #[error("request failed for {url}: {source}")]
    Request {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("request timed out for {url}")]
    Timeout { url: String },
    #[error("could not build HTTP client: {0}")]
    Client(#[source] reqwest::Error),
    #[error("size mismatch for {url}: expected {expected} bytes, server advertised {advertised}")]
    AdvertisedSizeMismatch {
        url: String,
        expected: u64,
        advertised: u64,
    },
    #[error("size mismatch for {url}: expected {expected} bytes, got {actual}")]
    SizeMismatch {
        url: String,
        expected: u64,
        actual: u64,
    },
    #[error("oversized download for {url}: expected {expected} bytes")]
    Oversized { url: String, expected: u64 },
    #[error("checksum mismatch for {url} ({algo}): expected {expected}, got {actual}")]
    ChecksumMismatch {
        url: String,
        algo: &'static str,
        expected: String,
        actual: String,
    },
    #[error("invalid JSON from {url}: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("worker task failed: {0}")]
    Join(String),
}

impl NetError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Status { .. } => "net.http_status",
            Self::Request { .. } => "net.request",
            Self::Timeout { .. } => "net.timeout",
            Self::Client(_) => "net.client",
            Self::AdvertisedSizeMismatch { .. } | Self::SizeMismatch { .. } => "net.size_mismatch",
            Self::Oversized { .. } => "net.oversized",
            Self::ChecksumMismatch { .. } => "net.checksum_mismatch",
            Self::Json { .. } => "net.json",
            Self::Io(_) => "net.io",
            Self::Join(_) => "net.join",
        }
    }
}
