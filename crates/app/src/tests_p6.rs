#![allow(clippy::unwrap_used)]
//! Parts 6 and 7 of `docs/ipc.md`: instance icons (block gallery, custom
//! image, modpack logo) and backgrounds (panorama, screenshot, upload).
//! Mock servers, fake jars and temp dirs only.

use crate::backgrounds::{extract_panoramas, panoramas_root};
use crate::block_icons::{blocks_root, extract_blocks, local_jar, texture_names, BLOCKS, FACES};
use crate::image_protocol::{resolve, Image, ImageError};
use crate::tests_p2::{json_ok, serve, Req};
use crate::tests_p4::{zip_bytes, Sink};
use crate::*;
use mango_core::Paths;
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------- helpers

fn png_bytes(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    let img = image::RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)));
    crate::icon_images::encode_png(&img).unwrap()
}

fn grey(name: &str) -> u8 {
    name.bytes().fold(0u8, |a, b| a.wrapping_add(b))
}

fn tex_entry(name: &str) -> String {
    format!("assets/minecraft/textures/block/{name}.png")
}

fn pano_entry(n: u8) -> String {
    format!("assets/minecraft/textures/gui/title/background/panorama_{n}.png")
}

/// Every whitelisted texture as a 2x2 grey square (grey derived from the
/// name), then `overrides` replace entries, then `extra` are appended.
fn jar_entries(
    overrides: &[(String, Vec<u8>)],
    extra: &[(String, Vec<u8>)],
    panorama: bool,
) -> Vec<(String, Vec<u8>)> {
    let mut entries: Vec<(String, Vec<u8>)> = texture_names()
        .into_iter()
        .map(|n| {
            let g = grey(n);
            (tex_entry(n), png_bytes(2, 2, |_, _| [g, g, g, 255]))
        })
        .collect();
    if panorama {
        for n in 0..6u8 {
            entries.push((pano_entry(n), png_bytes(2, 2, |_, _| [n, 1, 2, 255])));
        }
    }
    for (name, data) in overrides {
        entries.retain(|(n, _)| n != name);
        entries.push((name.clone(), data.clone()));
    }
    entries.extend(extra.iter().cloned());
    entries
}

fn jar_bytes(entries: &[(String, Vec<u8>)]) -> Vec<u8> {
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    zip_bytes(&refs)
}

fn write_jar(path: &Path, entries: &[(String, Vec<u8>)]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, jar_bytes(entries)).unwrap();
}

fn px(path: &Path) -> image::RgbaImage {
    image::open(path).unwrap().to_rgba8()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else {
                out.push(p);
            }
        }
    }
}

fn all_files(dir: &Path) -> Vec<PathBuf> {
    let mut v = Vec::new();
    walk(dir, &mut v);
    v
}

async fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..400 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition not reached in time");
}

struct Env {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
}

fn opts(manifest_url: &str) -> ServiceOptions {
    ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        partners: Vec::new(),
        manifest_url: manifest_url.to_string(),
        icon_hosts: vec!["127.0.0.1".into()],
        icon_allow_http: true,
        ..ServiceOptions::default()
    }
}

fn env_with(o: ServiceOptions) -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let svc = AppService::new(Paths::new(tmp.path().join("data")), sink.clone(), o).unwrap();
    Env { tmp, svc, sink }
}

/// No network at all: the manifest points at a closed port.
fn env() -> Env {
    env_with(opts("http://127.0.0.1:1/manifest.json"))
}

impl Env {
    fn paths(&self) -> &Paths {
        self.svc.paths()
    }
    fn profile(&self, mc: &str) -> ProfileView {
        self.svc
            .profile_create("P".into(), mc.into(), "vanilla".into(), None)
            .unwrap()
    }
    fn install_jar(&self, version: &str, entries: &[(String, Vec<u8>)]) -> PathBuf {
        let jar = self
            .paths()
            .versions()
            .join(version)
            .join(format!("{version}.jar"));
        write_jar(&jar, entries);
        jar
    }
    fn reopen(&self) -> AppService {
        AppService::new(
            Paths::new(self.tmp.path().join("data")),
            Arc::new(Sink::default()),
            opts("http://127.0.0.1:1/x"),
        )
        .unwrap()
    }
    fn view(&self, id: &str) -> ProfileView {
        self.svc
            .app_state()
            .unwrap()
            .profiles
            .into_iter()
            .find(|p| p.id == id)
            .unwrap()
    }
    fn blocks_events(&self) -> Vec<BlockIconsStatus> {
        self.sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                Event::BlocksStatus(s) => Some(s),
                _ => None,
            })
            .collect()
    }
    async fn wait_blocks(&self) -> BlockIconsStatus {
        wait_for(|| self.svc.block_icons_status().state != BlocksState::Preparing).await;
        self.svc.block_icons_status()
    }
}

/// Mojang stand-in: manifest, version meta and the client jar.
async fn mojang(jar: Vec<u8>, sha1_override: Option<String>) -> (String, Arc<Mutex<Vec<Req>>>) {
    serve(move |base| {
        let base = base.to_string();
        let sha = sha1_override.unwrap_or_else(|| hex::encode(Sha1::digest(&jar)));
        Arc::new(move |r: &Req| match r.path.as_str() {
            "/manifest.json" => json_ok(json!({
                "latest": {"release": "1.21.1", "snapshot": "1.21.1"},
                "versions": [{"id": "1.21.1", "type": "release", "url": format!("{base}/meta.json"),
                              "releaseTime": "2024-01-01T00:00:00Z"}]
            })),
            "/meta.json" => json_ok(json!({
                "downloads": {"client": {"url": format!("{base}/client.jar"), "sha1": sha, "size": jar.len()}}
            })),
            "/client.jar" => (200, jar.clone()),
            _ => (404, b"{}".to_vec()),
        })
    })
    .await
}

fn plain_extract(
    entries: &[(String, Vec<u8>)],
    force: bool,
) -> (tempfile::TempDir, PathBuf, Res<Vec<&'static str>>) {
    let tmp = tempfile::tempdir().unwrap();
    let jar = tmp.path().join("client.jar");
    write_jar(&jar, entries);
    let root = tmp.path().join("cache/block-icons");
    let r = extract_blocks(&jar, &root, "1.21.1", force);
    (tmp, root, r)
}

