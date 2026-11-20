//! Modrinth API client with an in-memory response cache.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use mango_net::{get_json, FetchOptions, HostPolicy, NetError};
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::OnceCell;

use crate::error::{Error, Result};
use crate::types::{Project, SearchParams, Version, VersionFile};

pub const API: &str = "https://api.modrinth.com/v2";
pub const SEARCH_TTL: Duration = Duration::from_secs(5 * 60);
pub const METADATA_TTL: Duration = Duration::from_secs(15 * 60);
pub const CATEGORY_TTL: Duration = Duration::from_secs(24 * 60 * 60);
const CACHE_CAP: usize = 250;
const HASH_BATCH: usize = 200;

type Flight = Arc<OnceCell<std::result::Result<Value, Arc<NetError>>>>;

#[derive(Default)]
struct Cache {
    map: HashMap<String, (Instant, Value)>,
    /// Least recently used first.
    order: VecDeque<String>,
}

/// Where `.mrpack` files may be downloaded from (the Modrinth pack spec).
pub const MRPACK_DOWNLOAD_HOSTS: [&str; 4] = [
    "cdn.modrinth.com",
    "github.com",
    "raw.githubusercontent.com",
    "gitlab.com",
];

struct Inner {
    base: String,
    fetch: FetchOptions,
    download_policy: HostPolicy,
    cache: Mutex<Cache>,
    pending: Mutex<HashMap<String, Flight>>,
}

/// Cheap to clone; clones share the cache.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl Default for Client {
    fn default() -> Self {
        Self::with_base(API)
    }
}

