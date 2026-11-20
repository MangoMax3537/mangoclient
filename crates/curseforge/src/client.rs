//! CurseForge Core API v1 client.
//!
//! The API key lives in an [`ApiKey`] whose `Debug` is redacted, is sent only
//! to the API base (never to a download CDN) and never appears in an error.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use crate::error::{Error, Result};
use crate::html::html_to_text;
use crate::types::{Envelope, File, Pagination, Project, SearchQuery, GAME_ID};

pub const API: &str = "https://api.curseforge.com/v1";
const CACHE_TTL: Duration = Duration::from_secs(5 * 60);
const CACHE_CAP: usize = 200;
const FILES_PAGE: u32 = 50;
const MAX_FILE_PAGES: u32 = 20;
/// `POST /mods/files` accepts big lists; stay well below any limit.
const BATCH: usize = 200;

/// A validated API key. `Debug`/`Display` never show it.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiKey(Arc<str>);

impl ApiKey {
    /// Keys are bcrypt-style strings (`$2a$10$...`): visible ASCII without
    /// whitespace or quotes, 16 to 128 characters.
    pub fn is_valid_format(raw: &str) -> bool {
        (16..=128).contains(&raw.len())
            && raw
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"$./_+=-".contains(&b))
    }

    pub fn new(raw: &str) -> Result<ApiKey> {
        let raw = raw.trim();
        if Self::is_valid_format(raw) {
            Ok(ApiKey(raw.into()))
        } else {
            Err(Error::InvalidApiKey)
        }
    }

    fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    pub retries: u32,
    pub backoff: Duration,
    pub timeout: Duration,
    /// Only accept download URLs on CurseForge's own CDN over https. Tests
    /// that serve files from localhost switch this off.
    pub strict_download_urls: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            retries: 2,
            backoff: Duration::from_millis(400),
            timeout: Duration::from_secs(30),
            strict_download_urls: true,
        }
    }
}

struct Inner {
    base: String,
    key: ApiKey,
    http: reqwest::Client,
    opts: Options,
    cache: Mutex<HashMap<String, (Instant, Value)>>,
}

/// Cheap to clone; clones share the cache.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client")
            .field("base", &self.inner.base)
            .field("key", &self.inner.key)
            .finish()
    }
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

/// Hosts CurseForge serves files and images from.
pub fn is_trusted_cdn_url(raw: &str) -> bool {
    // Parsed the way the HTTP client will parse it (so `\` and other
    // spellings cannot disagree with a hand-written splitter).
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return false;
    }
    let Some(host) = url.host_str() else {
        return false;
    };
    let host = host.to_ascii_lowercase();
    ["forgecdn.net", "curseforge.com"]
        .iter()
        .any(|d| host == *d || host.ends_with(&format!(".{d}")))
}

impl Client {
    pub fn new(key: ApiKey) -> Result<Client> {
        Self::with_base(key, API, Options::default())
    }

    pub fn with_base(key: ApiKey, base: impl Into<String>, opts: Options) -> Result<Client> {
        let base: String = base.into();
        let http = reqwest::Client::builder()
            .user_agent(mango_net::UA)
            .timeout(opts.timeout)
            .build()
            .map_err(|e| Error::Request(e.without_url().to_string()))?;
        Ok(Client {
            inner: Arc::new(Inner {
                base: base.trim_end_matches('/').to_string(),
                key,
                http,
                opts,
                cache: Mutex::new(HashMap::new()),
            }),
        })
    }

    pub fn base(&self) -> &str {
        &self.inner.base
    }

    pub fn options(&self) -> &Options {
        &self.inner.opts
    }

