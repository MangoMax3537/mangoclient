//! Download everything a version needs: client jar, libraries, natives,
//! log config and assets.

use crate::error::{MinecraftError, Result};
use crate::host::HostEnv;
use crate::libraries::{collect_libraries, LibraryFile};
use crate::natives::extract_natives;
use crate::version::{resolve_version, validate_safe_name, RESOURCES_URL, VERSION_MANIFEST_URL};
use mango_core::Paths;
use mango_net::{download_file, is_valid, pool, DownloadOptions, Expected, FetchOptions};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Client,
    Libraries,
    Assets,
    Done,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallProgress {
    pub phase: Phase,
    pub done: usize,
    pub total: usize,
    pub label: String,
}

pub type ProgressCb = Arc<dyn Fn(InstallProgress) + Send + Sync>;
pub type LogCb = Arc<dyn Fn(&str) + Send + Sync>;

/// Base URLs, injectable for tests and mirrors.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub manifest: String,
    pub resources: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            manifest: VERSION_MANIFEST_URL.into(),
            resources: RESOURCES_URL.into(),
        }
    }
}

#[derive(Clone)]
pub struct InstallOptions {
    /// Config `concurrentDownloads` (1..32).
    pub concurrency: usize,
    pub fetch: FetchOptions,
    pub endpoints: Endpoints,
    pub env: HostEnv,
    pub on_progress: Option<ProgressCb>,
    pub on_log: Option<LogCb>,
}

impl Default for InstallOptions {
    fn default() -> Self {
        Self {
            concurrency: 12,
            fetch: FetchOptions::default(),
            endpoints: Endpoints::default(),
            env: HostEnv::current(),
            on_progress: None,
            on_log: None,
        }
    }
}

