use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;
use tokio::process::Command;

/// A runnable Java and its major version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaInfo {
    pub path: PathBuf,
    pub major: u32,
    /// First line of `java -version` output.
    pub raw: String,
}

/// Extract the major version from `java -version` output. `1.8.0_x` is 8.
pub fn parse_java_version(output: &str) -> Option<u32> {
    static RE: OnceLock<Option<Regex>> = OnceLock::new();
    let re = RE
        .get_or_init(|| Regex::new(r#"version "(\d+)(?:\.(\d+))?[^"]*""#).ok())
        .as_ref()?;
    let caps = re.captures(output)?;
    let first = caps.get(1)?.as_str();
    let n = if first == "1" {
        caps.get(2)?.as_str()
    } else {
        first
    };
    n.parse().ok()
}

fn cache() -> &'static Mutex<HashMap<PathBuf, Option<JavaInfo>>> {
    static C: OnceLock<Mutex<HashMap<PathBuf, Option<JavaInfo>>>> = OnceLock::new();
    C.get_or_init(Default::default)
}

async fn run(path: &Path) -> Option<JavaInfo> {
    let mut attempts = 0;
    let out = loop {
        let fut = Command::new(path)
            .arg("-version")
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        match tokio::time::timeout(Duration::from_secs(10), fut).await {
            Err(_) => return None,
            Ok(Ok(o)) => break o,
            // ETXTBSY: a freshly written script may still be held by a forking thread.
            Ok(Err(e)) if e.raw_os_error() == Some(26) && attempts < 5 => {
                attempts += 1;
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            Ok(Err(_)) => return None,
        }
    };
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let major = parse_java_version(&text)?;
    Some(JavaInfo {
        path: path.to_path_buf(),
        major,
        raw: text.lines().next().unwrap_or("").trim().to_string(),
    })
}

/// Run `java -version` (10s timeout). Results, including failures, are
/// memoized per path for the life of the process.
pub async fn probe(java_path: &Path) -> Option<JavaInfo> {
    {
        let guard = cache().lock().unwrap_or_else(|e| e.into_inner());
        if let Some(hit) = guard.get(java_path) {
            return hit.clone();
        }
    }
    let info = run(java_path).await;
    cache()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(java_path.to_path_buf(), info.clone());
    info
}
