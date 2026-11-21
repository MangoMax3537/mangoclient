#![allow(clippy::unwrap_used)]

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use mango_core::model::{Loader, ModRecord, ModType, Profile};
use mango_core::paths::Paths;
use tempfile::TempDir;

use crate::perf::ensure_at;
use crate::*;

const ID: &str = "12345678-1234-1234-1234-123456789abc";

fn setup(loader: Loader) -> (TempDir, Paths, Profile, PathBuf) {
    let tmp = TempDir::new().unwrap();
    let paths = Paths::new(tmp.path());
    let mut p = Profile::new(ID.into(), 0);
    p.loader = loader;
    p.mc_version = "1.21.1".into();
    let dir = mods_dir(&paths, ID).unwrap();
    std::fs::create_dir_all(&dir).unwrap();
    (tmp, paths, p, dir)
}

fn png() -> Vec<u8> {
    let mut v = vec![0x89, b'P', b'N', b'G'];
    v.extend_from_slice(&[1, 2, 3, 4, 5]);
    v
}

fn make_jar(path: &Path, entries: &[(&str, &[u8])]) {
    let f = std::fs::File::create(path).unwrap();
    let mut z = zip::ZipWriter::new(f);
    let opts = zip::write::SimpleFileOptions::default();
    for (n, d) in entries {
        z.start_file(*n, opts).unwrap();
        z.write_all(d).unwrap();
    }
    z.finish().unwrap();
}