// ---------------------------------------------------------------- extraction

#[test]
fn every_face_is_extracted_at_native_resolution() {
    let (_tmp, root, r) = plain_extract(&jar_entries(&[], &[], false), false);
    assert_eq!(r.unwrap().len(), BLOCKS.len());
    let dir = root.join("1.21.1");
    for b in BLOCKS {
        for f in FACES {
            let img = px(&dir.join(b.id).join(format!("{f}.png")));
            assert_eq!(img.dimensions(), (2, 2), "{} {f}", b.id);
        }
    }
    // Distinct sources per face.
    let g = |n: &str| grey(n);
    assert_ne!(g("crafting_table_top"), g("crafting_table_side"));
    assert_ne!(g("crafting_table_side"), g("crafting_table_front"));
    let face = |id: &str, f: &str| px(&dir.join(id).join(format!("{f}.png"))).get_pixel(0, 0).0;
    assert_eq!(face("crafting_table", "top")[0], g("crafting_table_top"));
    assert_eq!(face("crafting_table", "left")[0], g("crafting_table_side"));
    assert_eq!(
        face("crafting_table", "right")[0],
        g("crafting_table_front")
    );
    assert_eq!(face("furnace", "left")[0], g("furnace_side"));
    assert_eq!(face("furnace", "right")[0], g("furnace_front"));
    assert_eq!(face("oak_log", "top")[0], g("oak_log_top"));
    assert_eq!(face("oak_log", "left")[0], g("oak_log"));
    assert_eq!(face("oak_log", "right")[0], g("oak_log"));
    assert_eq!(face("bookshelf", "top")[0], g("oak_planks"));
    assert_eq!(face("bookshelf", "right")[0], g("bookshelf"));
    assert_eq!(face("dirt", "top")[0], g("dirt"));
    // ready marker and current pointer
    assert!(dir.join("ready.json").is_file());
    assert!(root.join("current.json").is_file());
}

#[test]
fn grass_top_is_tinted_and_side_is_composited_with_the_tinted_overlay() {
    let top = png_bytes(2, 2, |x, y| {
        if (x, y) == (0, 0) {
            [200, 100, 50, 255]
        } else {
            [255, 255, 255, 255]
        }
    });
    let side = png_bytes(2, 2, |_, _| [100, 60, 20, 255]);
    let overlay = png_bytes(2, 2, |x, y| match (x, y) {
        (0, 0) => [200, 200, 200, 255],
        (1, 0) => [0, 0, 0, 0],
        (0, 1) => [255, 255, 255, 128],
        _ => [0, 0, 0, 255],
    });
    let entries = jar_entries(
        &[
            (tex_entry("grass_block_top"), top),
            (tex_entry("grass_block_side"), side),
            (tex_entry("grass_block_side_overlay"), overlay),
        ],
        &[],
        false,
    );
    let (_tmp, root, r) = plain_extract(&entries, false);
    r.unwrap();
    let dir = root.join("1.21.1/grass_block");
    let t = px(&dir.join("top.png"));
    // (c * tint + 127) / 255 with tint = 0x91 0xBD 0x59
    assert_eq!(t.get_pixel(0, 0).0, [114, 74, 17, 255]);
    assert_eq!(t.get_pixel(1, 1).0, [145, 189, 89, 255]);
    for f in ["left", "right"] {
        let s = px(&dir.join(format!("{f}.png")));
        // opaque overlay: exactly the tinted overlay
        assert_eq!(s.get_pixel(0, 0).0, [114, 148, 70, 255], "{f}");
        // transparent overlay: the untouched side texture
        assert_eq!(s.get_pixel(1, 0).0, [100, 60, 20, 255], "{f}");
        // half transparent tinted white over the side texture
        assert_eq!(s.get_pixel(0, 1).0, [123, 125, 55, 255], "{f}");
        // tinted black stays black
        assert_eq!(s.get_pixel(1, 1).0, [0, 0, 0, 255], "{f}");
    }
}

#[test]
fn animated_strips_use_the_first_square_frame() {
    // 2x6: frame 0 is rows 0-1.
    let strip = png_bytes(2, 6, |_, y| {
        if y < 2 {
            [10, 20, 30, 255]
        } else {
            [200, 0, 0, 255]
        }
    });
    let entries = jar_entries(&[(tex_entry("stone"), strip)], &[], false);
    let (_tmp, root, r) = plain_extract(&entries, false);
    r.unwrap();
    let img = px(&root.join("1.21.1/stone/top.png"));
    assert_eq!(img.dimensions(), (2, 2));
    assert!(img.pixels().all(|p| p.0 == [10, 20, 30, 255]));
}

#[test]
fn only_whitelisted_entries_are_read_and_nothing_escapes_the_cache() {
    let evil = b"EVIL".to_vec();
    let extra = vec![
        (
            "assets/minecraft/textures/block/../../../evil.png".to_string(),
            evil.clone(),
        ),
        ("../evil.txt".to_string(), evil.clone()),
        ("/abs/evil.png".to_string(), evil.clone()),
        (
            tex_entry("not_in_the_list"),
            png_bytes(2, 2, |_, _| [1, 2, 3, 255]),
        ),
        (
            "assets/minecraft/textures/entity/creeper.png".to_string(),
            evil,
        ),
    ];
    let (tmp, root, r) = plain_extract(&jar_entries(&[], &extra, false), false);
    r.unwrap();
    let files = all_files(tmp.path());
    for f in &files {
        let rel = f
            .strip_prefix(tmp.path())
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(
            !rel.contains("evil") && !rel.contains("not_in_the_list") && !rel.contains("creeper"),
            "{rel}"
        );
    }
    // Only the jar itself and our own outputs exist.
    for f in files {
        assert!(
            f == tmp.path().join("client.jar") || f.starts_with(&root),
            "{f:?}"
        );
    }
    assert!(!tmp.path().join("evil.txt").exists());
    assert!(!tmp.path().join("cache/evil.png").exists());
}