/// `encodeURIComponent`.
pub(crate) fn encode_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `URLSearchParams` serialization.
fn form_encode(pairs: &[(&str, String)]) -> String {
    let enc = |s: &str| {
        let mut out = String::new();
        for b in s.bytes() {
            if b.is_ascii_alphanumeric() || b"*-._".contains(&b) {
                out.push(b as char);
            } else if b == b' ' {
                out.push('+');
            } else {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        out
    };
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", enc(k), enc(v)))
        .collect::<Vec<_>>()
        .join("&")
}

/// Loaders that can also load plain-Fabric mods.
fn loader_facet(loader: &str) -> Option<Vec<String>> {
    match loader {
        "quilt" => Some(vec!["categories:quilt".into(), "categories:fabric".into()]),
        "neoforge" => Some(vec![
            "categories:neoforge".into(),
            "categories:forge".into(),
        ]),
        "vanilla" => None,
        other => Some(vec![format!("categories:{other}")]),
    }
}

fn loader_query(loader: &str) -> Vec<&str> {
    match loader {
        "quilt" => vec!["quilt", "fabric"],
        "neoforge" => vec!["neoforge", "forge"],
        other => vec![other],
    }
}

impl Client {
    pub fn with_base(base: impl Into<String>) -> Self {
        Self::with_options(base, FetchOptions::default())
    }

    pub fn with_options(base: impl Into<String>, fetch: FetchOptions) -> Self {
        let base: String = base.into();
        Self {
            inner: Arc::new(Inner {
                base: base.trim_end_matches('/').to_string(),
                fetch,
                download_policy: HostPolicy::https_hosts(&MRPACK_DOWNLOAD_HOSTS),
                cache: Mutex::new(Cache::default()),
                pending: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Replace the host allow-list for pack file downloads (tests use
    /// `HostPolicy::allow_all()` with local mock servers). The new client
    /// has its own cache.
    #[must_use]
    pub fn with_download_policy(self, policy: HostPolicy) -> Self {
        Self {
            inner: Arc::new(Inner {
                base: self.inner.base.clone(),
                fetch: self.inner.fetch.clone(),
                download_policy: policy,
                cache: Mutex::new(Cache::default()),
                pending: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// Hosts a modpack's files may come from.
    pub fn download_policy(&self) -> &HostPolicy {
        &self.inner.download_policy
    }

    pub fn base(&self) -> &str {
        &self.inner.base
    }

    fn lookup(&self, url: &str, ttl: Duration) -> Option<Value> {
        let mut c = self.inner.cache.lock().unwrap_or_else(|e| e.into_inner());
        let fresh = c
            .map
            .get(url)
            .filter(|(at, _)| at.elapsed() < ttl)?
            .1
            .clone();
        // Refresh recency so the cap acts as an LRU.
        c.order.retain(|k| k != url);
        c.order.push_back(url.to_string());
        Some(fresh)
    }

    fn store(&self, url: &str, value: &Value) {
        let mut c = self.inner.cache.lock().unwrap_or_else(|e| e.into_inner());
        c.map
            .insert(url.to_string(), (Instant::now(), value.clone()));
        c.order.retain(|k| k != url);
        c.order.push_back(url.to_string());
        while c.map.len() > CACHE_CAP {
            match c.order.pop_front() {
                Some(old) => {
                    c.map.remove(&old);
                }
                None => break,
            }
        }
    }

    /// Cached GET with identical simultaneous requests collapsed into one.
    async fn cached(&self, url: String, ttl: Duration) -> Result<Value> {
        if let Some(v) = self.lookup(&url, ttl) {
            return Ok(v);
        }
        let cell: Flight = {
            let mut p = self.inner.pending.lock().unwrap_or_else(|e| e.into_inner());
            p.entry(url.clone()).or_default().clone()
        };
        let result = cell
            .get_or_init(|| async {
                match get_json::<Value>(&url, &self.inner.fetch).await {
                    Ok(v) => {
                        self.store(&url, &v);
                        Ok(v)
                    }
                    Err(e) => Err(Arc::new(e)),
                }
            })
            .await
            .clone();
        {
            let mut p = self.inner.pending.lock().unwrap_or_else(|e| e.into_inner());
            if p.get(&url).is_some_and(|c| Arc::ptr_eq(c, &cell)) {
                p.remove(&url);
            }
        }
        result.map_err(Error::Net)
    }

    async fn typed<T: DeserializeOwned>(&self, url: String, ttl: Duration) -> Result<T> {
        let value = self.cached(url.clone(), ttl).await?;
        serde_json::from_value(value)
            .map_err(|source| Error::Net(Arc::new(NetError::Json { url, source })))
    }

    /// Raw search response (passed through to the UI).
    pub async fn search(&self, p: &SearchParams) -> Result<Value> {
        let mut facets: Vec<Vec<String>> = vec![vec![format!("project_type:{}", p.project_type)]];
        if p.project_type == "mod" {
            if let Some(lf) = loader_facet(&p.loader) {
                facets.push(lf);
            }
        }
        if !p.game_version.is_empty() {
            facets.push(vec![format!("versions:{}", p.game_version)]);
        }
        for c in &p.categories {
            facets.push(vec![format!("categories:{c}")]);
        }
        if p.open_source_only {
            facets.push(vec!["open_source:true".into()]);
        }
        let qs = form_encode(&[
            ("query", p.query.clone()),
            (
                "facets",
                serde_json::to_string(&facets).unwrap_or_else(|_| "[]".into()),
            ),
            ("limit", p.limit.to_string()),
            ("offset", p.offset.to_string()),
            ("index", p.index.clone()),
        ]);
        self.cached(format!("{}/search?{qs}", self.inner.base), SEARCH_TTL)
            .await
    }

    pub async fn project(&self, id_or_slug: &str) -> Result<Project> {
        self.typed(
            format!(
                "{}/project/{}",
                self.inner.base,
                encode_component(id_or_slug)
            ),
            METADATA_TTL,
        )
        .await
    }

    pub async fn projects(&self, ids: &[String]) -> Result<Vec<Project>> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let json = serde_json::to_string(ids).unwrap_or_else(|_| "[]".into());
        self.typed(
            format!(
                "{}/projects?ids={}",
                self.inner.base,
                encode_component(&json)
            ),
            METADATA_TTL,
        )
        .await
    }

    pub async fn versions(
        &self,
        id_or_slug: &str,
        loader: Option<&str>,
        game_version: Option<&str>,
    ) -> Result<Vec<Version>> {
        let mut pairs: Vec<(&str, String)> = Vec::new();
        if let Some(l) = loader.filter(|l| !l.is_empty() && *l != "vanilla") {
            pairs.push((
                "loaders",
                serde_json::to_string(&loader_query(l)).unwrap_or_default(),
            ));
        }
        if let Some(g) = game_version.filter(|g| !g.is_empty()) {
            pairs.push((
                "game_versions",
                serde_json::to_string(&[g]).unwrap_or_default(),
            ));
        }
        let qs = if pairs.is_empty() {
            String::new()
        } else {
            format!("?{}", form_encode(&pairs))
        };
        self.typed(
            format!(
                "{}/project/{}/version{qs}",
                self.inner.base,
                encode_component(id_or_slug)
            ),
            METADATA_TTL,
        )
        .await
    }

    pub async fn version(&self, version_id: &str) -> Result<Version> {
        self.typed(
            format!(
                "{}/version/{}",
                self.inner.base,
                encode_component(version_id)
            ),
            METADATA_TTL,
        )
        .await
    }

    /// Batch lookup by file hash (`POST /version_files`, algorithm sha1).
    /// Returns the version that owns each hash (keys lowercase); unknown
    /// hashes are simply absent. Never cached.
    pub async fn versions_by_sha1(&self, hashes: &[String]) -> Result<HashMap<String, Version>> {
        let mut out = HashMap::new();
        let mut unique: Vec<String> = hashes.iter().map(|h| h.to_ascii_lowercase()).collect();
        unique.sort();
        unique.dedup();
        for chunk in unique.chunks(HASH_BATCH) {
            let url = format!("{}/version_files", self.inner.base);
            let body = serde_json::json!({"hashes": chunk, "algorithm": "sha1"});
            let found: HashMap<String, Version> =
                mango_net::post_json(&url, &body, &self.inner.fetch)
                    .await
                    .map_err(|e| Error::Net(Arc::new(e)))?;
            out.extend(found.into_iter().map(|(h, v)| (h.to_ascii_lowercase(), v)));
        }
        Ok(out)
    }

    pub async fn categories(&self) -> Result<Value> {
        self.cached(format!("{}/tag/category", self.inner.base), CATEGORY_TTL)
            .await
    }
}

/// Newest release by `date_published`, else the newest of anything.
pub fn pick_version(versions: &[Version]) -> Option<&Version> {
    let mut by_date: Vec<&Version> = versions.iter().collect();
    // RFC 3339 timestamps from Modrinth sort lexicographically; stable sort
    // keeps API order for ties.
    by_date.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    by_date
        .iter()
        .find(|v| v.version_type == "release")
        .or(by_date.first())
        .copied()
}

/// The primary file, else the first.
pub fn primary_file(version: &Version) -> Option<&VersionFile> {
    version
        .files
        .iter()
        .find(|f| f.primary)
        .or(version.files.first())
}