fn meta(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> JarMeta {
    let jar = dir.join(name);
    make_jar(&jar, entries);
    read_jar_meta(&jar, &dir.join("icons"))
}

#[test]
fn fabric_with_trailing_comma_and_icon() {
    let t = TempDir::new().unwrap();
    let json = br#"{"id":"cool","name":"Cool Mod","version":"1.2.3","icon":"assets/i.png","depends":{"a":"*",},}"#;
    let m = meta(
        t.path(),
        "x.jar",
        &[("fabric.mod.json", json), ("assets/i.png", &png())],
    );
    assert_eq!(m.title, "Cool Mod");
    assert_eq!(m.version, "1.2.3");
    let url = m.icon.unwrap();
    assert!(url.starts_with("mangoimg://f/"));
    assert_eq!(
        std::fs::read_dir(t.path().join("icons")).unwrap().count(),
        1
    );
}

#[test]
fn fabric_icon_map_and_oversized_icon() {
    let t = TempDir::new().unwrap();
    let json = br#"{"id":"c","icon":{"64":"i.png"}}"#;
    let m = meta(
        t.path(),
        "c-1.0.jar",
        &[("fabric.mod.json", json), ("i.png", &png())],
    );
    assert_eq!(m.title, "c");
    assert_eq!(m.version, "1.0");
    assert!(m.icon.is_some());
    let big = vec![0u8; 300 * 1024];
    let m = meta(
        t.path(),
        "d.jar",
        &[("fabric.mod.json", json), ("i.png", &big)],
    );
    assert!(m.icon.is_none());
}

#[test]
fn quilt_meta() {
    let t = TempDir::new().unwrap();
    let json = br#"{"quilt_loader":{"id":"q","version":"2.0","metadata":{"name":"Quilty"}}}"#;
    let m = meta(t.path(), "q.jar", &[("quilt.mod.json", json)]);
    assert_eq!((m.title.as_str(), m.version.as_str()), ("Quilty", "2.0"));
}

#[test]
fn forge_toml_resolves_manifest_version() {
    let t = TempDir::new().unwrap();
    let toml = b"modLoader=\"javafml\"\n[[mods]]\nmodId=\"forgey\"\ndisplayName=\"Forgey\"\nversion=\"${file.jarVersion}\"\n[[dependencies.forgey]]\nmodId=\"x\"\n";
    let mf = b"Manifest-Version: 1.0\nImplementation-Version: 9.9.9\n";
    let m = meta(
        t.path(),
        "f.jar",
        &[("META-INF/mods.toml", toml), ("META-INF/MANIFEST.MF", mf)],
    );
    assert_eq!((m.title.as_str(), m.version.as_str()), ("Forgey", "9.9.9"));
}

#[test]
fn neoforge_toml_literal_version_and_logo() {
    let t = TempDir::new().unwrap();
    let toml = b"logoFile=\"l.png\"\n[[mods]]\nmodId='neo'\nversion='3.1'\n";
    let m = meta(
        t.path(),
        "n.jar",
        &[("META-INF/neoforge.mods.toml", toml), ("l.png", &png())],
    );
    assert_eq!((m.title.as_str(), m.version.as_str()), ("neo", "3.1"));
    assert!(m.icon.is_some());
}

#[test]
fn legacy_mcmod_info_and_filename_fallback() {
    let t = TempDir::new().unwrap();
    let m = meta(
        t.path(),
        "o.jar",
        &[(
            "mcmod.info",
            br#"[{"modid":"old","name":"Old One","version":"0.1"}]"#,
        )],
    );
    assert_eq!((m.title.as_str(), m.version.as_str()), ("Old One", "0.1"));
    let m = meta(
        t.path(),
        "my-cool_mod-v1.4.2+mc1.21.jar",
        &[("a.txt", b"x")],
    );
    assert_eq!(
        (m.title.as_str(), m.version.as_str()),
        ("my cool mod", "v1.4.2+mc1.21")
    );
    std::fs::write(t.path().join("junk.jar"), b"not a zip").unwrap();
    let m = read_jar_meta(&t.path().join("junk.jar"), t.path());
    assert_eq!(m.title, "junk");
}

fn fabric_jar(path: &Path, name: &str) {
    let json = format!(r#"{{"id":"{name}","name":"{name}","version":"1"}}"#);
    make_jar(path, &[("fabric.mod.json", json.as_bytes())]);
}

#[test]
fn unknown_jars_become_local_and_only_mangoconfig_is_hidden() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    fabric_jar(&dir.join("a.jar"), "A");
    fabric_jar(&dir.join("b.jar.disabled"), "B");
    fabric_jar(&dir.join("MangoConfig-1.10+mc1.21.jar"), "MC");
    fabric_jar(&dir.join("MangoConfig-1.10+mc1.21.jar.disabled"), "MC");
    fabric_jar(&dir.join("sodium-fabric-1.jar"), "S");
    std::fs::write(
        dir.join(".mangoclient-performance.json"),
        r#"{"files":{"sodium":"sodium-fabric-1.jar"}}"#,
    )
    .unwrap();
    std::fs::write(dir.join("notes.txt"), "x").unwrap();
    let r = sync_profile_mods(&paths, &p).unwrap();
    assert!(r.changed);
    // a, b and the launcher-installed sodium; only MangoConfig is hidden.
    assert_eq!(r.mods.len(), 3);
    assert_eq!(r.mods[0].project_id.as_deref(), Some("local:a.jar"));
    assert_eq!(r.mods[0].local, Some(true));
    assert_eq!(r.mods[0].title.as_deref(), Some("A"));
    assert_eq!(r.mods[1].project_id.as_deref(), Some("local:b.jar"));
    assert_eq!(r.mods[1].enabled, Some(false));
    assert_eq!(
        r.mods[2].project_id.as_deref(),
        Some("local:sodium-fabric-1.jar")
    );
    // A second pass with the result is stable.
    let mut p2 = p.clone();
    p2.mods = r.mods;
    assert!(!sync_profile_mods(&paths, &p2).unwrap().changed);
}

#[test]
fn live_jar_wins_over_disabled_twin() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    fabric_jar(&dir.join("a.jar.disabled"), "A");
    fabric_jar(&dir.join("a.jar"), "A");
    let r = sync_profile_mods(&paths, &p).unwrap();
    assert_eq!(r.mods.len(), 1);
    assert_eq!(r.mods[0].enabled, Some(true));
    assert!(r.mods[0].file.as_deref().unwrap().ends_with("a.jar"));
}

#[test]
fn vanished_jar_dropped_and_state_follows_disk() {
    let (_t, paths, mut p, dir) = setup(Loader::Fabric);
    fabric_jar(&dir.join("kept.jar.disabled"), "K");
    let rec = |name: &str| ModRecord {
        filename: Some(name.into()),
        file: Some(dir.join(name).to_string_lossy().into_owned()),
        enabled: Some(true),
        kind: Some(ModType::Mod),
        ..ModRecord::default()
    };
    let shader = ModRecord {
        kind: Some(ModType::Shader),
        file: Some(dir.join("gone.zip").to_string_lossy().into_owned()),
        ..ModRecord::default()
    };
    let pack = ModRecord {
        kind: Some(ModType::Resourcepack),
        file: Some(dir.join("kept.jar.disabled").to_string_lossy().into_owned()),
        ..ModRecord::default()
    };
    p.mods = vec![rec("kept.jar"), rec("gone.jar"), shader, pack];
    let r = sync_profile_mods(&paths, &p).unwrap();
    assert!(r.changed);
    assert_eq!(r.mods.len(), 2);
    assert_eq!(r.mods[0].enabled, Some(false));
    assert!(r.mods[0]
        .file
        .as_deref()
        .unwrap()
        .ends_with("kept.jar.disabled"));
    assert_eq!(r.mods[1].kind, Some(ModType::Resourcepack));
}

