//! Mod loader support: version listing and installation for Fabric, Quilt,
//! Forge and NeoForge. Port of `src/main/loaders.js`.

use mango_core::paths::Paths;
use mango_java::JavaInfo;
use mango_net::{download_file, get_bytes, get_json, DownloadOptions, FetchOptions, NetError};
use regex::Regex;
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

#[derive(Debug, Error)]
pub enum LoaderError {
    #[error("no {loader} build is available for Minecraft {mc}")]
    NoVersions { loader: String, mc: String },
    #[error("unsupported loader: {0}")]
    Unsupported(String),
    #[error("a Java runtime is required to install {0}")]
    JavaRequired(String),
    #[error("could not start the {loader} installer: {source}")]
    Spawn {
        loader: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{loader} installer exited with code {code}")]
    InstallerFailed { loader: String, code: String },
    #[error("{0} install finished but no version profile was created")]
    NoProfile(String),
    #[error("invalid loader version metadata: {0}")]
    BadMetadata(String),
    #[error("unsafe version id: {0}")]
    UnsafeId(String),
    #[error(transparent)]
    Net(#[from] NetError),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl LoaderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NoVersions { .. } => "loaders.no_versions",
            Self::Unsupported(_) => "loaders.unsupported",
            Self::JavaRequired(_) => "loaders.java_required",
            Self::Spawn { .. } => "loaders.spawn",
            Self::InstallerFailed { .. } => "loaders.installer_failed",
            Self::NoProfile(_) => "loaders.no_profile",
            Self::BadMetadata(_) => "loaders.bad_metadata",
            Self::UnsafeId(_) => "loaders.unsafe_id",
            Self::Net(e) => e.code(),
            Self::Io(_) => "loaders.io",
            Self::Json(_) => "loaders.json",
        }
    }
}

pub type Result<T> = std::result::Result<T, LoaderError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Loader {
    Vanilla,
    Fabric,
    Quilt,
    Forge,
    NeoForge,
}

impl Loader {
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "vanilla" | "" => Self::Vanilla,
            "fabric" => Self::Fabric,
            "quilt" => Self::Quilt,
            "forge" => Self::Forge,
            "neoforge" => Self::NeoForge,
            _ => return None,
        })
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
        }
    }
}

/// Base URLs, injectable for tests.
#[derive(Debug, Clone)]
pub struct Endpoints {
    pub fabric_meta: String,
    pub quilt_meta: String,
    pub neoforge_maven: String,
    pub forge_maven: String,
}

