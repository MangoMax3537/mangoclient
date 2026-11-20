//! `java_list` and `java_install`, and the Java resolution order of a launch.
//!
//! Order for a launch (see `mango_java::decide`): the instance's own
//! `javaPath` wins, then the global `config.javaPath`, then a managed runtime,
//! then a system JVM, then a Temurin download. A custom path (either one) is
//! never silently replaced: when it is not runnable or older than the game
//! needs, the launch fails (`java.not_runnable` / `java.too_old`).

use crate::error::{AppError, Res};
use crate::events::{Event, JavaProgressEvent};
use crate::service::AppService;
use mango_java::{ResolveOptions, SearchRoots};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "lowercase")]
pub enum JavaSourceName {
    /// Found on this machine (PATH, JAVA_HOME, JVM folders).
    System,
    /// Downloaded by the launcher (Eclipse Temurin).
    Managed,
    /// A path the user configured (globally or for an instance).
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct JavaInfo {
    /// The `java` executable.
    pub path: String,
    pub major: u32,
    /// First line of `java -version`.
    pub raw: String,
    pub vendor: Option<String>,
    pub source: JavaSourceName,
}

/// Best-effort vendor from the `java -version` line and the install path.
fn vendor_of(raw: &str, path: &Path, source: JavaSourceName) -> Option<String> {
    if source == JavaSourceName::Managed {
        return Some("Eclipse Temurin".into());
    }
    let hay = format!("{raw} {}", path.display()).to_ascii_lowercase();
    [
        ("temurin", "Eclipse Temurin"),
        ("adoptium", "Eclipse Temurin"),
        ("adoptopenjdk", "AdoptOpenJDK"),
        ("zulu", "Azul Zulu"),
        ("corretto", "Amazon Corretto"),
        ("graalvm", "GraalVM"),
        ("microsoft", "Microsoft"),
        ("semeru", "IBM Semeru"),
        ("bellsoft", "BellSoft Liberica"),
        ("liberica", "BellSoft Liberica"),
        ("oracle", "Oracle"),
    ]
    .iter()
    .find(|(needle, _)| hay.contains(needle))
    .map(|(_, name)| (*name).to_string())
}

/// The per-instance custom Java wins over the global one; empty = none.
pub(crate) fn custom_java_path(profile: Option<&str>, global: &str) -> Option<PathBuf> {
    [profile.unwrap_or(""), global]
        .into_iter()
        .find(|p| !p.trim().is_empty())
        .map(PathBuf::from)
}

impl AppService {
    /// Options of the Java resolver: managed runtimes under the data root,
    /// the Adoptium base and download policy of the service.
    pub(crate) fn java_options(&self) -> ResolveOptions {
        let paths = &self.inner.paths;
        let mut opts = ResolveOptions::new(paths.runtimes(), paths.cache());
        opts.adoptium_base = self.inner.opts.adoptium_base.clone();
        if let Some(policy) = &self.inner.opts.download_policy {
            opts.link_policy = policy.clone();
        }
        if let Some(roots) = &self.inner.opts.java_roots {
            opts.roots = roots.clone();
        }
        opts
    }

    async fn describe(path: PathBuf, source: JavaSourceName) -> Option<JavaInfo> {
        let info = mango_java::probe(&path).await?;
        Some(JavaInfo {
            vendor: vendor_of(&info.raw, &info.path, source),
            path: info.path.display().to_string(),
            major: info.major,
            raw: info.raw,
            source,
        })
    }

    /// Managed runtimes, then system JVMs, then custom paths (the config's and
    /// every instance's) that are not already listed. Only runnable ones.
    pub async fn java_list(&self) -> Res<Vec<JavaInfo>> {
        let opts = self.java_options();
        let mut out: Vec<JavaInfo> = Vec::new();
        let mut seen: Vec<PathBuf> = Vec::new();
        let mut push = |info: Option<JavaInfo>, out: &mut Vec<JavaInfo>| {
            let Some(info) = info else { return };
            let key =
                std::fs::canonicalize(&info.path).unwrap_or_else(|_| PathBuf::from(&info.path));
            if !seen.contains(&key) {
                seen.push(key);
                out.push(info);
            }
        };
        for c in mango_java::list_managed_runtimes(&opts.runtimes_dir, opts.platform).await {
            push(
                Self::describe(c.path, JavaSourceName::Managed).await,
                &mut out,
            );
        }
        for i in mango_java::find_system_javas(&opts.roots, opts.platform).await {
            push(
                Self::describe(i.path, JavaSourceName::System).await,
                &mut out,
            );
        }
        let customs: Vec<String> = self.inner.with_store(|s| {
            let mut v = vec![s.config.java_path.clone()];
            v.extend(s.profiles.iter().filter_map(|p| p.java_path.clone()));
            v
        });
        for c in customs.into_iter().filter(|c| !c.trim().is_empty()) {
            push(
                Self::describe(PathBuf::from(c), JavaSourceName::Custom).await,
                &mut out,
            );
        }
        Ok(out)
    }

    /// Download (or reuse) the managed Temurin runtime of `major`; progress
    /// goes out as `java:progress`.
    pub async fn java_install(&self, major: u32) -> Res<JavaInfo> {
        if !(8..=99).contains(&major) {
            return Err(AppError::validation("Java major version must be 8 to 99"));
        }
        let _busy = self.busy_guard();
        let mut opts = self.java_options();
        let inner = Arc::clone(&self.inner);
        opts.progress = Some(Arc::new(move |done, total| {
            inner.emit(Event::JavaProgress(JavaProgressEvent {
                major,
                done,
                total,
            }));
        }));
        let exe = mango_java::download_runtime(major, &opts).await?;
        Self::describe(exe, JavaSourceName::Managed)
            .await
            .ok_or_else(|| AppError::new("java.not_runnable", "The installed Java does not run"))
    }

    /// Validate a `javaPath` patch value: `null`/blank clears it, anything
    /// else must be an absolute path to a runnable Java.
    pub(crate) async fn check_java_path(&self, v: &Value) -> Res<Value> {
        let not_runnable =
            || AppError::new("java.not_runnable", "That Java executable does not run");
        let raw = match v {
            Value::Null => return Ok(Value::Null),
            Value::String(s) if s.trim().is_empty() => return Ok(Value::Null),
            Value::String(s) => s.as_str(),
            _ => return Err(AppError::validation("javaPath must be a string or null")),
        };
        crate::path_grants::reject_special_path(raw)?;
        let path = Path::new(raw);
        if !path.is_absolute() || !path.is_file() {
            return Err(not_runnable());
        }
        if mango_java::probe(path).await.is_none() {
            return Err(not_runnable());
        }
        Ok(Value::String(raw.to_string()))
    }
}

/// Where unit tests and the service share the default search roots.
#[allow(dead_code)]
pub(crate) fn default_roots() -> SearchRoots {
    SearchRoots::from_env(mango_java::Platform::current())
}