#[test]
fn a_texture_over_the_size_cap_is_refused_and_its_block_skipped() {
    let big = vec![0u8; 1024 * 1024 + 1];
    let entries = jar_entries(&[(tex_entry("dirt"), big)], &[], false);
    let (_tmp, root, r) = plain_extract(&entries, false);
    let done = r.unwrap();
    assert!(!done.contains(&"dirt"));
    assert!(done.contains(&"stone"));
    assert!(!root.join("1.21.1/dirt").exists());
}

#[test]
fn a_jar_without_the_textures_fails_with_a_stable_code() {
    let entries = vec![(tex_entry("dirt"), png_bytes(2, 2, |_, _| [1, 1, 1, 255]))];
    let (_tmp, root, r) = plain_extract(&entries, false);
    assert_eq!(r.unwrap_err().code, "blocks.incomplete");
    assert!(!root.join("1.21.1/ready.json").exists());
    // Not a zip at all.
    let tmp = tempfile::tempdir().unwrap();
    let jar = tmp.path().join("x.jar");
    std::fs::write(&jar, b"nope").unwrap();
    let err = extract_blocks(&jar, &tmp.path().join("r"), "1.21.1", false).unwrap_err();
    assert_eq!(err.code, "blocks.jar_unreadable");
    // Unsafe version folder.
    let err = extract_blocks(&jar, &tmp.path().join("r"), "../x", false).unwrap_err();
    assert_eq!(err.code, "blocks.bad_version");
}

#[test]
fn extraction_is_idempotent_and_force_re_extracts() {
    let tmp = tempfile::tempdir().unwrap();
    let jar = tmp.path().join("client.jar");
    write_jar(&jar, &jar_entries(&[], &[], false));
    let root = tmp.path().join("b");
    extract_blocks(&jar, &root, "1.21.1", false).unwrap();
    let face = root.join("1.21.1/dirt/top.png");
    let original = std::fs::read(&face).unwrap();
    std::fs::write(&face, b"MARK").unwrap();
    std::fs::remove_file(&jar).unwrap(); // not even needed without force
    extract_blocks(&jar, &root, "1.21.1", false).unwrap();
    assert_eq!(std::fs::read(&face).unwrap(), b"MARK");
    write_jar(&jar, &jar_entries(&[], &[], false));
    extract_blocks(&jar, &root, "1.21.1", true).unwrap();
    assert_eq!(std::fs::read(&face).unwrap(), original);
    // A damaged earlier run is repaired without force.
    std::fs::remove_file(root.join("1.21.1/stone/left.png")).unwrap();
    extract_blocks(&jar, &root, "1.21.1", false).unwrap();
    assert!(root.join("1.21.1/stone/left.png").is_file());
}

// ---------------------------------------------------------------- jar selection and the job

#[test]
fn the_newest_installed_release_is_the_source_jar() {
    let e = env();
    assert!(local_jar(e.paths()).is_none());
    let entries = jar_entries(&[], &[], false);
    for v in [
        "1.20.1",
        "1.21.1",
        "1.9",
        "24w14a",
        "fabric-loader-0.16.9-1.21.1",
    ] {
        e.install_jar(v, &entries);
    }
    // a release folder without a jar does not count
    std::fs::create_dir_all(e.paths().versions().join("1.22")).unwrap();
    assert_eq!(local_jar(e.paths()).unwrap().0, "1.21.1");
}

#[tokio::test]
async fn prepare_uses_an_installed_jar_without_any_network() {
    let e = env();
    e.install_jar("1.20.1", &jar_entries(&[], &[], true));
    assert_eq!(e.svc.block_icons_status().state, BlocksState::Missing);
    let st = e.svc.block_icons_prepare(false).await.unwrap();
    assert_eq!(st.state, BlocksState::Preparing);
    let done = e.wait_blocks().await;
    assert_eq!(done.state, BlocksState::Ready);
    assert_eq!(done.mc_version.as_deref(), Some("1.20.1"));
    assert_eq!(done.blocks.len(), BLOCKS.len());
    assert!(done.error_code.is_none());
    let grass = done.blocks.iter().find(|b| b.id == "grass_block").unwrap();
    assert_eq!(grass.faces["top"], "mangoimg://block/grass_block/top");
    assert_eq!(grass.faces["left"], "mangoimg://block/grass_block/left");
    assert_eq!(grass.faces["right"], "mangoimg://block/grass_block/right");
    // events: preparing first, ready last
    let ev = e.blocks_events();
    assert_eq!(ev.first().unwrap().state, BlocksState::Preparing);
    assert_eq!(ev.last().unwrap().state, BlocksState::Ready);
    // JSON shape
    let v = serde_json::to_value(&done).unwrap();
    assert_eq!(v["state"], "ready");
    assert_eq!(v["mcVersion"], "1.20.1");
    assert!(v.get("errorCode").is_none());
    // the panorama of the same jar came along
    assert!(panoramas_root(e.paths()).join("1.20.1/5.png").is_file());
    // rerun is idempotent
    e.svc.block_icons_prepare(false).await.unwrap();
    assert_eq!(e.wait_blocks().await.state, BlocksState::Ready);
}

#[tokio::test]
async fn prepare_downloads_only_the_client_jar_of_the_latest_release() {
    let jar = jar_bytes(&jar_entries(&[], &[], true));
    let (base, seen) = mojang(jar.clone(), None).await;
    let e = env_with(opts(&format!("{base}/manifest.json")));
    e.svc.block_icons_prepare(false).await.unwrap();
    let done = e.wait_blocks().await;
    assert_eq!(done.state, BlocksState::Ready, "{done:?}");
    assert_eq!(done.mc_version.as_deref(), Some("1.21.1"));
    let cached = blocks_root(e.paths()).join("_jar/1.21.1.jar");
    assert_eq!(std::fs::read(cached).unwrap(), jar);
    let paths: Vec<String> = seen
        .lock()
        .unwrap()
        .iter()
        .map(|r| r.path.clone())
        .collect();
    assert_eq!(paths, ["/manifest.json", "/meta.json", "/client.jar"]);
    // no libraries or assets were touched
    assert!(!e.paths().libraries().join("x").exists());
    assert!(std::fs::read_dir(e.paths().versions()).map_or(true, |d| d.count() == 0));
    // The cached jar is reused afterwards: no further request.
    e.svc.block_icons_prepare(true).await.unwrap();
    assert_eq!(e.wait_blocks().await.state, BlocksState::Ready);
    assert_eq!(seen.lock().unwrap().len(), 3);
}

