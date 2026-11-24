use crate::platform::{Os, Platform};
use crate::probe::{probe, JavaInfo};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Where to look for system JVMs. Injectable so tests can use temp dirs.
#[derive(Debug, Clone, Default)]
pub struct SearchRoots {
    pub java_home: Option<PathBuf>,
    /// Directories on PATH; `java` is looked up in each.
    pub path_dirs: Vec<PathBuf>,
    /// Directories whose children are JVM homes (`<child>/bin/java`,
    /// `<child>/Contents/Home/bin/java`).
    pub jvm_dirs: Vec<PathBuf>,
}

impl SearchRoots {
    pub fn from_env(platform: Platform) -> Self {
        let java_home = std::env::var_os("JAVA_HOME")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from);
        let mut path_dirs: Vec<PathBuf> = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default();
        if platform.os == Os::Mac {
            // macOS's /usr/bin/java is only Apple's stub: without a JDK, running it can pop up an "install Java" dialog.
            // Real JDKs are found in the JavaVirtualMachines folders below.
            path_dirs.retain(|d| d != Path::new("/usr/bin"));
        }
        let mut jvm_dirs: Vec<PathBuf> = ["/usr/lib/jvm", "/usr/java", "/opt/java"]
            .iter()
            .map(PathBuf::from)
            .collect();
        match platform.os {
            Os::Mac => {
                jvm_dirs.push("/Library/Java/JavaVirtualMachines".into());
                // per-user JDKs (e.g. IntelliJ downloads, SDKMAN-style installs)
                if let Some(home) = std::env::var_os("HOME").filter(|h| !h.is_empty()) {
                    jvm_dirs.push(Path::new(&home).join("Library/Java/JavaVirtualMachines"));
                }
            }
            Os::Windows => {
                let vendors = [
                    "Java",
                    "Eclipse Adoptium",
                    "Eclipse Foundation",
                    "Microsoft",
                    "Zulu",
                    "Amazon Corretto",
                    "BellSoft",
                    "Semeru",
                ];
                for var in ["ProgramFiles", "ProgramFiles(x86)", "ProgramW6432"] {
                    if let Some(base) = std::env::var_os(var) {
                        for v in vendors {
                            jvm_dirs.push(Path::new(&base).join(v));
                        }
                    }
                }
            }
            Os::Linux => {}
        }
        Self {
            java_home,
            path_dirs,
            jvm_dirs,
        }
    }
}

async fn is_file(p: &Path) -> bool {
    tokio::fs::metadata(p)
        .await
        .map(|m| m.is_file())
        .unwrap_or(false)
}

/// Probe every JVM found under `roots`. Unrunnable candidates are dropped.
pub async fn find_system_javas(roots: &SearchRoots, platform: Platform) -> Vec<JavaInfo> {
    let exe = platform.exe_name(false);
    let mut candidates = BTreeSet::new();
    if let Some(home) = &roots.java_home {
        candidates.insert(home.join("bin").join(exe));
    }
    for dir in &roots.path_dirs {
        candidates.insert(dir.join(exe));
    }
    for base in &roots.jvm_dirs {
        let Ok(mut rd) = tokio::fs::read_dir(base).await else {
            continue;
        };
        while let Ok(Some(e)) = rd.next_entry().await {
            let p = e.path();
            candidates.insert(p.join("bin").join(exe));
            candidates.insert(p.join("Contents").join("Home").join("bin").join(exe));
        }
    }
    let mut tasks = Vec::new();
    for c in candidates {
        tasks.push(tokio::spawn(async move {
            if is_file(&c).await {
                probe(&c).await
            } else {
                None
            }
        }));
    }
    let mut out = Vec::new();
    for t in tasks {
        if let Ok(Some(info)) = t.await {
            out.push(info);
        }
    }
    out
}

/// Find the java executable inside an unpacked runtime. JREs unpack into a
/// versioned subfolder whose name cannot be predicted, so this recurses.
/// With `prefer_javaw` on Windows, `javaw.exe` is preferred.
pub fn java_executable(root: &Path, platform: Platform, prefer_javaw: bool) -> Option<PathBuf> {
    if prefer_javaw {
        if let Some(p) = find_exe(root, platform.exe_name(true), 0) {
            return Some(p);
        }
    }
    find_exe(root, platform.exe_name(false), 0)
}

fn find_exe(root: &Path, exe: &str, depth: u32) -> Option<PathBuf> {
    let direct = root.join("bin").join(exe);
    if direct.is_file() {
        return Some(direct);
    }
    let mac = root.join("Contents").join("Home").join("bin").join(exe);
    if mac.is_file() {
        return Some(mac);
    }
    if depth >= 8 {
        return None;
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter().find_map(|d| find_exe(&d, exe, depth + 1))
}

/// Map a discovered `java(.exe)` to its `javaw.exe` sibling for game launch
/// on Windows; other platforms and missing siblings return the input.
pub fn launch_executable(path: &Path, platform: Platform, prefer_javaw: bool) -> PathBuf {
    if prefer_javaw && platform.os == Os::Windows {
        let sib = path.with_file_name("javaw.exe");
        if sib.is_file() {
            return sib;
        }
    }
    path.to_path_buf()
}

/// Runtimes under `runtimes_dir/<name>`, probed. `Candidate.id` is the dir name.
pub async fn list_managed_runtimes(
    runtimes_dir: &Path,
    platform: Platform,
) -> Vec<crate::resolve::Candidate> {
    let Ok(mut rd) = tokio::fs::read_dir(runtimes_dir).await else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    while let Ok(Some(e)) = rd.next_entry().await {
        if e.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
            dirs.push(e.path());
        }
    }
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        let d = dir.clone();
        let exe = tokio::task::spawn_blocking(move || java_executable(&d, platform, false))
            .await
            .ok()
            .flatten();
        let Some(exe) = exe else { continue };
        if let Some(info) = probe(&exe).await {
            out.push(crate::resolve::Candidate {
                path: info.path,
                major: info.major,
                id: dir
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            });
        }
    }
    out
}
