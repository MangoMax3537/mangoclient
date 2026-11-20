//! HTTP client, verified downloads, integrity cache and bounded concurrency.
//!
//! Port of `src/main/net.js`. No Tauri dependency.

mod cache;
mod download;
mod error;
mod hash;
mod http;
mod pool;

#[cfg(test)]
mod tests;

pub use cache::IntegrityCache;
pub use download::{
    check_advertised_length, download_file, is_valid, DownloadOptions, DownloadOutcome, Expected,
    ProgressFn,
};
pub use error::NetError;
pub use hash::{hash_file, HashAlgo};
pub use http::{fetch_with_retry, get_bytes, get_json, FetchOptions, UA};
pub use pool::pool;