#[tokio::test]
async fn a_jar_with_a_wrong_checksum_is_rejected_and_reported() {
    let jar = jar_bytes(&jar_entries(&[], &[], false));
    let (base, _) = mojang(jar, Some("0".repeat(40))).await;
    let e = env_with(opts(&format!("{base}/manifest.json")));
    e.svc.block_icons_prepare(false).await.unwrap();
    let st = e.wait_blocks().await;
    assert_eq!(st.state, BlocksState::Error);
    assert!(st.error_code.as_deref().is_some_and(|c| !c.is_empty()));
    assert!(st.blocks.is_empty());
    assert!(!blocks_root(e.paths()).join("_jar/1.21.1.jar").exists());
    assert_eq!(e.blocks_events().last().unwrap().state, BlocksState::Error);
    // an offline manifest is an error too, and a retry can succeed later
    let e2 = env();
    e2.svc.block_icons_prepare(false).await.unwrap();
    assert_eq!(e2.wait_blocks().await.state, BlocksState::Error);
    e2.install_jar("1.21.1", &jar_entries(&[], &[], false));
    e2.svc.block_icons_prepare(false).await.unwrap();
    assert_eq!(e2.wait_blocks().await.state, BlocksState::Ready);
}

#[tokio::test]
async fn auto_block_is_deterministic_and_only_set_when_ready() {
    let e = env();
    let p = e.profile("1.21.1");
    assert!(e.view(&p.id).auto_block.is_none());
    e.install_jar("1.21.1", &jar_entries(&[], &[], false));
    e.svc.block_icons_prepare(false).await.unwrap();
    e.wait_blocks().await;
    let a = e.view(&p.id).auto_block.unwrap();
    assert!(BLOCKS.iter().any(|b| b.id == a));
    assert_eq!(e.view(&p.id).auto_block.as_deref(), Some(a.as_str()));
    assert_eq!(
        e.reopen()
            .app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|x| x.id == p.id)
            .unwrap()
            .auto_block
            .as_deref(),
        Some(a.as_str())
    );
    // The hash spreads over the gallery.
    let mut seen = std::collections::HashSet::new();
    for _ in 0..40 {
        seen.insert(e.profile("1.21.1").id).then_some(());
    }
    let picks: std::collections::HashSet<String> = e
        .svc
        .app_state()
        .unwrap()
        .profiles
        .iter()
        .filter_map(|p| p.auto_block.clone())
        .collect();
    assert!(picks.len() > 5, "{picks:?}");
    // JSON
    let v = serde_json::to_value(e.view(&p.id)).unwrap();
    assert_eq!(v["autoBlock"], json!(a));
}

// ---------------------------------------------------------------- protocol

#[test]
fn block_panorama_and_background_urls_follow_a_strict_policy() {
    let e = env();
    let p = e.profile("1.21.1");
    let jar = e.install_jar("1.21.1", &jar_entries(&[], &[], true));
    extract_blocks(&jar, &blocks_root(e.paths()), "1.21.1", false).unwrap();
    extract_panoramas(&jar, e.paths(), "1.21.1", false).unwrap();
    let ok = |u: &str| {
        matches!(
            resolve(e.paths(), u),
            Ok(Image::File {
                mime: "image/png",
                ..
            })
        )
    };
    for u in [
        "mangoimg://block/dirt/top",
        "mangoimg://block/grass_block/left",
        "mangoimg://block/lapis_block/right",
        "mangoimg://panorama/1.21.1/0",
        "mangoimg://panorama/1.21.1/5",
        "http://mangoimg.localhost/block/dirt/top",
    ] {
        assert!(ok(u), "{u}");
    }
    let err = |u: &str| resolve(e.paths(), u).unwrap_err();
    for u in [
        "mangoimg://block/DIRT/top",
        "mangoimg://block/../top",
        "mangoimg://block/dirt/bottom",
        "mangoimg://block/dirt",
        "mangoimg://block/dirt/top/x",
        "mangoimg://block//top",
        "mangoimg://block/dirt/top?x=1",
        "mangoimg://block/dirt/top.png",
        "mangoimg://block/di%72t/top",
        "mangoimg://block/dirt\\x/top",
        "mangoimg://panorama/../0",
        "mangoimg://panorama/.hidden/0",
        "mangoimg://panorama/1.21.1/6",
        "mangoimg://panorama/1.21.1/00",
        "mangoimg://panorama/1.21.1/0.png",
        "mangoimg://panorama//0",
        "mangoimg://panorama/1.21.1",
        "mangoimg://panorama/a%2fb/0",
        "mangoimg://panorama/1.21.1/0/1",
        "mangoimg://cover/not-a-uuid-bg",
        "mangoimg://cover/a/b",
        "mangoimg://head/00000000-0000-0000-0000-000000000000/x",
    ] {
        assert_eq!(err(u), ImageError::BadUrl, "{u}");
    }
    for u in [
        "mangoimg://block/bogus_block/top",
        "mangoimg://panorama/1.20.1/0",
        &format!("mangoimg://cover/{}-bg", p.id),
    ] {
        assert_eq!(err(u), ImageError::NotFound, "{u}");
    }
    // Nothing else of the cache is reachable through these kinds.
    assert_eq!(err("mangoimg://block/_jar/top"), ImageError::NotFound);
    assert_eq!(err("mangoimg://f/Zm9v"), ImageError::Forbidden);
}