    async fn send(&self, req: reqwest::RequestBuilder, path: &str) -> Result<Value> {
        let o = &self.inner.opts;
        let mut attempt = 0u32;
        loop {
            let Some(req) = req.try_clone() else {
                return Err(Error::Request("request cannot be repeated".into()));
            };
            let res = req
                .header("x-api-key", self.inner.key.expose())
                .header("Accept", "application/json")
                .send()
                .await;
            let retry = match &res {
                Err(_) => true,
                Ok(r) => r.status().is_server_error() || r.status().as_u16() == 429,
            };
            if retry && attempt < o.retries {
                tokio::time::sleep(o.backoff * 2u32.saturating_pow(attempt)).await;
                attempt += 1;
                continue;
            }
            let res = res.map_err(|e| Error::Request(e.without_url().to_string()))?;
            let status = res.status();
            if status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(Error::Unauthorized);
            }
            if !status.is_success() {
                return Err(Error::Status {
                    status: status.as_u16(),
                    path: path.to_string(),
                });
            }
            return res.json::<Value>().await.map_err(|e| Error::BadResponse {
                path: path.to_string(),
                message: e.without_url().to_string(),
            });
        }
    }

    fn cache_get(&self, path: &str) -> Option<Value> {
        let c = self.inner.cache.lock().unwrap_or_else(|e| e.into_inner());
        c.get(path)
            .filter(|(at, _)| at.elapsed() < CACHE_TTL)
            .map(|(_, v)| v.clone())
    }

    fn cache_put(&self, path: &str, v: &Value) {
        let mut c = self.inner.cache.lock().unwrap_or_else(|e| e.into_inner());
        if c.len() >= CACHE_CAP {
            c.retain(|_, (at, _)| at.elapsed() < CACHE_TTL);
            if c.len() >= CACHE_CAP {
                c.clear();
            }
        }
        c.insert(path.to_string(), (Instant::now(), v.clone()));
    }

    async fn get_value(&self, path: &str) -> Result<Value> {
        if let Some(v) = self.cache_get(path) {
            return Ok(v);
        }
        let url = format!("{}{path}", self.inner.base);
        let v = self.send(self.inner.http.get(url), path).await?;
        self.cache_put(path, &v);
        Ok(v)
    }

    fn parse<T: DeserializeOwned>(path: &str, v: Value) -> Result<Envelope<T>> {
        serde_json::from_value(v).map_err(|e| Error::BadResponse {
            path: path.to_string(),
            message: e.to_string(),
        })
    }

    async fn get<T: DeserializeOwned>(&self, path: &str) -> Result<Envelope<T>> {
        Self::parse(path, self.get_value(path).await?)
    }

    async fn post<T: DeserializeOwned>(&self, path: &str, body: &Value) -> Result<Envelope<T>> {
        let url = format!("{}{path}", self.inner.base);
        let v = self
            .send(self.inner.http.post(url).json(body), path)
            .await?;
        Self::parse(path, v)
    }

    /// `GET /mods/search`.
    pub async fn search(&self, q: &SearchQuery) -> Result<(Vec<Project>, Pagination)> {
        let mut pairs: Vec<(&str, String)> = vec![
            ("gameId", GAME_ID.to_string()),
            ("classId", q.class.id().to_string()),
            ("sortField", q.sort.id().to_string()),
            ("sortOrder", "desc".into()),
            ("index", q.index.to_string()),
            ("pageSize", q.page_size.clamp(1, 50).to_string()),
        ];
        if !q.filter.is_empty() {
            pairs.push(("searchFilter", q.filter.clone()));
        }
        if let Some(v) = q.game_version.as_deref().filter(|v| !v.is_empty()) {
            pairs.push(("gameVersion", v.to_string()));
        }
        if let Some(l) = q.loader {
            pairs.push(("modLoaderType", l.id().to_string()));
        }
        let path = format!("/mods/search?{}", form_encode(&pairs));
        let env: Envelope<Vec<Project>> = self.get(&path).await?;
        Ok((env.data, env.pagination.unwrap_or_default()))
    }

    pub async fn project(&self, id: u64) -> Result<Project> {
        let env: Envelope<Project> = self.get(&format!("/mods/{id}")).await?;
        Ok(env.data)
    }

    /// `POST /mods`: several projects at once (order not guaranteed).
    pub async fn projects(&self, ids: &[u64]) -> Result<Vec<Project>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(BATCH) {
            let env: Envelope<Vec<Project>> = self
                .post("/mods", &json!({"modIds": chunk, "filterPcOnly": true}))
                .await?;
            out.extend(env.data);
        }
        Ok(out)
    }

    /// The project's description as markdown-lite text. The API returns HTML,
    /// which is never passed on.
    pub async fn description(&self, id: u64) -> Result<String> {
        let env: Envelope<String> = self.get(&format!("/mods/{id}/description")).await?;
        Ok(html_to_text(&env.data))
    }

    /// All files of a project (paged), optionally narrowed to a game version.
    pub async fn files(&self, mod_id: u64, game_version: Option<&str>) -> Result<Vec<File>> {
        let mut out: Vec<File> = Vec::new();
        for page in 0..MAX_FILE_PAGES {
            let mut pairs: Vec<(&str, String)> = vec![
                ("index", (page * FILES_PAGE).to_string()),
                ("pageSize", FILES_PAGE.to_string()),
            ];
            if let Some(v) = game_version.filter(|v| !v.is_empty()) {
                pairs.push(("gameVersion", v.to_string()));
            }
            let path = format!("/mods/{mod_id}/files?{}", form_encode(&pairs));
            let env: Envelope<Vec<File>> = self.get(&path).await?;
            let got = env.data.len();
            out.extend(env.data);
            let total = env.pagination.map_or(0, |p| p.total_count);
            if got == 0 || out.len() as u64 >= total {
                break;
            }
        }
        Ok(out)
    }

    pub async fn file(&self, mod_id: u64, file_id: u64) -> Result<File> {
        let env: Envelope<File> = self.get(&format!("/mods/{mod_id}/files/{file_id}")).await?;
        Ok(env.data)
    }

    /// `POST /mods/files`: several files at once (order not guaranteed).
    pub async fn files_by_ids(&self, ids: &[u64]) -> Result<Vec<File>> {
        let mut out = Vec::new();
        for chunk in ids.chunks(BATCH) {
            let env: Envelope<Vec<File>> =
                self.post("/mods/files", &json!({"fileIds": chunk})).await?;
            out.extend(env.data);
        }
        Ok(out)
    }

    /// The download endpoint for a file whose `downloadUrl` is null. A refusal
    /// (403/404) means the author does not allow third-party downloads.
    pub async fn download_url(&self, mod_id: u64, file_id: u64) -> Result<Option<String>> {
        let path = format!("/mods/{mod_id}/files/{file_id}/download-url");
        let url = format!("{}{path}", self.inner.base);
        match self.send(self.inner.http.get(url), &path).await {
            Ok(v) => Ok(Self::parse::<Option<String>>(&path, v)?.data),
            Err(Error::Unauthorized) | Err(Error::Status { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn url_ok(&self, url: &str) -> bool {
        !self.inner.opts.strict_download_urls || is_trusted_cdn_url(url)
    }

    /// A URL the file may be downloaded from, or `None` when distribution is
    /// blocked: the project forbids it, there is no URL even from the
    /// download endpoint, or the URL is not on CurseForge's CDN. Nothing here
    /// ever builds a CDN URL by hand: that would circumvent an author's
    /// opt-out.
    pub async fn resolve_download_url(
        &self,
        project: Option<&Project>,
        file: &File,
    ) -> Result<Option<String>> {
        if project.is_some_and(Project::distribution_blocked) || !file.is_available() {
            return Ok(None);
        }
        if let Some(url) = file.download_url.as_deref().filter(|u| !u.is_empty()) {
            return Ok(self.url_ok(url).then(|| url.to_string()));
        }
        let fetched = self.download_url(file.mod_id, file.id).await?;
        Ok(fetched.filter(|u| !u.is_empty() && self.url_ok(u)))
    }
}
