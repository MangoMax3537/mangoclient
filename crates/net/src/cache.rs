use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct Entry {
    expected: String,
    size: u64,
    #[serde(rename = "mtimeMs")]
    mtime_ms: f64,
    #[serde(rename = "ctimeMs")]
    ctime_ms: f64,
}

#[derive(Default)]
struct State {
    entries: HashMap<String, Entry>,
    dirty: bool,
}

/// Remembers that a file already verified against an expected hash, keyed by
/// path and invalidated when size, mtime or ctime change. Only paths under
/// `root` are tracked. Call [`flush`](Self::flush) to persist (the legacy
/// code debounced writes by 250ms; callers decide when to flush here).
pub struct IntegrityCache {
    file: PathBuf,
    root: PathBuf,
    state: Mutex<State>,
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn to_ms(t: std::time::SystemTime) -> f64 {
    t.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// (mtimeMs, ctimeMs) for a file. ctime falls back to the creation time
/// where the platform has no change time.
fn times(meta: &std::fs::Metadata) -> (f64, f64) {
    let mtime = meta.modified().map(to_ms).unwrap_or(0.0);
    #[cfg(unix)]
    let ctime = {
        use std::os::unix::fs::MetadataExt;
        meta.ctime() as f64 * 1000.0 + meta.ctime_nsec() as f64 / 1e6
    };
    #[cfg(not(unix))]
    let ctime = meta.created().map(to_ms).unwrap_or(0.0);
    (mtime, ctime)
}

impl IntegrityCache {
    /// Load the cache file (missing or corrupt files give an empty cache).
    pub async fn load(file: impl Into<PathBuf>, root: impl Into<PathBuf>) -> Self {
        let file = file.into();
        let entries = tokio::fs::read(&file)
            .await
            .ok()
            .and_then(|raw| serde_json::from_slice::<HashMap<String, Entry>>(&raw).ok())
            .unwrap_or_default();
        Self {
            file,
            root: normalize(&root.into()),
            state: Mutex::new(State {
                entries,
                dirty: false,
            }),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn managed(&self, path: &Path) -> Option<String> {
        let p = normalize(path);
        p.starts_with(&self.root)
            .then(|| p.to_string_lossy().into_owned())
    }

    /// True when `path` was previously verified for `expected` and is unchanged.
    pub fn is_verified(&self, path: &Path, meta: &std::fs::Metadata, expected: &str) -> bool {
        let Some(key) = self.managed(path) else {
            return false;
        };
        let (mtime_ms, ctime_ms) = times(meta);
        self.lock().entries.get(&key).is_some_and(|e| {
            e.expected == expected
                && e.size == meta.len()
                && e.mtime_ms == mtime_ms
                && e.ctime_ms == ctime_ms
        })
    }

    pub fn remember(&self, path: &Path, meta: &std::fs::Metadata, expected: &str) {
        if expected.is_empty() {
            return;
        }
        let Some(key) = self.managed(path) else {
            return;
        };
        let (mtime_ms, ctime_ms) = times(meta);
        let mut st = self.lock();
        st.entries.insert(
            key,
            Entry {
                expected: expected.to_string(),
                size: meta.len(),
                mtime_ms,
                ctime_ms,
            },
        );
        st.dirty = true;
    }

    /// Persist if changed (temp file + rename). Errors are returned but are
    /// safe to ignore: the cache is only an optimization.
    pub async fn flush(&self) -> std::io::Result<()> {
        let json = {
            let mut st = self.lock();
            if !st.dirty {
                return Ok(());
            }
            st.dirty = false;
            serde_json::to_vec(&st.entries).map_err(std::io::Error::other)?
        };
        if let Some(dir) = self.file.parent() {
            tokio::fs::create_dir_all(dir).await?;
        }
        let mut tmp = self.file.clone().into_os_string();
        tmp.push(format!(".{}.tmp", std::process::id()));
        let tmp = PathBuf::from(tmp);
        let res = async {
            tokio::fs::write(&tmp, &json).await?;
            tokio::fs::rename(&tmp, &self.file).await
        }
        .await;
        if res.is_err() {
            let _ = tokio::fs::remove_file(&tmp).await;
            self.lock().dirty = true;
        }
        res
    }
}
