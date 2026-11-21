//! Library and native selection from a flattened version JSON.

use crate::host::{HostArch, HostEnv};
use crate::rules::{rules_allow, Features};
use serde_json::Value;
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};

/// One downloadable file: a classpath jar or a natives archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryFile {
    pub name: String,
    pub url: String,
    pub path: PathBuf,
    pub sha1: Option<String>,
    pub size: Option<u64>,
    /// Natives extraction excludes (path prefixes).
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Collected {
    pub classpath: Vec<LibraryFile>,
    pub natives: Vec<LibraryFile>,
}

impl Collected {
    /// Jars for `-cp`. Modern natives (`lib:natives-linux` as its own library)
    /// must be on the classpath too: since 26.x LWJGL finds `liblwjgl.so` via
    /// the classpath and extracts it to `-Dorg.lwjgl.system.SharedLibraryExtractPath`,
    /// while `java.library.path` points at an (empty) subdirectory.
    pub fn runtime_classpath(&self) -> Vec<&LibraryFile> {
        let mut seen: HashSet<&Path> = HashSet::new();
        self.classpath
            .iter()
            .chain(self.natives.iter().filter(|n| n.name.contains(":natives-")))
            .filter(|f| seen.insert(f.path.as_path()))
            .collect()
    }
}

/// `group:artifact:version[:classifier][@ext]` -> `group/path/artifact/version/file.ext`
/// (always `/`-separated). `None` when the coordinates are incomplete.
pub fn maven_to_path(name: &str, extra_ext: Option<&str>) -> Option<String> {
    let (coords, ext) = match name.split_once('@') {
        Some((c, e)) => (c, e),
        None => (name, extra_ext.unwrap_or("jar")),
    };
    let parts: Vec<&str> = coords.split(':').collect();
    let (group, artifact, version) = (*parts.first()?, *parts.get(1)?, *parts.get(2)?);
    let file = match parts.get(3) {
        Some(c) => format!("{artifact}-{version}-{c}.{ext}"),
        None => format!("{artifact}-{version}.{ext}"),
    };
    Some(format!(
        "{}/{artifact}/{version}/{file}",
        group.replace('.', "/")
    ))
}

/// A relative path from untrusted JSON must stay below its root.
pub fn is_safe_relative(rel: &str) -> bool {
    !rel.is_empty()
        && !rel.contains('\0')
        && Path::new(rel)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        && !rel.contains('\\')
}

fn native_classifier(lib: &Value, env: &HostEnv) -> Option<String> {
    let raw = lib
        .get("natives")?
        .get(env.os.rule_name())?
        .as_str()
        .filter(|s| !s.is_empty())?;
    let bits = if env.arch == HostArch::X64 {
        "64"
    } else {
        "32"
    };
    Some(raw.replace("${arch}", bits))
}

/// Modern style: the native is its own `...:natives-linux` library.
fn is_modern_native(lib: &Value) -> bool {
    lib.get("natives").is_none_or(Value::is_null) && lib_name(lib).contains(":natives-")
}

fn lib_name(lib: &Value) -> &str {
    lib.get("name").and_then(Value::as_str).unwrap_or("")
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct NativeInfo {
    /// Rule-style OS name (`osx` for `macos`).
    pub os: String,
    pub arch: Option<String>,
}

/// Split `...:natives-windows-arm64` into OS and architecture. A trailing word
/// that is not an architecture (`natives-macos-patch`) belongs to the OS
/// variant, not the arch.
pub(crate) fn parse_native_classifier(name: &str) -> Option<NativeInfo> {
    let idx = name.find(":natives-")?;
    let rest = &name[idx + ":natives-".len()..];
    let mut parts = rest.split('-');
    let os = parts.next()?;
    let suffix = parts.next();
    if parts.next().is_some() {
        return None;
    }
    let ok = |s: &str, extra: bool| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || (extra && b == b'_'))
    };
    if !ok(os, false) || suffix.is_some_and(|s| !ok(s, true)) {
        return None;
    }
    Some(NativeInfo {
        os: if os == "macos" {
            "osx".into()
        } else {
            os.into()
        },
        arch: suffix
            .filter(|s| HostArch::is_known_alias(s))
            .map(str::to_string),
    })
}

