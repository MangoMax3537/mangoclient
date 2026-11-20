use crate::discover::{find_system_javas, launch_executable, list_managed_runtimes, SearchRoots};
use crate::error::JavaError;
use crate::platform::Platform;
use crate::probe::{probe, JavaInfo};
use mango_net::ProgressFn;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Candidate {
    pub path: PathBuf,
    pub major: u32,
    pub id: String,
}

impl From<JavaInfo> for Candidate {
    fn from(i: JavaInfo) -> Self {
        Self {
            path: i.path,
            major: i.major,
            id: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JavaSource {
    Custom,
    Managed,
    System,
    Downloaded,
}

/// A user-configured Java and its probe result (`None` = not runnable).
#[derive(Debug, Clone)]
pub struct CustomJava {
    pub path: PathBuf,
    pub major: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Decision {
    Use {
        path: PathBuf,
        major: u32,
        source: JavaSource,
    },
    /// Nothing suitable; download a Temurin JRE of the required major.
    Download,
}

/// Required Java major: declared `javaVersion.majorVersion`, else a fallback
/// derived from the version id.
pub fn required_major(declared: Option<u32>, version_id: &str) -> u32 {
    if let Some(d) = declared.filter(|d| *d > 0) {
        return d;
    }
    let b = version_id.as_bytes();
    // Year-style ids (26.1, ...) come after 1.21.
    if b.len() >= 3 && b[0].is_ascii_digit() && b[1].is_ascii_digit() && b[2] == b'.' {
        return 25;
    }
    let minor = version_id
        .strip_prefix("1.")
        .map(|r| {
            r.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(21);
    match minor {
        20.. => 21,
        18..=19 => 17,
        17 => 16,
        _ => 8,
    }
}

/// [`required_major`] over a version JSON (`javaVersion.majorVersion`, `id`).
pub fn required_major_from_json(v: &serde_json::Value) -> u32 {
    let declared = v
        .pointer("/javaVersion/majorVersion")
        .and_then(|m| m.as_u64())
        .and_then(|m| u32::try_from(m).ok());
    required_major(declared, v.get("id").and_then(|i| i.as_str()).unwrap_or(""))
}

/// Pure resolution order: custom override -> managed (exact, else greater)
/// -> system (exact, else smallest newer only if required >= 17) -> download.
pub fn decide(
    required: u32,
    custom: Option<&CustomJava>,
    managed: &[Candidate],
    system: &[Candidate],
) -> Result<Decision, JavaError> {
    if let Some(c) = custom {
        let Some(found) = c.major else {
            return Err(JavaError::NotRunnable(c.path.clone()));
        };
        if found < required {
            return Err(JavaError::TooOld { required, found });
        }
        return Ok(Decision::Use {
            path: c.path.clone(),
            major: found,
            source: JavaSource::Custom,
        });
    }
    let used = |c: &Candidate, source| Decision::Use {
        path: c.path.clone(),
        major: c.major,
        source,
    };
    if let Some(c) = managed
        .iter()
        .find(|c| c.major == required)
        .or_else(|| managed.iter().find(|c| c.major > required))
    {
        return Ok(used(c, JavaSource::Managed));
    }
    // Prefer an exact match: newer JVMs can break old Forge versions.
    if let Some(c) = system.iter().find(|c| c.major == required) {
        return Ok(used(c, JavaSource::System));
    }
    if required >= 17 {
        if let Some(c) = system
            .iter()
            .filter(|c| c.major > required)
            .min_by_key(|c| c.major)
        {
            return Ok(used(c, JavaSource::System));
        }
    }
    Ok(Decision::Download)
}

#[derive(Clone)]
pub struct ResolveOptions {
    pub custom_path: Option<PathBuf>,
    pub runtimes_dir: PathBuf,
    /// Where the runtime archive is downloaded before extraction.
    pub cache_dir: PathBuf,
    pub roots: SearchRoots,
    pub platform: Platform,
    pub adoptium_base: String,
    /// Hosts the Adoptium API and the package link (including every redirect
    /// hop) may use. Tests with a local mock server use `HostPolicy::allow_all()`.
    pub link_policy: mango_net::HostPolicy,
    /// Prefer `javaw.exe` on Windows (game launch).
    pub prefer_javaw: bool,
    pub progress: Option<ProgressFn>,
}

impl ResolveOptions {
    pub fn new(runtimes_dir: impl Into<PathBuf>, cache_dir: impl Into<PathBuf>) -> Self {
        let platform = Platform::current();
        Self {
            custom_path: None,
            runtimes_dir: runtimes_dir.into(),
            cache_dir: cache_dir.into(),
            roots: SearchRoots::from_env(platform),
            platform,
            adoptium_base: crate::adoptium::DEFAULT_ADOPTIUM_BASE.to_string(),
            link_policy: crate::adoptium::default_link_policy(),
            prefer_javaw: false,
            progress: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    pub path: PathBuf,
    pub major: u32,
    pub source: JavaSource,
}

/// Resolve the java binary to launch with `required` major version.
pub async fn resolve_java(required: u32, opts: &ResolveOptions) -> Result<Resolved, JavaError> {
    let custom = match &opts.custom_path {
        Some(p) => Some(CustomJava {
            path: p.clone(),
            major: probe(Path::new(p)).await.map(|i| i.major),
        }),
        None => None,
    };
    let (managed, system) = if custom.is_some() {
        (Vec::new(), Vec::new())
    } else {
        let managed = list_managed_runtimes(&opts.runtimes_dir, opts.platform).await;
        let system: Vec<Candidate> = find_system_javas(&opts.roots, opts.platform)
            .await
            .into_iter()
            .map(Candidate::from)
            .collect();
        (managed, system)
    };
    match decide(required, custom.as_ref(), &managed, &system)? {
        Decision::Use {
            path,
            major,
            source,
        } => {
            let path = if source == JavaSource::Custom {
                path
            } else {
                launch_executable(&path, opts.platform, opts.prefer_javaw)
            };
            Ok(Resolved {
                path,
                major,
                source,
            })
        }
        Decision::Download => {
            let exe = crate::adoptium::download_runtime(required, opts).await?;
            Ok(Resolved {
                path: exe,
                major: required,
                source: JavaSource::Downloaded,
            })
        }
    }
}
