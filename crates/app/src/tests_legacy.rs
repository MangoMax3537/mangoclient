//! Upgrade from MangoClient 1.3.4 (Electron) to 2.0, end to end: a data directory written by the real 1.3.4 code
//! (`tools/make-legacy-fixture.js`, frozen in `tests/fixtures/legacy-1.3.4`) is opened by the 2.0 service. Every instance,
//! setting, mod record, world, screenshot, server entry, stats session and the install id must come through, and the files
//! the game owns (servers.dat, level.dat, options.txt, jars) must stay byte-for-byte untouched.
#![allow(clippy::unwrap_used)]

use crate::*;
use mango_core::{AccountType, Loader};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct NoSink;
impl EventSink for NoSink {
    fn emit(&self, _: Event) {}
}

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/legacy-1.3.4")
}

fn expected() -> Value {
    serde_json::from_slice(&std::fs::read(fixture().join("expected.json")).unwrap()).unwrap()
}

/// Copies the frozen 1.3.4 root into `dst`, putting the real path back where 1.3.4 wrote absolute paths.
fn install_fixture(dst: &Path) {
    let root = fixture().join("root");
    let esc = serde_json::to_string(&dst.display().to_string()).unwrap();
    let esc = &esc[1..esc.len() - 1];
    for entry in walk(&root) {
        let rel = entry.strip_prefix(&root).unwrap();
        let to = dst.join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        let bytes = std::fs::read(&entry).unwrap();
        let is_json = rel.extension().is_some_and(|e| e == "json")
            || rel.to_string_lossy().ends_with(".json.bak");
        if is_json {
            std::fs::write(
                &to,
                String::from_utf8(bytes).unwrap().replace("{{ROOT}}", esc),
            )
            .unwrap();
        } else {
            std::fs::write(&to, bytes).unwrap();
        }
    }
}

fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in std::fs::read_dir(dir).unwrap() {
        let p = e.unwrap().path();
        if p.is_dir() {
            out.extend(walk(&p));
        } else {
            out.push(p);
        }
    }
    out.sort();
    out
}

/// Bytes of every file the game or the user owns (everything except the launcher's JSON stores).
fn game_files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    walk(root)
        .into_iter()
        .filter(|p| p.starts_with(root.join("instances")) || p.starts_with(root.join("covers")))
        .map(|p| {
            (
                p.strip_prefix(root).unwrap().to_path_buf(),
                std::fs::read(&p).unwrap(),
            )
        })
        .collect()
}

fn open(root: &Path) -> AppService {
    let opts = ServiceOptions {
        open_folders: false,
        ..ServiceOptions::default()
    };
    AppService::new(mango_core::Paths::new(root), Arc::new(NoSink), opts).unwrap()
}

fn rt() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap()
}