#[cfg(unix)]
#[test]
fn a_symlink_out_of_the_cache_is_never_served() {
    let e = env();
    let jar = e.install_jar("1.21.1", &jar_entries(&[], &[], true));
    extract_blocks(&jar, &blocks_root(e.paths()), "1.21.1", false).unwrap();
    extract_panoramas(&jar, e.paths(), "1.21.1", false).unwrap();
    let outside = e.tmp.path().join("secret.png");
    std::fs::write(&outside, png_bytes(2, 2, |_, _| [9, 9, 9, 255])).unwrap();
    let face = blocks_root(e.paths()).join("1.21.1/dirt/top.png");
    std::fs::remove_file(&face).unwrap();
    std::os::unix::fs::symlink(&outside, &face).unwrap();
    assert_eq!(
        resolve(e.paths(), "mangoimg://block/dirt/top").unwrap_err(),
        ImageError::Forbidden
    );
    let pano = panoramas_root(e.paths()).join("1.21.1/0.png");
    std::fs::remove_file(&pano).unwrap();
    std::os::unix::fs::symlink(&outside, &pano).unwrap();
    assert_eq!(
        resolve(e.paths(), "mangoimg://panorama/1.21.1/0").unwrap_err(),
        ImageError::Forbidden
    );
}

// ---------------------------------------------------------------- icons

#[test]
fn legacy_profiles_without_icon_fields_load_with_automatic_defaults() {
    let id = "12345678-1234-1234-1234-123456789abc";
    let p = mango_core::Profile::from_stored(
        &json!({"id": id, "name": "Old", "cover": false, "mods": []}),
        1,
    )
    .unwrap();
    assert!(p.icon_block.is_none() && p.background.is_none());
    // and a file on disk
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("accounts.json"),
        serde_json::to_vec(&json!({"accounts": [], "profiles": [{"id": id, "name": "Old", "mcVersion": "1.21.1", "loader": "vanilla", "mods": []}]})).unwrap(),
    )
    .unwrap();
    let svc = AppService::new(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        opts("http://127.0.0.1:1/m"),
    )
    .unwrap();
    let v = serde_json::to_value(
        svc.app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|p| p.id == id)
            .unwrap(),
    )
    .unwrap();
    assert!(v["icon"].is_null());
    assert!(v["autoBlock"].is_null());
    assert_eq!(v["background"], json!({"kind": "auto"}));
    assert!(v["coverUrl"].is_null());
}

fn write_png(path: &Path, w: u32, h: u32) {
    std::fs::write(
        path,
        png_bytes(w, h, |x, y| [(x % 256) as u8, (y % 256) as u8, 7, 255]),
    )
    .unwrap();
}

#[tokio::test]
async fn setting_and_clearing_a_block_icon_persists() {
    let e = env();
    let p = e.profile("1.21.1");
    let err = e
        .svc
        .profile_icon_set_block(p.id.clone(), "bedrock".into())
        .unwrap_err();
    assert_eq!(err.code, "block_unknown");
    assert_eq!(
        e.svc
            .profile_icon_set_block("nope".into(), "dirt".into())
            .unwrap_err()
            .code,
        "invalid_profile_id"
    );
    let v = e
        .svc
        .profile_icon_set_block(p.id.clone(), "diamond_ore".into())
        .unwrap();
    assert_eq!(
        serde_json::to_value(&v.icon).unwrap(),
        json!({"kind": "block", "block": "diamond_ore"})
    );
    // survives a restart
    let again = e.reopen();
    let v2 = again
        .app_state()
        .unwrap()
        .profiles
        .into_iter()
        .find(|x| x.id == p.id)
        .unwrap();
    assert_eq!(v2.icon, v.icon);
    // an image replaces the block, clearing returns to automatic
    let src = e.tmp.path().join("pic.png");
    write_png(&src, 300, 100);
    let v = e
        .svc
        .profile_icon_set_image(p.id.clone(), src.display().to_string())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&v.icon).unwrap(),
        json!({"kind": "image"})
    );
    assert!(v.cover_url.is_some());
    let cover = px(&e.paths().cover_file(&p.id).unwrap());
    assert_eq!(cover.dimensions(), (256, 256));
    // choosing a block removes the image
    let v = e
        .svc
        .profile_icon_set_block(p.id.clone(), "tnt".into())
        .unwrap();
    assert_eq!(
        serde_json::to_value(&v.icon).unwrap(),
        json!({"kind": "block", "block": "tnt"})
    );
    assert!(v.cover_url.is_none());
    assert!(!e.paths().cover_file(&p.id).unwrap().exists());
    let v = e.svc.profile_icon_clear(p.id.clone()).unwrap();
    assert!(v.icon.is_none() && v.cover_url.is_none());
    assert!(e
        .reopen()
        .app_state()
        .unwrap()
        .profiles
        .iter()
        .find(|x| x.id == p.id)
        .unwrap()
        .icon
        .is_none());
}

#[tokio::test]
async fn image_icons_are_center_cropped_and_resized_to_256() {
    let e = env();
    let p = e.profile("1.21.1");
    // 400x200: the middle 200x200 survives; left/right thirds are cut.
    let src = e.tmp.path().join("wide.png");
    std::fs::write(
        &src,
        png_bytes(400, 200, |x, _| {
            if (100..300).contains(&x) {
                [255, 0, 0, 255]
            } else {
                [0, 0, 255, 255]
            }
        }),
    )
    .unwrap();
    e.svc
        .profile_icon_set_image(p.id.clone(), src.display().to_string())
        .await
        .unwrap();
    let cover = px(&e.paths().cover_file(&p.id).unwrap());
    assert_eq!(cover.dimensions(), (256, 256));
    assert_eq!(cover.get_pixel(0, 128).0, [255, 0, 0, 255]);
    assert_eq!(cover.get_pixel(255, 128).0, [255, 0, 0, 255]);
    // no temp leftovers
    let names: Vec<_> = std::fs::read_dir(e.paths().covers())
        .unwrap()
        .flatten()
        .map(|d| d.file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, [format!("{}.png", p.id)]);
    // other formats
    for (name, fmt) in [
        ("a.jpg", image::ImageFormat::Jpeg),
        ("a.jpeg", image::ImageFormat::Jpeg),
        ("a.gif", image::ImageFormat::Gif),
        ("a.webp", image::ImageFormat::WebP),
    ] {
        let path = e.tmp.path().join(name);
        let img = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(
            32,
            48,
            image::Rgba([50, 100, 150, 255]),
        ));
        let img = if fmt == image::ImageFormat::Jpeg {
            image::DynamicImage::ImageRgb8(img.to_rgb8())
        } else {
            img
        };
        if img.save_with_format(&path, fmt).is_err() {
            continue; // an encoder that is not compiled in
        }
        e.svc
            .profile_icon_set_image(p.id.clone(), path.display().to_string())
            .await
            .unwrap_or_else(|err| panic!("{name}: {err}"));
        assert_eq!(
            px(&e.paths().cover_file(&p.id).unwrap()).dimensions(),
            (256, 256),
            "{name}"
        );
    }
}