#[test]
fn toggle_local_mod_renames() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    fabric_jar(&dir.join("a.jar"), "A");
    let rec = sync_profile_mods(&paths, &p).unwrap().mods.remove(0);
    let off = set_local_mod_enabled(&rec, false).unwrap();
    assert!(dir.join("a.jar.disabled").exists() && !dir.join("a.jar").exists());
    assert_eq!(off.enabled, Some(false));
    let on = set_local_mod_enabled(&off, true).unwrap();
    assert!(dir.join("a.jar").exists());
    assert!(on.file.unwrap().ends_with("a.jar"));
    let mut remote = rec;
    remote.local = None;
    assert_eq!(
        set_local_mod_enabled(&remote, false).unwrap_err().code(),
        "not_local"
    );
}

// ---- performance mods ----

#[derive(Default)]
struct Mock {
    files: Mutex<HashMap<String, Result<Option<ResolvedFile>, ()>>>,
    resolves: Mutex<u32>,
}

impl Mock {
    fn with(slugs: &[(&str, &str)]) -> Self {
        let m = Mock::default();
        for (slug, file) in slugs {
            m.set(slug, Ok(Some(rf(file))));
        }
        m
    }
    fn set(&self, slug: &str, v: Result<Option<ResolvedFile>, ()>) {
        self.files.lock().unwrap().insert(slug.into(), v);
    }
}

fn rf(name: &str) -> ResolvedFile {
    ResolvedFile {
        filename: name.into(),
        url: "https://example.invalid/x".into(),
        sha512: None,
        sha1: None,
        size: None,
    }
}

impl ModrinthLookup for Mock {
    async fn resolve(&self, slug: &str, _mc: &str) -> Result<Option<ResolvedFile>, ContentError> {
        *self.resolves.lock().unwrap() += 1;
        match self.files.lock().unwrap().get(slug) {
            Some(Ok(f)) => Ok(f.clone()),
            _ => Err(ContentError::Lookup("offline".into())),
        }
    }
    async fn download(&self, file: &ResolvedFile, dest: &Path) -> Result<(), ContentError> {
        std::fs::write(dest, file.filename.as_bytes()).map_err(|e| ContentError::io(dest, e))
    }
}

fn opts(p: &Profile) -> PerfOptions<'_> {
    PerfOptions {
        profile: p,
        global_enabled: true,
        profile_enabled: None,
    }
}

const ALL: [(&str, &str); 3] = [
    ("sodium", "sodium-fabric-0.6.jar"),
    ("lithium", "lithium-fabric-0.14.jar"),
    ("ferrite-core", "ferritecore-7.jar"),
];