impl InstallOptions {
    fn log(&self, line: &str) {
        if let Some(cb) = &self.on_log {
            cb(line);
        }
    }
    fn progress(&self, phase: Phase, done: usize, total: usize, label: &str) {
        if let Some(cb) = &self.on_progress {
            cb(InstallProgress {
                phase,
                done,
                total,
                label: label.to_string(),
            });
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallResult {
    /// The flattened version JSON.
    pub version: Value,
    pub client_jar: PathBuf,
    /// Classpath libraries, without the client jar.
    pub classpath: Vec<PathBuf>,
    pub natives_dir: PathBuf,
    pub assets_index_id: String,
    pub logging_arg: Option<String>,
    pub is_legacy_assets: bool,
}

fn dl_opts(expected: Expected, fetch: &FetchOptions) -> DownloadOptions {
    DownloadOptions {
        expected,
        fetch: fetch.clone(),
        ..Default::default()
    }
}

fn sha1_size(sha1: Option<&String>, size: Option<u64>) -> Expected {
    Expected {
        sha1: sha1.cloned(),
        size,
        ..Default::default()
    }
}

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn is_hex_hash(h: &str) -> bool {
    h.len() >= 2 && h.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Relative asset name that stays below its root.
fn safe_asset_name(name: &str) -> bool {
    !name.is_empty()
        && !name.contains(['\0', '\\'])
        && !name.starts_with('/')
        && name
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
        && !name.contains(':')
}

async fn download_libraries(opts: &InstallOptions, libs: Vec<LibraryFile>) -> Result<()> {
    let total = libs.len();
    let done = Arc::new(AtomicUsize::new(0));
    let (o, d) = (opts.clone(), done.clone());
    pool(libs, opts.concurrency, move |lib, _| {
        let (o, d) = (o.clone(), d.clone());
        async move {
            let expected = sha1_size(lib.sha1.as_ref(), lib.size);
            if let Err(err) =
                download_file(&lib.url, &lib.path, &dl_opts(expected.clone(), &o.fetch)).await
            {
                // One optional library must not abort the install.
                if !is_valid(&lib.path, &expected, None).await {
                    o.log(&format!("Library failed: {} ({err})", lib.name));
                }
            }
            let n = d.fetch_add(1, Ordering::SeqCst) + 1;
            o.progress(Phase::Libraries, n, total, &lib.name);
        }
    })
    .await?;
    Ok(())
}

async fn install_assets(paths: &Paths, opts: &InstallOptions, version: &Value) -> Result<String> {
    let asset_index = version.get("assetIndex").filter(|v| !v.is_null());
    let mut index_id = str_of(version, "assets")
        .map(str::to_string)
        .or_else(|| {
            asset_index
                .and_then(|a| str_of(a, "id"))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "legacy".into());
    let Some(ai) = asset_index else {
        return Ok(index_id);
    };
    let Some(url) = str_of(ai, "url") else {
        return Ok(index_id);
    };
    if let Some(id) = str_of(ai, "id") {
        index_id = id.to_string();
    }
    validate_safe_name(&index_id)?;
    let index_file = paths.asset_indexes().join(format!("{index_id}.json"));
    let expected = sha1_size(
        ai.get("sha1")
            .and_then(Value::as_str)
            .map(str::to_string)
            .as_ref(),
        ai.get("size").and_then(Value::as_u64),
    );
    download_file(url, &index_file, &dl_opts(expected, &opts.fetch)).await?;
    let index: Value =
        serde_json::from_slice(&tokio::fs::read(&index_file).await?).map_err(|source| {
            MinecraftError::Json {
                what: format!("asset index {index_id}"),
                source,
            }
        })?;

    let truthy = |k: &str| match index.get(k) {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(_) => true,
    };
    let map_to_resources = truthy("map_to_resources");
    let legacy = truthy("virtual") || map_to_resources;
    let legacy_root = if map_to_resources {
        paths.asset_resources()
    } else {
        paths.asset_virtual().join(&index_id)
    };

    let objects: Vec<(String, Value)> = index
        .get("objects")
        .and_then(Value::as_object)
        .map(|m| m.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let total = objects.len();
    opts.progress(Phase::Assets, 0, total, "assets");
    let done = Arc::new(AtomicUsize::new(0));
    let objects_dir = paths.asset_objects();
    let resources = opts.endpoints.resources.trim_end_matches('/').to_string();
    let (o, d) = (opts.clone(), done.clone());
    pool(objects, opts.concurrency, move |(name, obj), _| {
        let (o, d) = (o.clone(), d.clone());
        let (objects_dir, legacy_root, resources) =
            (objects_dir.clone(), legacy_root.clone(), resources.clone());
        async move {
            let result = install_asset(
                &o,
                &objects_dir,
                &resources,
                &name,
                &obj,
                legacy.then_some(&legacy_root),
            )
            .await;
            if let Err(msg) = result {
                o.log(&format!("Asset failed: {name} ({msg})"));
            }
            let n = d.fetch_add(1, Ordering::SeqCst) + 1;
            if n % 25 == 0 || n == total {
                o.progress(Phase::Assets, n, total, &name);
            }
        }
    })
    .await?;
    Ok(index_id)
}

async fn install_asset(
    o: &InstallOptions,
    objects_dir: &Path,
    resources: &str,
    name: &str,
    obj: &Value,
    legacy_root: Option<&PathBuf>,
) -> std::result::Result<(), String> {
    let hash = str_of(obj, "hash").ok_or("missing hash")?;
    if !is_hex_hash(hash) {
        return Err("invalid hash".into());
    }
    let sub = &hash[..2];
    let dest = objects_dir.join(sub).join(hash);
    let expected = Expected {
        sha1: Some(hash.to_string()),
        size: obj.get("size").and_then(Value::as_u64),
        ..Default::default()
    };
    download_file(
        &format!("{resources}/{sub}/{hash}"),
        &dest,
        &dl_opts(expected, &o.fetch),
    )
    .await
    .map_err(|e| e.to_string())?;
    if let Some(root) = legacy_root {
        if !safe_asset_name(name) {
            return Err("unsafe asset name".into());
        }
        let target = root.join(name);
        if tokio::fs::metadata(&target).await.is_err() {
            if let Some(parent) = target.parent() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| e.to_string())?;
            }
            tokio::fs::copy(&dest, &target)
                .await
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Install a version (resolving, flattening and downloading as needed).
pub async fn install_version(
    paths: &Paths,
    version_id: &str,
    opts: &InstallOptions,
) -> Result<InstallResult> {
    let resolved =
        resolve_version(paths, version_id, &opts.endpoints.manifest, &opts.fetch).await?;
    let version = resolved.version;
    let id = str_of(&version, "id").unwrap_or(version_id).to_string();
    validate_safe_name(&id)?;
    let collected = collect_libraries(&version, &paths.libraries(), &opts.env);

    // client jar
    opts.progress(Phase::Client, 0, 1, &format!("{id} client"));
    let client_jar = paths.versions().join(&id).join(format!("{id}.jar"));
    let client_dl = version
        .get("downloads")
        .and_then(|d| d.get("client"))
        .filter(|c| !c.is_null());
    if let Some(dl) = client_dl.filter(|c| str_of(c, "url").is_some()) {
        let expected = sha1_size(
            dl.get("sha1")
                .and_then(Value::as_str)
                .map(str::to_string)
                .as_ref(),
            dl.get("size").and_then(Value::as_u64),
        );
        let url = str_of(dl, "url").unwrap_or_default();
        download_file(url, &client_jar, &dl_opts(expected, &opts.fetch)).await?;
    } else if tokio::fs::metadata(&client_jar).await.is_err() {
        // Loader-only versions reuse the parent's jar.
        let parent = resolved.parent_id.as_deref().unwrap_or(&id);
        if validate_safe_name(parent).is_ok() {
            let parent_jar = paths.versions().join(parent).join(format!("{parent}.jar"));
            if tokio::fs::metadata(&parent_jar).await.is_ok() && parent_jar != client_jar {
                tokio::fs::copy(&parent_jar, &client_jar).await?;
            }
        }
    }
    opts.progress(Phase::Client, 1, 1, &format!("{id} client"));

    // libraries + natives
    let all: Vec<LibraryFile> = collected
        .classpath
        .iter()
        .chain(collected.natives.iter())
        .cloned()
        .collect();
    download_libraries(opts, all).await?;
    let log_cb = opts.on_log.clone();
    let natives_dir = extract_natives(&paths.natives(), &id, &collected.natives, &move |l| {
        if let Some(cb) = &log_cb {
            cb(l);
        }
    })
    .await?;

    // log4j config
    let mut logging_arg = None;
    if let Some(file) = version
        .get("logging")
        .and_then(|l| l.get("client"))
        .and_then(|c| c.get("file"))
    {
        if let (Some(url), Some(fid)) = (str_of(file, "url"), str_of(file, "id")) {
            if validate_safe_name(fid).is_ok() {
                let dest = paths.asset_log_configs().join(fid);
                let expected = sha1_size(
                    file.get("sha1")
                        .and_then(Value::as_str)
                        .map(str::to_string)
                        .as_ref(),
                    file.get("size").and_then(Value::as_u64),
                );
                let _ = download_file(url, &dest, &dl_opts(expected, &opts.fetch)).await;
                if tokio::fs::metadata(&dest).await.is_ok() {
                    let template = version
                        .get("logging")
                        .and_then(|l| l.get("client"))
                        .and_then(|c| str_of(c, "argument"))
                        .unwrap_or("-Dlog4j.configurationFile=${path}");
                    logging_arg = Some(template.replace("${path}", &dest.to_string_lossy()));
                }
            }
        }
    }

    let assets_index_id = install_assets(paths, opts, &version).await?;
    opts.progress(Phase::Done, 1, 1, "ready");

    Ok(InstallResult {
        client_jar,
        classpath: collected.classpath.into_iter().map(|l| l.path).collect(),
        natives_dir,
        is_legacy_assets: assets_index_id == "legacy" || assets_index_id == "pre-1.6",
        assets_index_id,
        logging_arg,
        version,
    })
}