impl Default for Endpoints {
    fn default() -> Self {
        Self {
            fabric_meta: "https://meta.fabricmc.net/v2".into(),
            quilt_meta: "https://meta.quiltmc.org/v3".into(),
            neoforge_maven: "https://maven.neoforged.net/releases/net/neoforged/neoforge".into(),
            forge_maven: "https://maven.minecraftforge.net/net/minecraftforge/forge".into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoaderVersion {
    pub version: String,
    pub stable: bool,
    /// Forge only: full maven version (`<mc>-<lv>`).
    pub full: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameVersion {
    pub version: String,
    pub stable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledLoader {
    pub version_id: String,
    pub loader_version: String,
}

fn percent_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Parse all `<version>` entries of a maven-metadata.xml, in file order.
pub fn parse_maven_versions(xml: &str) -> Vec<String> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let Some(re) = RE
        .get_or_init(|| Regex::new(r"<version>([^<]+)</version>").ok())
        .as_ref()
    else {
        return Vec::new();
    };
    re.captures_iter(xml)
        .filter_map(|c| c.get(1).map(|m| m.as_str().trim().to_string()))
        .collect()
}

/// NeoForge encodes the MC version in its own: 1.21.4 -> `21.4.`, 26.1 -> `26.1.`.
pub fn neoforge_prefix(mc: &str) -> String {
    let parts: Vec<&str> = mc.split('.').collect();
    if parts.first() == Some(&"1") {
        format!(
            "{}.{}.",
            parts.get(1).copied().unwrap_or(""),
            parts.get(2).copied().unwrap_or("0")
        )
    } else {
        format!(
            "{}.{}.",
            parts.first().copied().unwrap_or(""),
            parts.get(1).copied().unwrap_or("0")
        )
    }
}

fn contains_ci(s: &str, needle: &str) -> bool {
    s.to_ascii_lowercase().contains(needle)
}

/// Quilt marks pre-releases by name only.
pub fn quilt_stable(v: &str) -> bool {
    !(contains_ci(v, "beta") || contains_ci(v, "pre"))
}

pub fn neoforge_stable(v: &str) -> bool {
    !contains_ci(v, "beta")
}

/// Version id the installed loader will have in `versions/`.
pub fn version_id_for(loader: Loader, mc: &str, lv: &str) -> String {
    match loader {
        Loader::Fabric => format!("fabric-loader-{lv}-{mc}"),
        Loader::Quilt => format!("quilt-loader-{lv}-{mc}"),
        Loader::NeoForge => format!("neoforge-{lv}"),
        Loader::Forge => format!("{mc}-forge-{lv}"),
        Loader::Vanilla => mc.to_string(),
    }
}

/// Default pick: first stable, else first.
pub fn default_version(list: &[LoaderVersion]) -> Option<&LoaderVersion> {
    list.iter().find(|v| v.stable).or_else(|| list.first())
}

#[derive(Deserialize)]
struct MetaEntry {
    loader: MetaLoader,
}
#[derive(Deserialize)]
struct MetaLoader {
    version: String,
    #[serde(default)]
    stable: bool,
}
#[derive(Deserialize)]
struct MetaGame {
    version: String,
    #[serde(default)]
    stable: bool,
}

pub struct Loaders {
    pub endpoints: Endpoints,
    pub fetch: FetchOptions,
}

impl Default for Loaders {
    fn default() -> Self {
        Self::new(Endpoints::default())
    }
}

/// Reject ids that could escape `versions/`.
fn safe_id(id: &str) -> Result<()> {
    let bad = id.is_empty()
        || id == "."
        || id == ".."
        || id.contains(['/', '\\', '\0'])
        || id.contains(':');
    if bad {
        Err(LoaderError::UnsafeId(id.to_string()))
    } else {
        Ok(())
    }
}

impl Loaders {
    pub fn new(endpoints: Endpoints) -> Self {
        Self {
            endpoints,
            fetch: FetchOptions::default(),
        }
    }

    /// Loader versions for a Minecraft version, newest first.
    pub async fn list_versions(&self, loader: Loader, mc: &str) -> Result<Vec<LoaderVersion>> {
        let e = &self.endpoints;
        match loader {
            Loader::Fabric | Loader::Quilt => {
                let base = if loader == Loader::Fabric {
                    &e.fabric_meta
                } else {
                    &e.quilt_meta
                };
                let url = format!("{base}/versions/loader/{}", percent_encode(mc));
                let list: Vec<MetaEntry> = get_json(&url, &self.fetch).await?;
                Ok(list
                    .into_iter()
                    .map(|m| LoaderVersion {
                        stable: if loader == Loader::Fabric {
                            m.loader.stable
                        } else {
                            quilt_stable(&m.loader.version)
                        },
                        version: m.loader.version,
                        full: None,
                    })
                    .collect())
            }
            Loader::NeoForge => {
                let all = self.maven_versions(&e.neoforge_maven).await?;
                let prefix = neoforge_prefix(mc);
                Ok(all
                    .into_iter()
                    .rev()
                    .filter(|v| v.starts_with(&prefix))
                    .map(|v| LoaderVersion {
                        stable: neoforge_stable(&v),
                        version: v,
                        full: None,
                    })
                    .collect())
            }
            Loader::Forge => {
                let all = self.maven_versions(&e.forge_maven).await?;
                let prefix = format!("{mc}-");
                Ok(all
                    .into_iter()
                    .rev()
                    .filter(|v| v.starts_with(&prefix))
                    .map(|v| LoaderVersion {
                        version: v.split('-').nth(1).unwrap_or("").to_string(),
                        stable: true,
                        full: Some(v),
                    })
                    .collect())
            }
            Loader::Vanilla => Ok(Vec::new()),
        }
    }

    async fn maven_versions(&self, base: &str) -> Result<Vec<String>> {
        let bytes = get_bytes(&format!("{base}/maven-metadata.xml"), &self.fetch).await?;
        Ok(parse_maven_versions(&String::from_utf8_lossy(&bytes)))
    }

    /// Minecraft versions supported by Fabric/Quilt (empty for others).
    pub async fn list_game_versions(&self, loader: Loader) -> Result<Vec<GameVersion>> {
        let base = match loader {
            Loader::Fabric => &self.endpoints.fabric_meta,
            Loader::Quilt => &self.endpoints.quilt_meta,
            _ => return Ok(Vec::new()),
        };
        let list: Vec<MetaGame> = get_json(&format!("{base}/versions/game"), &self.fetch).await?;
        Ok(list
            .into_iter()
            .map(|g| GameVersion {
                version: g.version,
                stable: g.stable,
            })
            .collect())
    }

    /// Ensure the launchable version for a profile exists; returns its id.
    /// `java` is only needed for Forge/NeoForge. `on_log` gets progress lines.
    pub async fn ensure(
        &self,
        paths: &Paths,
        loader: Loader,
        mc: &str,
        loader_version: Option<&str>,
        java: Option<&JavaInfo>,
        on_log: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<InstalledLoader> {
        if loader == Loader::Vanilla {
            return Ok(InstalledLoader {
                version_id: mc.to_string(),
                loader_version: String::new(),
            });
        }
        let lv = match loader_version.filter(|s| !s.is_empty()) {
            Some(v) => v.to_string(),
            None => {
                let list = self.list_versions(loader, mc).await?;
                let v = default_version(&list)
                    .ok_or_else(|| LoaderError::NoVersions {
                        loader: loader.as_str().into(),
                        mc: mc.into(),
                    })?
                    .version
                    .clone();
                on_log(&format!("Using latest {} {v}", loader.as_str()));
                v
            }
        };
        let version_id = match loader {
            Loader::Fabric | Loader::Quilt => {
                self.install_json_loader(paths, loader, mc, &lv, on_log)
                    .await?
            }
            _ => {
                self.install_forge_like(paths, loader, mc, &lv, java, on_log)
                    .await?
            }
        };
        Ok(InstalledLoader {
            version_id,
            loader_version: lv,
        })
    }

    /// Fabric and Quilt serve a ready-made version JSON.
    pub async fn install_json_loader(
        &self,
        paths: &Paths,
        loader: Loader,
        mc: &str,
        lv: &str,
        on_log: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<String> {
        let base = match loader {
            Loader::Fabric => &self.endpoints.fabric_meta,
            Loader::Quilt => &self.endpoints.quilt_meta,
            _ => return Err(LoaderError::Unsupported(loader.as_str().into())),
        };
        let expected = version_id_for(loader, mc, lv);
        safe_id(&expected)?;
        let file = paths
            .versions()
            .join(&expected)
            .join(format!("{expected}.json"));
        if let Ok(raw) = tokio::fs::read(&file).await {
            if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&raw) {
                if v["id"] == expected || v["inheritsFrom"] == mc {
                    on_log(&format!("{} {lv} already installed.", loader.as_str()));
                    return Ok(expected);
                }
            }
        }
        let url = format!(
            "{base}/versions/loader/{}/{}/profile/json",
            percent_encode(mc),
            percent_encode(lv)
        );
        on_log(&format!("Fetching {} {lv} for {mc}...", loader.as_str()));
        let mut json: serde_json::Value = get_json(&url, &self.fetch).await?;
        let obj = json
            .as_object_mut()
            .ok_or_else(|| LoaderError::BadMetadata("profile is not an object".into()))?;
        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&expected)
            .to_string();
        safe_id(&id)?;
        obj.insert("id".into(), serde_json::Value::String(id.clone()));
        let dir = paths.versions().join(&id);
        tokio::fs::create_dir_all(&dir).await?;
        tokio::fs::write(
            dir.join(format!("{id}.json")),
            serde_json::to_vec_pretty(&json)?,
        )
        .await?;
        Ok(id)
    }

    /// Run the official Forge/NeoForge installer headlessly.
    pub async fn install_forge_like(
        &self,
        paths: &Paths,
        loader: Loader,
        mc: &str,
        lv: &str,
        java: Option<&JavaInfo>,
        on_log: &(dyn Fn(&str) + Send + Sync),
    ) -> Result<String> {
        let name = loader.as_str();
        let id = version_id_for(loader, mc, lv);
        safe_id(&id)?;
        let versions = paths.versions();
        if tokio::fs::try_exists(versions.join(&id).join(format!("{id}.json")))
            .await
            .unwrap_or(false)
        {
            on_log(&format!("{name} {lv} already installed."));
            return Ok(id);
        }
        let java = java.ok_or_else(|| LoaderError::JavaRequired(name.into()))?;
        let (full, url) = match loader {
            Loader::NeoForge => (
                lv.to_string(),
                format!(
                    "{0}/{lv}/neoforge-{lv}-installer.jar",
                    self.endpoints.neoforge_maven
                ),
            ),
            Loader::Forge => {
                let full = format!("{mc}-{lv}");
                let url = format!(
                    "{0}/{full}/forge-{full}-installer.jar",
                    self.endpoints.forge_maven
                );
                (full, url)
            }
            _ => return Err(LoaderError::Unsupported(name.into())),
        };
        safe_id(&full)?;
        let jar = paths.cache().join(format!("{name}-{full}-installer.jar"));
        on_log(&format!("Downloading {name} installer..."));
        let opts = DownloadOptions {
            fetch: self.fetch.clone(),
            ..Default::default()
        };
        download_file(&url, &jar, &opts).await?;

        let root = paths.root();
        tokio::fs::create_dir_all(root).await?;
        let stub = serde_json::to_vec_pretty(&serde_json::json!({"profiles": {}, "version": 3}))?;
        let pf = root.join("launcher_profiles.json");
        if !tokio::fs::try_exists(&pf).await.unwrap_or(false) {
            tokio::fs::write(&pf, &stub).await?;
        }
        // Best effort, as in the legacy launcher.
        let _ = tokio::fs::write(root.join("launcher_profiles_microsoft_store.json"), &stub).await;

        on_log(&format!(
            "Running {name} installer (this can take a minute)..."
        ));
        run_installer(name, &java.path, &jar, root, on_log).await?;

        if tokio::fs::try_exists(versions.join(&id).join(format!("{id}.json")))
            .await
            .unwrap_or(false)
        {
            return Ok(id);
        }
        // Installers occasionally name the folder slightly differently.
        let mut found: Vec<String> = Vec::new();
        if let Ok(mut rd) = tokio::fs::read_dir(&versions).await {
            while let Ok(Some(ent)) = rd.next_entry().await {
                let n = ent.file_name().to_string_lossy().to_string();
                if n.to_lowercase().contains(name) && n.contains(lv) {
                    found.push(n);
                }
            }
        }
        found.sort();
        found
            .into_iter()
            .next()
            .ok_or_else(|| LoaderError::NoProfile(name.into()))
    }
}

async fn pump<R: AsyncRead + Unpin>(r: R, on_log: &(dyn Fn(&str) + Send + Sync)) {
    let mut lines = BufReader::new(r).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        let t = line.trim_end();
        if !t.is_empty() {
            on_log(t);
        }
    }
}

async fn run_installer(
    name: &str,
    java: &Path,
    jar: &Path,
    root: &Path,
    on_log: &(dyn Fn(&str) + Send + Sync),
) -> Result<()> {
    let spawn_err = |source| LoaderError::Spawn {
        loader: name.to_string(),
        source,
    };
    let mut attempts = 0;
    let mut child = loop {
        let res = Command::new(java)
            .arg("-jar")
            .arg(jar)
            .arg("--installClient")
            .arg(root)
            .current_dir(root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn();
        match res {
            Ok(c) => break c,
            // ETXTBSY: a freshly written executable may still be open elsewhere.
            Err(e) if e.raw_os_error() == Some(26) && attempts < 5 => {
                attempts += 1;
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            Err(e) => return Err(spawn_err(e)),
        }
    };
    let out = child.stdout.take();
    let err = child.stderr.take();
    let (_, _, status) = tokio::join!(
        async {
            if let Some(o) = out {
                pump(o, on_log).await;
            }
        },
        async {
            if let Some(e) = err {
                pump(e, on_log).await;
            }
        },
        child.wait()
    );
    let status = status?;
    if status.success() {
        Ok(())
    } else {
        Err(LoaderError::InstallerFailed {
            loader: name.to_string(),
            code: status
                .code()
                .map_or_else(|| "signal".to_string(), |c| c.to_string()),
        })
    }
}

/// Path of a version's JSON inside the versions dir.
pub fn version_json_path(paths: &Paths, id: &str) -> PathBuf {
    paths.versions().join(id).join(format!("{id}.json"))
}

#[cfg(test)]
mod tests;