#[tokio::test]
async fn bad_icon_uploads_map_to_stable_codes() {
    let e = env();
    let p = e.profile("1.21.1");
    let set = |name: &str, bytes: &[u8]| {
        let path = e.tmp.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        e.svc
            .profile_icon_set_image(p.id.clone(), path.display().to_string())
    };
    assert_eq!(
        set("x.png", b"not a png").await.unwrap_err().code,
        "image_decode"
    );
    assert_eq!(
        set("x.txt", b"hello").await.unwrap_err().code,
        "image_unsupported_type"
    );
    assert_eq!(
        set("x.svg", b"<svg/>").await.unwrap_err().code,
        "image_unsupported_type"
    );
    assert_eq!(
        set("x.png", &vec![0u8; 10 * 1024 * 1024 + 1])
            .await
            .unwrap_err()
            .code,
        "image_too_large"
    );
    // wrong extension, right content is still decoded by content
    assert!(set("x.jpg", &png_bytes(4, 4, |_, _| [1, 2, 3, 255]))
        .await
        .is_ok());
    let missing = e
        .svc
        .profile_icon_set_image(
            p.id.clone(),
            e.tmp.path().join("gone.png").display().to_string(),
        )
        .await;
    assert_eq!(missing.unwrap_err().code, "not_found");
    let dir = e.tmp.path().join("dir.png");
    std::fs::create_dir(&dir).unwrap();
    assert_eq!(
        e.svc
            .profile_icon_set_image(p.id.clone(), dir.display().to_string())
            .await
            .unwrap_err()
            .code,
        "not_found"
    );
    // a failed upload leaves the previous icon alone
    assert!(e.view(&p.id).cover_url.is_some());
}

// ---------------------------------------------------------------- modpack logo

#[tokio::test]
async fn a_pack_logo_is_downloaded_from_allowed_hosts_only() {
    let logo = png_bytes(64, 32, |_, _| [0, 200, 0, 255]);
    let (base, seen) = serve({
        let logo = logo.clone();
        move |_| {
            Arc::new(move |r: &Req| match r.path.as_str() {
                "/logo.png" => (200, logo.clone()),
                "/junk.png" => (200, b"junk".to_vec()),
                _ => (404, Vec::new()),
            })
        }
    })
    .await;
    let e = env();
    let p = e.profile("1.21.1");
    e.svc
        .apply_pack_logo(&p.id, Some(&format!("{base}/logo.png")))
        .await;
    let v = e.view(&p.id);
    assert_eq!(
        serde_json::to_value(&v.icon).unwrap(),
        json!({"kind": "image"})
    );
    assert_eq!(
        px(&e.paths().cover_file(&p.id).unwrap()).dimensions(),
        (256, 256)
    );
    assert_eq!(v.icon_source.as_deref(), Some("modpack"));
    assert!(e.paths().pack_logo_file(&p.id).unwrap().is_file());
    // failures are silent and change nothing
    let q = e.profile("1.21.1");
    for url in [
        format!("{base}/junk.png"),
        format!("{base}/missing.png"),
        "https://evil.example.com/x.png".into(),
        "file:///etc/passwd".into(),
    ] {
        e.svc.apply_pack_logo(&q.id, Some(&url)).await;
        assert!(e.view(&q.id).icon.is_none(), "{url}");
    }
    e.svc.apply_pack_logo(&q.id, None).await;
    assert!(seen.lock().unwrap().iter().all(|r| r.path != "/etc/passwd"));
    // https on an unlisted host is never contacted: host policy with the real defaults
    let strict = env_with(ServiceOptions {
        icon_hosts: vec!["cdn.modrinth.com".into()],
        icon_allow_http: false,
        ..opts("http://127.0.0.1:1/m")
    });
    let r = strict.profile("1.21.1");
    strict
        .svc
        .apply_pack_logo(&r.id, Some(&format!("{base}/logo.png")))
        .await;
    assert!(strict.view(&r.id).icon.is_none());
}

// ---------------------------------------------------------------- panoramas

#[test]
fn panorama_extraction_is_whitelisted_bounded_byte_exact_and_idempotent() {
    let e = env();
    let evil = b"EVIL".to_vec();
    let extra = vec![
        (
            "assets/minecraft/textures/gui/title/background/panorama_6.png".to_string(),
            evil.clone(),
        ),
        (
            "assets/minecraft/textures/gui/title/background/panorama_overlay.png".to_string(),
            evil.clone(),
        ),
        (
            "assets/minecraft/textures/gui/title/background/../../../../../../evil.png".to_string(),
            evil,
        ),
    ];
    let jar = e.tmp.path().join("j.jar");
    let entries = jar_entries(&[], &extra, true);
    write_jar(&jar, &entries);
    extract_panoramas(&jar, e.paths(), "1.21.1", false).unwrap();
    let dir = panoramas_root(e.paths()).join("1.21.1");
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|d| d.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        ["0.png", "1.png", "2.png", "3.png", "4.png", "5.png"]
    );
    for n in 0..6u8 {
        let original = &entries
            .iter()
            .find(|(name, _)| *name == pano_entry(n))
            .unwrap()
            .1;
        assert_eq!(
            &std::fs::read(dir.join(format!("{n}.png"))).unwrap(),
            original,
            "byte exact"
        );
    }
    // idempotent: untouched files stay, force rewrites
    std::fs::write(dir.join("0.png"), b"MARK").unwrap();
    extract_panoramas(&jar, e.paths(), "1.21.1", false).unwrap();
    assert_eq!(std::fs::read(dir.join("0.png")).unwrap(), b"MARK");
    extract_panoramas(&jar, e.paths(), "1.21.1", true).unwrap();
    assert_ne!(std::fs::read(dir.join("0.png")).unwrap(), b"MARK");
    assert!(!e.paths().cache().join("evil.png").exists());
    // bad version folder
    assert_eq!(
        extract_panoramas(&jar, e.paths(), "../x", false)
            .unwrap_err()
            .code,
        "panorama.bad_version"
    );
}

