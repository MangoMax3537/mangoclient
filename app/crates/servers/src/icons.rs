//! Durable partner favicon store (`server-icons.json`).

use serde_json::{Map, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_ICON_LENGTH: usize = 2 * 1024 * 1024;

/// Lowercase, trim, strip one trailing dot and a trailing `:25565`.
pub fn normalize_address(address: &str) -> String {
    let lower = address.trim().to_lowercase();
    let s = lower.strip_suffix('.').unwrap_or(&lower);
    s.strip_suffix(":25565").unwrap_or(s).to_string()
}

/// `^data:image/png;base64,[A-Za-z0-9+/]+={0,2}$`, at most 2 MB.
pub fn valid_icon(value: &str) -> bool {
    if value.len() > MAX_ICON_LENGTH {
        return false;
    }
    let Some(body) = value.strip_prefix("data:image/png;base64,") else {
        return false;
    };
    let data = body.trim_end_matches('=');
    let pad = body.len() - data.len();
    pad <= 2
        && !data.is_empty()
        && data
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'/')
}

/// Abstraction over the icon store so list code can be tested with fakes.
pub trait IconStore: Send + Sync {
    fn get(&self, address: &str) -> Option<String>;
    /// Keep the first valid icon; returns the stored one.
    fn remember(&self, address: &str, icon: &str) -> Option<String>;
}

/// An icon store that never has or keeps anything.
pub struct NoIcons;

impl IconStore for NoIcons {
    fn get(&self, _: &str) -> Option<String> {
        None
    }
    fn remember(&self, _: &str, _: &str) -> Option<String> {
        None
    }
}

/// File-backed store. The first valid favicon per address wins forever.
pub struct ServerIcons {
    file: PathBuf,
    cache: Mutex<Option<HashMap<String, String>>>,
}

impl ServerIcons {
    pub fn new(file: impl Into<PathBuf>) -> Self {
        Self {
            file: file.into(),
            cache: Mutex::new(None),
        }
    }

    /// Store at `<root>/server-icons.json`.
    pub fn in_root(root: &Path) -> Self {
        Self::new(root.join("server-icons.json"))
    }

    fn load(&self) -> HashMap<String, String> {
        let mut out = HashMap::new();
        let saved = std::fs::read(&self.file)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        if let Some(Value::Object(map)) = saved {
            for (addr, icon) in map {
                if let Some(icon) = icon.as_str().filter(|i| valid_icon(i)) {
                    out.insert(normalize_address(&addr), icon.to_string());
                }
            }
        }
        out
    }

    fn save(&self, cache: &HashMap<String, String>) {
        let mut keys: Vec<_> = cache.keys().collect();
        keys.sort();
        let mut map = Map::new();
        for k in keys {
            map.insert(k.clone(), Value::String(cache[k].clone()));
        }
        let Ok(mut text) = serde_json::to_string_pretty(&Value::Object(map)) else {
            return;
        };
        text.push('\n');
        // Best effort: a failed save must not break pinging.
        let _ = crate::fsutil::write_file_atomic(&self.file, text.as_bytes());
    }
}

impl IconStore for ServerIcons {
    fn get(&self, address: &str) -> Option<String> {
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let cache = guard.get_or_insert_with(|| self.load());
        cache.get(&normalize_address(address)).cloned()
    }

    fn remember(&self, address: &str, icon: &str) -> Option<String> {
        let key = normalize_address(address);
        let mut guard = self.cache.lock().unwrap_or_else(|e| e.into_inner());
        let cache = guard.get_or_insert_with(|| self.load());
        if key.is_empty() || !valid_icon(icon) {
            return cache.get(&key).cloned();
        }
        if let Some(existing) = cache.get(&key) {
            return Some(existing.clone());
        }
        cache.insert(key, icon.to_string());
        self.save(cache);
        Some(icon.to_string())
    }
}