#[test]
fn a_1_3_4_data_dir_loads_completely_in_2_0() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    install_fixture(root);
    let exp = expected();
    let id = |k: &str| exp["profiles"][k].as_str().unwrap().to_string();
    let before = game_files(root);
    let install_before = std::fs::read(root.join("install.json")).unwrap();

    let svc = open(root);
    let r = rt();
    let st = svc.app_state().unwrap();

    // settings: every 1.3.4 value the user set comes through; the selection is kept
    let c = &st.config;
    assert_eq!(c.language, "en");
    assert_eq!(c.ram, 6144);
    assert_eq!(c.java_args, "-XX:+UseG1GC");
    assert!(c.fullscreen);
    assert_eq!((c.width, c.height), (1600, 900));
    assert!(!c.keep_launcher_open);
    assert!(c.show_snapshots);
    assert!(c.mango_config);
    assert!(!c.performance_mods);
    assert_eq!(c.concurrent_downloads, 8);
    assert_eq!(c.performance_preset, "quality");
    assert!(c.first_run_done, "no onboarding again for an existing user");
    assert!(!c.telemetry, "an opt-out stays an opt-out");
    assert_eq!(c.selected_profile.as_deref(), Some(id("neoforge").as_str()));

    // instances: all four, with name (umlauts, emoji), version, loader, loader version, RAM, Java args, colour, cover, play time
    let by_id = |k: &str| {
        st.profiles
            .iter()
            .find(|p| p.id == id(k))
            .unwrap_or_else(|| panic!("instance {k} is missing"))
    };
    assert_eq!(st.profiles.len(), 4);
    let d = by_id("default");
    assert_eq!(
        (d.name.as_str(), d.mc_version.as_str(), d.loader),
        ("MangoClient 1.21.11", "1.21.11", Loader::Fabric)
    );
    assert_eq!(d.play_time_ms, 51 * 3_600_000);
    assert!(d.last_played.is_some());
    let v = by_id("vanilla");
    assert_eq!(
        (v.name.as_str(), v.mc_version.as_str(), v.loader),
        ("Vanilla Survival", "1.20.1", Loader::Vanilla)
    );
    assert_eq!(v.ram, Some(4096));
    assert_eq!(v.color.as_deref(), Some("#86b86b"));
    assert!(v.cover_url.is_some(), "the 1.3.4 cover image is shown");
    let n = by_id("neoforge");
    assert_eq!(
        (n.name.as_str(), n.mc_version.as_str(), n.loader),
        ("Create Ä Ü 🚀", "1.21.1", Loader::Neoforge)
    );
    assert_eq!(n.loader_version.as_deref(), Some("21.1.77"));
    assert_eq!(n.mango_config, Some(false));
    let q = by_id("quilt");
    assert_eq!(
        (q.loader, q.loader_version.as_deref(), q.java_args.as_str()),
        (Loader::Quilt, Some("0.26.0"), "-Dfoo=bar")
    );

    // accounts: the offline account works as before; the Microsoft account is listed but signs in once again (Electron's OSCrypt
    // envelope is unreadable by design), and no token ever appears in plain text
    let off = st
        .accounts
        .iter()
        .find(|a| a.uuid == exp["offlineUuid"].as_str().unwrap())
        .expect("offline account");
    assert_eq!(
        (off.name.as_str(), off.kind, off.needs_relogin),
        ("Steve_Old", AccountType::Offline, false)
    );
    let ms = st
        .accounts
        .iter()
        .find(|a| a.name == "Notch")
        .expect("microsoft account is still listed");
    assert_eq!(ms.kind, AccountType::Microsoft);
    assert!(
        ms.needs_relogin,
        "Electron tokens cannot be decrypted: one new sign-in"
    );

    // content: Modrinth records with their metadata (also the one disabled the 1.3.4 way), the hand-added jar, shader, resource pack
    let content = r.block_on(svc.content_list(id("default"))).unwrap();
    let item = |t: &str| {
        content
            .iter()
            .find(|c| c.title == t)
            .unwrap_or_else(|| panic!("{t} is missing"))
    };
    assert_eq!(content.len(), 5);
    assert!(item("Sodium").enabled && !item("Sodium").local);
    assert_eq!(item("Sodium").project_id, "AANobbMI");
    assert!(
        !item("Lithium").enabled && !item("Lithium").local,
        "a mod disabled in 1.3.4 stays a disabled Modrinth mod"
    );
    assert_eq!(
        item("Lithium").filename,
        "lithium-fabric-0.15.0+mc1.21.11.jar"
    );
    assert!(item("my local mod").local && item("my local mod").enabled);
    assert!(content
        .iter()
        .any(|c| c.title == "Complementary Shaders - Reimagined" && c.kind == ContentType::Shader));
    assert!(content
        .iter()
        .any(|c| c.title == "Fresh Animations" && c.kind == ContentType::Resourcepack));

    // worlds (level.dat names, also non-ASCII folders) and screenshots
    let worlds = r.block_on(svc.worlds_list(id("default"))).unwrap();
    let names: Vec<_> = worlds.iter().map(|w| w.name.as_str()).collect();
    assert!(
        names.contains(&"My Survival World") && names.contains(&"Ünïcødé Wörld ★"),
        "{names:?}"
    );
    assert_eq!(
        r.block_on(svc.worlds_list(id("vanilla"))).unwrap()[0].name,
        "Old Map"
    );
    let shots = r.block_on(svc.screenshots_list(id("default"))).unwrap();
    assert_eq!(shots.len(), 2);

    // servers: the partner pinned by 1.3.4 first, then the player's own list in order, modified UTF-8 names intact
    svc.profile_select(id("default")).unwrap();
    let servers = r.block_on(svc.servers_list()).unwrap();
    let list: Vec<_> = servers
        .iter()
        .map(|s| (s.name.as_str(), s.address.as_str()))
        .collect();
    assert_eq!(
        list,
        vec![
            ("VincentVanilla", "vincentvanilla.net"),
            ("Hypixel", "mc.hypixel.net"),
            ("Bäckerei 🍞 Server", "play.example.de:25570"),
            ("LAN a\0b", "192.168.1.40"),
        ]
    );
    assert!(servers[0].partner);

    // stats journal and the telemetry install id
    assert_eq!(
        crate::media::read_journal(&svc.paths().stats_file()).len(),
        3
    );
    assert_eq!(
        mango_mango::telemetry::install_id(svc.paths()),
        exp["installId"].as_str().unwrap()
    );
    assert_eq!(
        std::fs::read(root.join("install.json")).unwrap(),
        install_before,
        "install.json is not rewritten"
    );

    // nothing the game owns was touched (servers.dat, level.dat, region files, options.txt, jars, screenshots, covers)
    let after = game_files(root);
    for (file, bytes) in &before {
        assert_eq!(
            after.get(file),
            Some(bytes),
            "{} changed or vanished",
            file.display()
        );
    }

    // a save in 2.0 keeps everything readable and never writes a token in plain text
    svc.config_set(serde_json::json!({ "width": 1700 }))
        .unwrap();
    drop(svc);
    for f in ["accounts.json", "config.json"] {
        let text = std::fs::read_to_string(root.join(f)).unwrap();
        assert!(
            !text.contains("fixture-access-token") && !text.contains("fixture-refresh-token"),
            "{f} holds a plain token"
        );
    }
    let again = open(root).app_state().unwrap();
    assert_eq!(again.profiles.len(), 4);
    assert_eq!(again.config.width, 1700);
    assert_eq!(again.config.language, "en");
    assert_eq!(again.accounts.len(), 2);
}