#[test]
fn panorama_extraction_is_all_or_nothing_with_a_size_cap() {
    let e = env();
    let jar = e.tmp.path().join("j.jar");
    // one face missing
    let mut entries = jar_entries(&[], &[], true);
    entries.retain(|(n, _)| *n != pano_entry(3));
    write_jar(&jar, &entries);
    assert_eq!(
        extract_panoramas(&jar, e.paths(), "a", false)
            .unwrap_err()
            .code,
        "blocks.texture_missing"
    );
    assert!(!panoramas_root(e.paths()).join("a").exists());
    // one face over 8 MB
    let big = vec![0x89u8; 8 * 1024 * 1024 + 1];
    write_jar(&jar, &jar_entries(&[(pano_entry(2), big)], &[], true));
    assert_eq!(
        extract_panoramas(&jar, e.paths(), "b", false)
            .unwrap_err()
            .code,
        "blocks.texture_too_large"
    );
    assert!(!panoramas_root(e.paths()).join("b").exists());
    // not a PNG
    write_jar(
        &jar,
        &jar_entries(&[(pano_entry(1), b"GIF89a....".to_vec())], &[], true),
    );
    assert_eq!(
        extract_panoramas(&jar, e.paths(), "c", false)
            .unwrap_err()
            .code,
        "panorama.not_png"
    );
    assert!(!panoramas_root(e.paths()).join("c").exists());
}

#[tokio::test]
async fn panorama_for_profile_prefers_the_own_version_and_reports_the_source() {
    let e = env();
    let own = e.profile("1.21.1");
    // nothing installed and no network: nothing to show
    assert!(e
        .svc
        .panorama_for_profile(own.id.clone())
        .await
        .unwrap()
        .is_none());
    wait_for(|| lock_free(&e.svc)).await;
    // another version is installed (not the profile's own): the first answer
    // is null, the job extracts it, then it is the nearest set.
    e.install_jar("1.20.1", &jar_entries(&[], &[], true));
    // the job downloads nothing: it falls back to the local jar
    assert!(e
        .svc
        .panorama_for_profile(own.id.clone())
        .await
        .unwrap()
        .is_none());
    wait_ready(&e, &own.id, 1).await;
    let set = e
        .svc
        .panorama_for_profile(own.id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(set.mc_version, "1.20.1");
    assert_eq!(set.source, PanoramaSource::Nearest);
    assert_eq!(set.faces.len(), 6);
    assert_eq!(set.faces[3], "mangoimg://panorama/1.20.1/3");
    // its own jar arrives: the nearest set is returned meanwhile, then own
    e.install_jar("1.21.1", &jar_entries(&[], &[], true));
    let again = e
        .svc
        .panorama_for_profile(own.id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.source, PanoramaSource::Nearest);
    wait_ready(&e, &own.id, 2).await;
    let set = e
        .svc
        .panorama_for_profile(own.id.clone())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (set.mc_version.as_str(), set.source),
        ("1.21.1", PanoramaSource::Own)
    );
    let v = serde_json::to_value(&set).unwrap();
    assert_eq!(v["mcVersion"], "1.21.1");
    assert_eq!(v["source"], "own");
    for url in set.faces {
        assert!(
            matches!(resolve(e.paths(), &url), Ok(Image::File { .. })),
            "{url}"
        );
    }
    // another profile on 1.19 with no jar of its own uses the newest set
    let other = e.profile("1.19.4");
    let set = e.svc.panorama_for_profile(other.id).await.unwrap().unwrap();
    assert_eq!(
        (set.mc_version.as_str(), set.source),
        ("1.21.1", PanoramaSource::Nearest)
    );
    assert!(e.svc.panorama_for_profile("nope".into()).await.is_err());
}

fn lock_free(svc: &AppService) -> bool {
    crate::service::lock(&svc.inner.pano_busy).is_empty()
}

async fn wait_ready(e: &Env, id: &str, n: usize) {
    wait_for(|| {
        e.sink
            .events()
            .iter()
            .filter(|ev| matches!(ev, Event::PanoramaReady(p) if p.profile_id == id))
            .count()
            >= n
    })
    .await;
    wait_for(|| lock_free(&e.svc)).await;
}

#[tokio::test]
async fn panorama_for_profile_downloads_the_jar_when_nothing_is_installed() {
    let jar = jar_bytes(&jar_entries(&[], &[], true));
    let (base, _) = mojang(jar, None).await;
    let e = env_with(opts(&format!("{base}/manifest.json")));
    let p = e.profile("1.21.1");
    assert!(e
        .svc
        .panorama_for_profile(p.id.clone())
        .await
        .unwrap()
        .is_none());
    wait_ready(&e, &p.id, 1).await;
    let set = e.svc.panorama_for_profile(p.id).await.unwrap().unwrap();
    assert_eq!(set.source, PanoramaSource::Own); // the downloaded latest release is 1.21.1
}

// ---------------------------------------------------------------- backgrounds

