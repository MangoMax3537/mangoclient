use crate::cache::IntegrityCache;
use crate::error::NetError;
use crate::hash::{hash_file, HashAlgo, Hasher};
use crate::http::{fetch_with_retry, FetchOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::AsyncWriteExt;

/// Progress callback: (bytes done, total). Total is the expected size, else
/// the advertised length, else 0. Throttling is up to the caller.
pub type ProgressFn = Arc<dyn Fn(u64, u64) + Send + Sync>;

#[derive(Debug, Clone, Default)]
pub struct Expected {
    pub sha1: Option<String>,
    pub sha256: Option<String>,
    pub sha512: Option<String>,
    pub size: Option<u64>,
}

impl Expected {
    fn key(&self) -> String {
        let s = |v: &Option<String>| v.clone().unwrap_or_default();
        let size = self.size.map(|n| n.to_string()).unwrap_or_default();
        format!(
            "{}|{}|{}|{}",
            s(&self.sha512),
            s(&self.sha256),
            s(&self.sha1),
            size
        )
        .to_lowercase()
    }

    fn has_hash(&self) -> bool {
        self.sha1.is_some() || self.sha256.is_some() || self.sha512.is_some()
    }
}

#[derive(Default, Clone)]
pub struct DownloadOptions {
    pub expected: Expected,
    pub progress: Option<ProgressFn>,
    pub cache: Option<Arc<IntegrityCache>>,
    pub fetch: FetchOptions,
}

#[derive(Debug, Clone)]
pub struct DownloadOutcome {
    pub skipped: bool,
    pub dest: PathBuf,
    pub bytes: u64,
}

/// Whether `path` exists and matches the expected size and hash. Without any
/// expected hash a non-empty file counts as valid. Errors mean invalid.
pub async fn is_valid(path: &Path, expected: &Expected, cache: Option<&IntegrityCache>) -> bool {
    let Ok(meta) = tokio::fs::metadata(path).await else {
        return false;
    };
    if expected.size.is_some_and(|s| s != meta.len()) {
        return false;
    }
    if !expected.has_hash() {
        return meta.len() > 0;
    }
    let key = expected.key();
    if cache.is_some_and(|c| c.is_verified(path, &meta, &key)) {
        return true;
    }
    let (algo, want) = if let Some(h) = &expected.sha512 {
        (HashAlgo::Sha512, h)
    } else if let Some(h) = &expected.sha256 {
        (HashAlgo::Sha256, h)
    } else if let Some(h) = &expected.sha1 {
        (HashAlgo::Sha1, h)
    } else {
        return false;
    };
    let valid = hash_file(path, algo)
        .await
        .is_ok_and(|got| got == want.to_lowercase());
    if valid {
        if let Some(c) = cache {
            c.remember(path, &meta, &key);
        }
    }
    valid
}

/// Only a positive advertised length is authoritative: some Mojang CDN
/// responses expose a placeholder `Content-Length: 0` on a normal body.
pub fn check_advertised_length(
    url: &str,
    expected: Option<u64>,
    advertised: Option<u64>,
) -> Result<(), NetError> {
    match (expected, advertised) {
        (Some(expected), Some(advertised)) if advertised > 0 && advertised != expected => {
            Err(NetError::AdvertisedSizeMismatch {
                url: url.into(),
                expected,
                advertised,
            })
        }
        _ => Ok(()),
    }
}

/// Deletes the temp file on drop unless disarmed, so failures and
/// cancellation never leave a truncated `.part-*` file behind.
struct TempGuard {
    path: PathBuf,
    armed: bool,
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if self.armed {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Download `url` to `dest`, skipping the transfer when a valid copy exists.
/// Data goes through `<dest>.part-*`, is verified (sha512, sha256, sha1 as
/// provided, then exact size) and atomically renamed into place.
pub async fn download_file(
    url: &str,
    dest: &Path,
    opts: &DownloadOptions,
) -> Result<DownloadOutcome, NetError> {
    let exp = &opts.expected;
    if is_valid(dest, exp, opts.cache.as_deref()).await {
        if let (Some(cb), Some(size)) = (&opts.progress, exp.size) {
            cb(size, size);
        }
        return Ok(DownloadOutcome {
            skipped: true,
            dest: dest.to_path_buf(),
            bytes: 0,
        });
    }
    if let Some(dir) = dest.parent() {
        tokio::fs::create_dir_all(dir).await?;
    }
    let mut tmp_name = dest.as_os_str().to_owned();
    tmp_name.push(format!(
        ".part-{}-{}",
        std::process::id(),
        uuid::Uuid::new_v4().simple()
    ));
    let tmp = PathBuf::from(tmp_name);

    let mut res = fetch_with_retry(url, &opts.fetch).await?;
    if !res.status().is_success() {
        return Err(NetError::Status {
            status: res.status().as_u16(),
            url: url.into(),
        });
    }
    let advertised = res.content_length();
    check_advertised_length(url, exp.size, advertised)?;
    let total = exp.size.or(advertised).unwrap_or(0);

    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .await?;
    let mut guard = TempGuard {
        path: tmp.clone(),
        armed: true,
    };

    let mut hashers: Vec<(HashAlgo, &str, Hasher)> = Vec::new();
    for (algo, want) in [
        (HashAlgo::Sha512, &exp.sha512),
        (HashAlgo::Sha256, &exp.sha256),
        (HashAlgo::Sha1, &exp.sha1),
    ] {
        if let Some(w) = want {
            hashers.push((algo, w.as_str(), Hasher::new(algo)));
        }
    }

    let mut counted: u64 = 0;
    loop {
        let chunk = res.chunk().await.map_err(|source| NetError::Request {
            url: url.into(),
            source,
        })?;
        let Some(chunk) = chunk else { break };
        counted += chunk.len() as u64;
        if exp.size.is_some_and(|s| counted > s) {
            return Err(NetError::Oversized {
                url: url.into(),
                expected: exp.size.unwrap_or(0),
            });
        }
        for (_, _, h) in hashers.iter_mut() {
            h.update(&chunk);
        }
        file.write_all(&chunk).await?;
        if let Some(cb) = &opts.progress {
            cb(counted, total);
        }
    }
    file.flush().await?;
    file.sync_all().await?;
    drop(file);

    for (algo, want, hasher) in hashers {
        let actual = hasher.finish_hex();
        if actual != want.to_lowercase() {
            return Err(NetError::ChecksumMismatch {
                url: url.into(),
                algo: algo.name(),
                expected: want.to_string(),
                actual,
            });
        }
    }
    if let Some(expected) = exp.size {
        if counted != expected {
            return Err(NetError::SizeMismatch {
                url: url.into(),
                expected,
                actual: counted,
            });
        }
    }

    tokio::fs::rename(&tmp, dest).await?;
    guard.armed = false;

    if let Some(c) = &opts.cache {
        if let Ok(meta) = tokio::fs::metadata(dest).await {
            c.remember(dest, &meta, &exp.key());
        }
    }
    Ok(DownloadOutcome {
        skipped: false,
        dest: dest.to_path_buf(),
        bytes: counted,
    })
}