fn variant_key(name: &str, info: &NativeInfo) -> String {
    let base: Vec<&str> = name.split(':').take(3).collect();
    format!("{}@{}", base.join(":"), info.os)
}

fn string_of(v: Option<&Value>) -> Option<String> {
    v.and_then(Value::as_str).map(str::to_string)
}

fn file_from_download(
    name: &str,
    dl: &Value,
    fallback_rel: Option<String>,
    root: &Path,
    exclude: Vec<String>,
) -> Option<LibraryFile> {
    let rel = string_of(dl.get("path")).or(fallback_rel)?;
    if !is_safe_relative(&rel) {
        return None;
    }
    Some(LibraryFile {
        name: name.to_string(),
        url: string_of(dl.get("url")).unwrap_or_default(),
        path: root.join(&rel),
        sha1: string_of(dl.get("sha1")),
        size: dl.get("size").and_then(Value::as_u64),
        exclude,
    })
}

/// Choose the classpath jars and natives archives for this host.
pub fn collect_libraries(version: &Value, libraries_dir: &Path, env: &HostEnv) -> Collected {
    let features = Features::new();
    let libs: &[Value] = version
        .get("libraries")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice);
    let aliases = env.arch.aliases();

    // Natives that ship a build for exactly this architecture beat the
    // un-suffixed (x64) variant, which is only a fallback.
    let mut has_exact: HashSet<String> = HashSet::new();
    for lib in libs {
        if !is_modern_native(lib) || !rules_allow(lib.get("rules"), &features, env) {
            continue;
        }
        let name = lib_name(lib);
        if let Some(info) = parse_native_classifier(name) {
            if info.arch.as_deref().is_some_and(|a| aliases.contains(&a)) {
                has_exact.insert(variant_key(name, &info));
            }
        }
    }

    let mut out = Collected::default();
    let mut seen: HashSet<String> = HashSet::new();
    for lib in libs {
        if !rules_allow(lib.get("rules"), &features, env) {
            continue;
        }
        let name = lib_name(lib);
        if !seen.insert(name.to_string()) {
            continue;
        }
        let modern = is_modern_native(lib);
        if modern {
            if let Some(info) = parse_native_classifier(name) {
                // Extraction is flat, so a foreign OS/arch would overwrite
                // our lwjgl.dll and kill the JVM.
                if info.os != env.os.rule_name() {
                    continue;
                }
                match &info.arch {
                    Some(a) if !aliases.contains(&a.as_str()) => continue,
                    None if has_exact.contains(&variant_key(name, &info)) => continue,
                    _ => {}
                }
            }
        }

        let artifact = lib.get("downloads").and_then(|d| d.get("artifact"));
        let exclude: Vec<String> = lib
            .get("extract")
            .and_then(|e| e.get("exclude"))
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        if let Some(classifier) = native_classifier(lib, env) {
            let nat = lib
                .get("downloads")
                .and_then(|d| d.get("classifiers"))
                .and_then(|c| c.get(&classifier));
            if let Some(nat) = nat {
                let fallback = maven_to_path(&format!("{name}:{classifier}"), None);
                if let Some(f) = file_from_download(name, nat, fallback, libraries_dir, exclude) {
                    out.natives.push(f);
                }
            }
            // Some legacy libs ship both a native and a normal jar.
            if let Some(a) = artifact {
                if let Some(f) =
                    file_from_download(name, a, maven_to_path(name, None), libraries_dir, vec![])
                {
                    out.classpath.push(f);
                }
            }
            continue;
        }

        let Some(rel) =
            string_of(artifact.and_then(|a| a.get("path"))).or_else(|| maven_to_path(name, None))
        else {
            continue;
        };
        if !is_safe_relative(&rel) {
            continue;
        }
        let url = match string_of(artifact.and_then(|a| a.get("url"))) {
            Some(u) => u,
            None => {
                let base = string_of(lib.get("url"))
                    .unwrap_or_else(|| "https://libraries.minecraft.net/".into());
                format!("{}/{rel}", base.trim_end_matches('/'))
            }
        };
        let entry = LibraryFile {
            name: name.to_string(),
            url,
            path: libraries_dir.join(&rel),
            sha1: string_of(artifact.and_then(|a| a.get("sha1"))),
            size: artifact.and_then(|a| a.get("size")).and_then(Value::as_u64),
            exclude,
        };
        if modern {
            out.natives.push(entry);
        } else {
            out.classpath.push(entry);
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::host::HostOs;
    use serde_json::json;

    fn env(os: HostOs, arch: HostArch) -> HostEnv {
        HostEnv::new(os, arch)
    }

    fn native(name: &str) -> Value {
        json!({"name": name, "downloads": {"artifact": {
            "path": format!("{}.jar", name.replace(':', "_")), "url": "http://x/n", "sha1": "aa", "size": 3}},
            "rules": [{"action":"allow","os":{"name": if name.contains("windows") {"windows"} else if name.contains("macos") {"osx"} else {"linux"}}}]})
    }

    fn natives_names(c: &Collected) -> Vec<String> {
        c.natives.iter().map(|n| n.name.clone()).collect()
    }

    #[test]
    fn maven_paths() {
        assert_eq!(
            maven_to_path("org.lwjgl:lwjgl:3.3.3:natives-linux", None).unwrap(),
            "org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3-natives-linux.jar"
        );
        assert_eq!(
            maven_to_path("a.b:c:1@zip", None).unwrap(),
            "a/b/c/1/c-1.zip"
        );
        assert!(maven_to_path("bad", None).is_none());
    }

    #[test]
    fn classifier_parsing() {
        let i = parse_native_classifier("org.lwjgl:lwjgl:3.3.3:natives-windows-arm64").unwrap();
        assert_eq!(i.os, "windows");
        assert_eq!(i.arch.as_deref(), Some("arm64"));
        let i = parse_native_classifier("g:a:1:natives-macos-patch").unwrap();
        assert_eq!((i.os.as_str(), i.arch), ("osx", None));
        let i = parse_native_classifier("g:a:1:natives-linux").unwrap();
        assert_eq!(i.arch, None);
        assert!(parse_native_classifier("g:a:1").is_none());
    }

    #[test]
    fn windows_arm_natives_never_reach_x64() {
        let v = json!({"libraries": [
            native("org.lwjgl:lwjgl:3.3.3:natives-windows"),
            native("org.lwjgl:lwjgl:3.3.3:natives-windows-arm64"),
            native("org.lwjgl:lwjgl:3.3.3:natives-windows-x86"),
            native("org.lwjgl:lwjgl:3.3.3:natives-linux"),
        ]});
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Windows, HostArch::X64));
        assert_eq!(
            natives_names(&c),
            vec!["org.lwjgl:lwjgl:3.3.3:natives-windows"]
        );
        assert!(c.classpath.is_empty());
    }

    #[test]
    fn exact_arch_beats_unsuffixed() {
        let v = json!({"libraries": [
            native("org.lwjgl:lwjgl:3.3.3:natives-windows"),
            native("org.lwjgl:lwjgl:3.3.3:natives-windows-arm64"),
            native("org.lwjgl:lwjgl-glfw:3.3.3:natives-windows"),
        ]});
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Windows, HostArch::Arm64));
        assert_eq!(
            natives_names(&c),
            vec![
                "org.lwjgl:lwjgl:3.3.3:natives-windows-arm64",
                "org.lwjgl:lwjgl-glfw:3.3.3:natives-windows"
            ]
        );
    }

    #[test]
    fn mac_patch_is_not_an_arch() {
        let v = json!({"libraries": [
            native("org.lwjgl:lwjgl:3.3.3:natives-macos"),
            native("org.lwjgl:lwjgl:3.3.3:natives-macos-arm64"),
            native("org.lwjgl:lwjgl:3.3.3:natives-macos-patch"),
        ]});
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Osx, HostArch::Arm64));
        assert_eq!(
            natives_names(&c),
            vec!["org.lwjgl:lwjgl:3.3.3:natives-macos-arm64"]
        );
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Osx, HostArch::X64));
        assert_eq!(
            natives_names(&c),
            vec![
                "org.lwjgl:lwjgl:3.3.3:natives-macos",
                "org.lwjgl:lwjgl:3.3.3:natives-macos-patch"
            ]
        );
    }

    #[test]
    fn legacy_natives_use_classifier_and_arch_placeholder() {
        let v = json!({"libraries": [{
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
            "natives": {"linux": "natives-linux", "windows": "natives-windows-${arch}"},
            "extract": {"exclude": ["META-INF/"]},
            "downloads": {
                "artifact": {"path": "org/lwjgl/lwjgl-platform-2.9.4.jar", "url": "http://x/a", "size": 1},
                "classifiers": {
                    "natives-windows-64": {"path": "p/win64.jar", "url": "http://x/w64", "sha1": "s", "size": 2},
                    "natives-windows-32": {"path": "p/win32.jar", "url": "http://x/w32", "size": 2},
                    "natives-linux": {"path": "p/lin.jar", "url": "http://x/l", "size": 2}
                }
            }
        }]});
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Windows, HostArch::X64));
        assert_eq!(c.natives.len(), 1);
        assert_eq!(c.natives[0].path, Path::new("/l/p/win64.jar"));
        assert_eq!(c.natives[0].exclude, vec!["META-INF/"]);
        assert_eq!(c.classpath.len(), 1);
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Windows, HostArch::X86));
        assert_eq!(c.natives[0].path, Path::new("/l/p/win32.jar"));
    }

    #[test]
    fn rules_filter_and_duplicates_and_default_url() {
        let v = json!({"libraries": [
            {"name": "a:b:1", "rules": [{"action":"allow","os":{"name":"osx"}}]},
            {"name": "c:d:2", "url": "https://maven.example/repo"},
            {"name": "c:d:2"},
            {"name": "../../evil:x:1", "downloads": {"artifact": {"path": "../../evil.jar", "url": "u"}}}
        ]});
        let c = collect_libraries(&v, Path::new("/l"), &env(HostOs::Linux, HostArch::X64));
        assert_eq!(c.classpath.len(), 1);
        assert_eq!(
            c.classpath[0].url,
            "https://maven.example/repo/c/d/2/d-2.jar"
        );
    }

    fn fixture_26_3() -> Value {
        serde_json::from_str(include_str!("../testdata/26.3-trimmed.json")).unwrap()
    }

    fn cp_names(c: &Collected) -> Vec<String> {
        c.runtime_classpath()
            .iter()
            .map(|f| f.name.clone())
            .collect()
    }

    #[test]
    fn v26_natives_are_on_the_classpath_per_platform() {
        let v = fixture_26_3();
        let root = Path::new("/r");
        for (os, arch, want) in [
            (
                HostOs::Linux,
                HostArch::X64,
                "org.lwjgl:lwjgl:3.4.3:natives-linux",
            ),
            (
                HostOs::Windows,
                HostArch::X64,
                "org.lwjgl:lwjgl:3.4.3:natives-windows",
            ),
            (
                HostOs::Windows,
                HostArch::Arm64,
                "org.lwjgl:lwjgl:3.4.3:natives-windows-arm64",
            ),
            (
                HostOs::Osx,
                HostArch::X64,
                "org.lwjgl:lwjgl:3.4.3:natives-macos",
            ),
            (
                HostOs::Osx,
                HostArch::Arm64,
                "org.lwjgl:lwjgl:3.4.3:natives-macos-arm64",
            ),
        ] {
            let c = collect_libraries(&v, root, &env(os, arch));
            let cp = cp_names(&c);
            assert!(cp.contains(&want.to_string()), "{os:?}/{arch:?}: {cp:?}");
            assert!(cp.contains(&"org.lwjgl:lwjgl:3.4.3".to_string()));
            // exactly one lwjgl natives jar for this host
            let core: Vec<_> = cp
                .iter()
                .filter(|n| n.starts_with("org.lwjgl:lwjgl:3.4.3:natives"))
                .collect();
            assert_eq!(core.len(), 1, "{os:?}/{arch:?}: {core:?}");
        }
    }

    #[test]
    fn legacy_natives_stay_off_the_classpath() {
        let v = json!({"libraries": [{
            "name": "org.lwjgl.lwjgl:lwjgl-platform:2.9.4",
            "natives": {"linux": "natives-linux"},
            "downloads": {"classifiers": {"natives-linux": {"path": "a/n.jar", "url": "http://x", "sha1": "a", "size": 1}}}
        }]});
        let c = collect_libraries(&v, Path::new("/r"), &env(HostOs::Linux, HostArch::X64));
        assert_eq!(c.natives.len(), 1);
        assert!(c.runtime_classpath().is_empty());
    }
}