fn shots(e: &Env, id: &str) -> PathBuf {
    let dir = e.paths().instance_dir(id).unwrap().join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn screenshot_and_upload_backgrounds_are_downscaled_and_persisted() {
    let e = env();
    let p = e.profile("1.21.1");
    assert_eq!(
        serde_json::to_value(&p.background).unwrap(),
        json!({"kind": "auto"})
    );
    let dir = shots(&e, &p.id);
    write_png(&dir.join("big.png"), 3000, 2000);
    write_png(&dir.join("small.png"), 640, 360);
    let v = e
        .svc
        .profile_background_set_screenshot(p.id.clone(), "big.png".into())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&v.background).unwrap(),
        json!({"kind": "image", "url": format!("mangoimg://cover/{}-bg", p.id)})
    );
    let bg = e.paths().background_file(&p.id).unwrap();
    assert_eq!(px(&bg).dimensions(), (1620, 1080));
    assert!(matches!(
        resolve(e.paths(), &format!("mangoimg://cover/{}-bg", p.id)),
        Ok(Image::File { .. })
    ));
    // small pictures are not upscaled
    e.svc
        .profile_background_set_screenshot(p.id.clone(), "small.png".into())
        .await
        .unwrap();
    assert_eq!(px(&bg).dimensions(), (640, 360));
    // survives a restart
    let again = e
        .reopen()
        .app_state()
        .unwrap()
        .profiles
        .into_iter()
        .find(|x| x.id == p.id)
        .unwrap();
    assert!(matches!(again.background, ProfileBackground::Image { .. }));
    // an upload: wide image is limited to 1920 wide
    let up = e.tmp.path().join("wide.png");
    write_png(&up, 4000, 500);
    e.svc
        .profile_background_set_image(p.id.clone(), up.display().to_string())
        .await
        .unwrap();
    assert_eq!(px(&bg).dimensions(), (1920, 240));
    // errors
    assert_eq!(
        e.svc
            .profile_background_set_image(
                p.id.clone(),
                e.tmp.path().join("a.exe").display().to_string()
            )
            .await
            .unwrap_err()
            .code,
        "image_unsupported_type"
    );
    let junk = e.tmp.path().join("junk.png");
    std::fs::write(&junk, b"junk").unwrap();
    assert_eq!(
        e.svc
            .profile_background_set_image(p.id.clone(), junk.display().to_string())
            .await
            .unwrap_err()
            .code,
        "image_decode"
    );
    // none, auto
    let v = e.svc.profile_background_clear(p.id.clone(), true).unwrap();
    assert_eq!(
        serde_json::to_value(&v.background).unwrap(),
        json!({"kind": "none"})
    );
    assert!(!bg.exists());
    assert!(matches!(
        e.reopen()
            .app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|x| x.id == p.id)
            .unwrap()
            .background,
        ProfileBackground::None
    ));
    e.svc
        .profile_background_set_screenshot(p.id.clone(), "small.png".into())
        .await
        .unwrap();
    let v = e.svc.profile_background_clear(p.id.clone(), false).unwrap();
    assert_eq!(
        serde_json::to_value(&v.background).unwrap(),
        json!({"kind": "auto"})
    );
    assert!(!bg.exists());
    // deleting the profile removes its picture
    e.svc
        .profile_background_set_screenshot(p.id.clone(), "small.png".into())
        .await
        .unwrap();
    let other = e.profile("1.21.1");
    e.svc.profile_delete(p.id.clone()).unwrap();
    assert!(!bg.exists());
    assert!(e
        .paths()
        .instance_dir(&other.id)
        .unwrap()
        .parent()
        .unwrap()
        .is_dir());
}

#[tokio::test]
async fn screenshot_names_must_be_plain_files_of_the_instance() {
    let e = env();
    let p = e.profile("1.21.1");
    let dir = shots(&e, &p.id);
    write_png(&dir.join("ok.png"), 10, 10);
    // another instance's file and a file next to the screenshots folder
    let q = e.profile("1.21.1");
    write_png(&shots(&e, &q.id).join("theirs.png"), 10, 10);
    write_png(
        &e.paths().instance_dir(&p.id).unwrap().join("level.png"),
        10,
        10,
    );
    for name in [
        "../level.png",
        "..\\level.png",
        "../../theirs.png",
        &format!("../../{}/screenshots/theirs.png", q.id),
        "sub/ok.png",
        "/etc/passwd",
        "ok.png/",
        "..",
        ".",
        "",
        "ok.txt",
        "ok.png\0",
        "ok.gif",
    ] {
        let err = e
            .svc
            .profile_background_set_screenshot(p.id.clone(), name.into())
            .await
            .unwrap_err();
        assert_eq!(err.code, "invalid_name", "{name:?}");
    }
    assert_eq!(
        e.svc
            .profile_background_set_screenshot(p.id.clone(), "missing.png".into())
            .await
            .unwrap_err()
            .code,
        "not_found"
    );
    assert_eq!(
        e.svc
            .profile_background_set_screenshot("nope".into(), "ok.png".into())
            .await
            .unwrap_err()
            .code,
        "invalid_profile_id"
    );
    assert!(!e.paths().background_file(&p.id).unwrap().exists());
    #[cfg(unix)]
    {
        let outside = e.tmp.path().join("outside.png");
        write_png(&outside, 10, 10);
        std::os::unix::fs::symlink(&outside, dir.join("link.png")).unwrap();
        let err = e
            .svc
            .profile_background_set_screenshot(p.id.clone(), "link.png".into())
            .await
            .unwrap_err();
        assert_eq!(err.code, "not_found");
        // a screenshots folder that is itself a link out of the instance
        let r = e.profile("1.21.1");
        let real = e.tmp.path().join("elsewhere");
        std::fs::create_dir_all(&real).unwrap();
        write_png(&real.join("x.png"), 10, 10);
        let inst = e.paths().instance_dir(&r.id).unwrap();
        std::fs::create_dir_all(&inst).unwrap();
        std::os::unix::fs::symlink(&real, inst.join("screenshots")).unwrap();
        let err = e
            .svc
            .profile_background_set_screenshot(r.id.clone(), "x.png".into())
            .await
            .unwrap_err();
        assert_eq!(err.code, "not_found");
    }
    assert!(e
        .svc
        .profile_background_set_screenshot(p.id.clone(), "ok.png".into())
        .await
        .is_ok());
}

#[test]
fn profile_json_gains_icon_auto_block_and_background() {
    let e = env();
    let p = e.profile("1.21.1");
    let v = serde_json::to_value(&p).unwrap();
    for key in ["icon", "autoBlock", "background"] {
        assert!(v.get(key).is_some(), "{key}");
    }
    assert_eq!(v["background"], json!({"kind": "auto"}));
    let _: Value = v;
}