#[tokio::test]
async fn installs_and_writes_manifest() {
    let (_t, paths, p, dir) = setup(Loader::Quilt);
    let mock = Mock::with(&ALL);
    let out = ensure_at(&paths, &opts(&p), &mock, 1000).await;
    assert_eq!(out.state, PerfState::Ok);
    assert_eq!(out.installed, ["Sodium", "Lithium", "FerriteCore"]);
    assert!(dir.join("sodium-fabric-0.6.jar").exists());
    let m: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(dir.join(".mangoclient-performance.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(m["files"]["ferrite-core"], "ferritecore-7.jar");
    assert!(is_managed(&dir, "sodium-fabric-0.6.jar"));
    // Installed performance mods are listed like any other jar (so they can
    // be removed); a stale manifest never hides a jar.
    let listed = sync_profile_mods(&paths, &p).unwrap().mods;
    assert_eq!(listed.len(), 3);
    assert!(listed.iter().all(|m| m.local == Some(true)));
    assert!(paths
        .cache()
        .join("performance-mods/sodium-1.21.1.json")
        .exists());
}

#[tokio::test]
async fn user_jar_wins_and_ours_removed() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    let mock = Mock::with(&ALL);
    ensure_at(&paths, &opts(&p), &mock, 1000).await;
    fabric_jar(&dir.join("Embeddium-1.0.jar"), "E");
    let out = ensure_at(&paths, &opts(&p), &mock, 2000).await;
    assert!(!dir.join("sodium-fabric-0.6.jar").exists());
    assert!(dir.join("lithium-fabric-0.14.jar").exists());
    assert!(out
        .skipped
        .iter()
        .any(|s| s.starts_with("Sodium (you have Embeddium")));
    assert!(!managed_names(&dir).contains("sodium-fabric-0.6.jar"));
}

#[tokio::test]
async fn opt_out_and_unsupported_loader_clean_up() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    let mock = Mock::with(&ALL);
    ensure_at(&paths, &opts(&p), &mock, 1000).await;
    fabric_jar(&dir.join("mine.jar"), "M");
    let o = PerfOptions {
        profile: &p,
        global_enabled: true,
        profile_enabled: Some(false),
    };
    let out = ensure_at(&paths, &o, &mock, 1000).await;
    assert_eq!(out.state, PerfState::Removed);
    assert!(!dir.join("lithium-fabric-0.14.jar").exists());
    assert!(!dir.join(".mangoclient-performance.json").exists());
    assert!(dir.join("mine.jar").exists());
    assert_eq!(
        ensure_at(&paths, &o, &mock, 1000).await.state,
        PerfState::Off
    );

    let g = PerfOptions {
        profile: &p,
        global_enabled: false,
        profile_enabled: None,
    };
    assert_eq!(
        ensure_at(&paths, &g, &mock, 1000).await.state,
        PerfState::Off
    );
    let mut forge = p.clone();
    forge.loader = Loader::Forge;
    assert_eq!(
        ensure_at(&paths, &opts(&forge), &mock, 1000).await.state,
        PerfState::Off
    );
    assert!(!dir.join("sodium-fabric-0.6.jar").exists());
}

#[tokio::test]
async fn offline_keeps_existing_and_skips_missing() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    let mock = Mock::with(&[("sodium", "sodium-fabric-0.6.jar")]);
    ensure_at(&paths, &opts(&p), &mock, 1000).await;
    assert!(dir.join("sodium-fabric-0.6.jar").exists());
    // Cache expired and every lookup fails now.
    let later = 1000 + 8 * 24 * 3600 * 1000;
    let offline = Mock::default();
    let out = ensure_at(&paths, &opts(&p), &offline, later).await;
    assert!(dir.join("sodium-fabric-0.6.jar").exists());
    assert!(managed_names(&dir).contains("sodium-fabric-0.6.jar"));
    assert!(out.skipped.contains(&"Lithium (offline)".to_string()));
    assert!(!out.skipped.iter().any(|s| s.starts_with("Sodium")));
}

#[tokio::test]
async fn cache_ttl_and_upgrade_removes_old_build() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    let mock = Mock::with(&[("sodium", "sodium-1.jar")]);
    ensure_at(&paths, &opts(&p), &mock, 1000).await;
    let after_first = *mock.resolves.lock().unwrap();
    // Within TTL: sodium is served from cache, no new lookup for it.
    mock.set("sodium", Ok(Some(rf("sodium-2.jar"))));
    ensure_at(&paths, &opts(&p), &mock, 2000).await;
    let sodium_calls = *mock.resolves.lock().unwrap() - after_first;
    assert_eq!(sodium_calls, 2); // only lithium + ferrite (not cached: unknown to mock)
    assert!(dir.join("sodium-1.jar").exists());
    // Past TTL: new build resolved, old jar removed.
    ensure_at(&paths, &opts(&p), &mock, 1000 + 8 * 24 * 3600 * 1000).await;
    assert!(dir.join("sodium-2.jar").exists());
    assert!(!dir.join("sodium-1.jar").exists());
    assert!(managed_names(&dir).contains("sodium-2.jar"));
}

#[tokio::test]
async fn no_build_removes_old_and_bad_filename_rejected() {
    let (_t, paths, p, dir) = setup(Loader::Fabric);
    let mock = Mock::with(&[("sodium", "sodium-1.jar")]);
    ensure_at(&paths, &opts(&p), &mock, 1000).await;
    mock.set("sodium", Ok(None));
    mock.set("lithium", Ok(Some(rf("../evil.jar"))));
    let later = 1000 + 8 * 24 * 3600 * 1000;
    let out = ensure_at(&paths, &opts(&p), &mock, later).await;
    assert!(!dir.join("sodium-1.jar").exists());
    assert!(out
        .skipped
        .contains(&"Sodium (no build for 1.21.1)".to_string()));
    assert!(out
        .skipped
        .contains(&"Lithium (invalid filename)".to_string()));
    assert!(!dir.parent().unwrap().join("evil.jar").exists());
}
