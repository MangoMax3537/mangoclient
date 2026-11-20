#![allow(clippy::unwrap_used)]
//! Part 9 (second group): instance import/export, crash analysis,
//! diagnostics export, groups and tags. Temp dirs and mock servers only.

use crate::blocked;
use crate::models::{BlockedFile, BlockedState, ContentType};
use crate::tests_p2::{json_ok, serve, Handler, Req};
use crate::tests_p4::{cf_file, cf_project, cf_server, zip_bytes, Sink, KEY};
use crate::*;
use mango_core::{Loader, Paths, Secret};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use sha2::Sha512;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct Fx {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
}

fn dead() -> String {
    "http://127.0.0.1:1".to_string()
}

fn fx_with(modrinth: &str, curse: &str, key: Option<&str>) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(Sink::default());
    let opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        partners: Vec::new(),
        icon_hosts: Vec::new(),
        modrinth_api: modrinth.to_string(),
        curseforge_api: curse.to_string(),
        curseforge_key: key.map(Secret::new),
        curseforge: mango_curseforge::Options {
            retries: 0,
            strict_download_urls: false,
            ..mango_curseforge::Options::default()
        },
        loader_endpoints: mango_loaders::Endpoints {
            fabric_meta: "http://127.0.0.1:1/v2".into(),
            quilt_meta: "http://127.0.0.1:1/v3".into(),
            neoforge_maven: "http://127.0.0.1:1/neo".into(),
            forge_maven: "http://127.0.0.1:1/forge".into(),
        },
        ..ServiceOptions::default()
    };
    let svc = AppService::new(Paths::new(tmp.path()), sink.clone(), opts).unwrap();
    Fx { tmp, svc, sink }
}

fn fx() -> Fx {
    fx_with(&dead(), &dead(), None)
}

impl Fx {
    fn paths(&self) -> Paths {
        Paths::new(self.tmp.path())
    }
    fn inst(&self, id: &str) -> PathBuf {
        self.paths().instance_dir(id).unwrap()
    }
    fn profile(&self, name: &str, mc: &str, loader: &str, lv: Option<&str>) -> String {
        self.svc
            .profile_create(name.into(), mc.into(), loader.into(), lv.map(Into::into))
            .unwrap()
            .id
    }
    /// A scratch folder outside the data dir (inputs and outputs).
    fn scratch(&self) -> PathBuf {
        let d = self.tmp.path().join("scratch");
        std::fs::create_dir_all(&d).unwrap();
        d
    }
    fn put(&self, id: &str, rel: &str, bytes: &[u8]) -> PathBuf {
        let p = self.inst(id).join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, bytes).unwrap();
        p
    }
    fn install_events(&self, id: &str) -> Vec<(u64, u64)> {
        self.sink
            .events()
            .into_iter()
            .filter_map(|e| match e {
                Event::InstallProgress(p) if p.profile_id == id => Some((p.done, p.total)),
                _ => None,
            })
            .collect()
    }
    fn profile_ids(&self) -> Vec<String> {
        self.svc
            .app_state()
            .unwrap()
            .profiles
            .into_iter()
            .map(|p| p.id)
            .collect()
    }
}

fn sha1_hex(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}

fn sha512_hex(b: &[u8]) -> String {
    hex::encode(Sha512::digest(b))
}

fn png_bytes() -> Vec<u8> {
    let img = image::RgbaImage::from_pixel(8, 8, image::Rgba([200, 30, 30, 255]));
    crate::icon_images::encode_png(&img).unwrap()
}

fn write_zip(dir: &Path, name: &str, entries: &[(&str, &[u8])]) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, zip_bytes(entries)).unwrap();
    p
}

fn zip_names(p: &Path) -> Vec<String> {
    let z = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
    let mut v: Vec<String> = z.file_names().map(str::to_string).collect();
    v.sort();
    v
}

fn zip_text(p: &Path, name: &str) -> String {
    let mut z = zip::ZipArchive::new(std::fs::File::open(p).unwrap()).unwrap();
    let mut s = String::new();
    z.by_name(name).unwrap().read_to_string(&mut s).unwrap();
    s
}

fn no_part_files(dir: &Path) {
    for e in std::fs::read_dir(dir).unwrap().flatten() {
        let n = e.file_name().to_string_lossy().into_owned();
        assert!(!n.contains(".part-"), "temp file left behind: {n}");
    }
}

// ---------------------------------------------------------------- groups and tags

#[test]
fn groups_and_tags_are_edited_through_profile_update() {
    let f = fx();
    let id = f.profile("A", "1.21.1", "vanilla", None);
    let v = f
        .svc
        .profile_update(
            id.clone(),
            json!({"group": "  Survival ", "tags": [" smp ", "SMP", "pvp"]}),
        )
        .unwrap();
    assert_eq!(v.group.as_deref(), Some("Survival"));
    assert_eq!(v.tags, ["smp", "pvp"]);
    let json = serde_json::to_value(&v).unwrap();
    assert_eq!(json["group"], "Survival");
    assert_eq!(json["tags"], json!(["smp", "pvp"]));
    // It survives a reload from disk.
    let again = AppService::new(
        Paths::new(f.tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            auto_update_check: false,
            partners: Vec::new(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let state = again.app_state().unwrap();
    let mine = state.profiles.iter().find(|p| p.id == id).unwrap();
    assert_eq!(mine.group.as_deref(), Some("Survival"));
    assert_eq!(mine.tags, ["smp", "pvp"]);
    // A whole view can be spread into a patch (derived fields are ignored).
    let again_patch = f
        .svc
        .profile_update(id.clone(), serde_json::to_value(&v).unwrap_or_default());
    assert_eq!(
        again_patch.unwrap_err().code,
        "validation",
        "id is immutable"
    );

    for bad in [
        json!({"group": "x".repeat(49)}),
        json!({"group": "a\nb"}),
        json!({"group": 3}),
        json!({"tags": "smp"}),
        json!({"tags": ["a", ""]}),
        json!({"tags": ["x".repeat(25)]}),
        json!({"tags": (0..13).map(|i| format!("t{i}")).collect::<Vec<_>>()}),
        json!({"tags": ["bad\u{7}tag"]}),
    ] {
        let e = f.svc.profile_update(id.clone(), bad.clone()).unwrap_err();
        assert_eq!(e.code, "validation", "{bad}");
    }
    // Nothing changed by the failures; clearing works.
    let tags = |f: &Fx| {
        f.svc
            .app_state()
            .unwrap()
            .profiles
            .into_iter()
            .find(|p| p.id == id)
            .unwrap()
            .tags
    };
    assert_eq!(tags(&f), ["smp", "pvp"]);
    let v = f
        .svc
        .profile_update(id.clone(), json!({"group": null, "tags": []}))
        .unwrap();
    assert_eq!((v.group, v.tags), (None, Vec::<String>::new()));
    // Duplicating keeps them.
    f.svc
        .profile_update(id.clone(), json!({"group": "G", "tags": ["x"]}))
        .unwrap();
    let dup = tokio::runtime::Builder::new_current_thread()
        .build()
        .unwrap()
        .block_on(f.svc.profile_duplicate(id))
        .unwrap();
    assert_eq!(
        (dup.group.as_deref(), dup.tags),
        (Some("G"), vec!["x".to_string()])
    );
}

#[test]
fn legacy_profile_files_load_without_group_and_tags() {
    let tmp = tempfile::tempdir().unwrap();
    let id = "12345678-1234-1234-1234-123456789abc";
    std::fs::write(
        tmp.path().join("accounts.json"),
        json!({"accounts": [], "profiles": [{
            "id": id, "name": "Legacy", "mcVersion": "1.20.1", "loader": "forge",
            "loaderVersion": "47.2.0", "created": 7
        }]})
        .to_string(),
    )
    .unwrap();
    let svc = AppService::new(
        Paths::new(tmp.path()),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            auto_update_check: false,
            partners: Vec::new(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let p = &svc.app_state().unwrap().profiles[0];
    assert_eq!(
        (p.name.as_str(), p.group.clone(), p.tags.clone()),
        ("Legacy", None, vec![])
    );
    let json = serde_json::to_value(p).unwrap();
    assert_eq!(json["group"], Value::Null);
    assert_eq!(json["tags"], json!([]));
}

// ---------------------------------------------------------------- import: fixtures

fn mrpack_index(name: &str, files: &[Value]) -> Vec<u8> {
    serde_json::to_vec(&json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1.0.0", "name": name,
        "dependencies": {"minecraft": "1.21.1", "fabric-loader": "0.16.9"},
        "files": files
    }))
    .unwrap()
}

const PRISM_CFG: &str = "[General]\nInstanceType=OneSix\nname=Cool Pack\niconKey=default\nOverrideMemory=true\nMaxMemAlloc=6144\n";

fn mmc_pack(extra: Value) -> Vec<u8> {
    let mut components = vec![
        json!({"uid": "net.minecraft", "version": "1.20.1", "important": true}),
        json!({"uid": "net.fabricmc.intermediary", "version": "1.20.1"}),
    ];
    components.extend(extra.as_array().unwrap().iter().cloned());
    serde_json::to_vec(&json!({"formatVersion": 1, "components": components})).unwrap()
}

fn prism_zip(dir: &Path, name: &str, prefix: &str, game: &str, loader: Value) -> PathBuf {
    let pack = mmc_pack(loader);
    let png = png_bytes();
    let entries: Vec<(String, Vec<u8>)> = vec![
        (
            format!("{prefix}instance.cfg"),
            PRISM_CFG.as_bytes().to_vec(),
        ),
        (format!("{prefix}mmc-pack.json"), pack),
        (format!("{prefix}icon.png"), png),
        (format!("{prefix}{game}/mods/a.jar"), b"jar-a".to_vec()),
        (
            format!("{prefix}{game}/options.txt"),
            b"lang:de_de".to_vec(),
        ),
        (format!("{prefix}{game}/config/x.toml"), b"x=1".to_vec()),
        (format!("{prefix}{game}/logs/latest.log"), b"noise".to_vec()),
        (
            format!("{prefix}{game}/.mangoclient-blocked.json"),
            b"{}".to_vec(),
        ),
    ];
    let refs: Vec<(&str, &[u8])> = entries
        .iter()
        .map(|(n, d)| (n.as_str(), d.as_slice()))
        .collect();
    write_zip(dir, name, &refs)
}

fn fabric_loader() -> Value {
    json!([{"uid": "net.fabricmc.fabric-loader", "version": "0.15.7"}])
}

/// Modrinth mock that serves `files` (name -> bytes) under `/files/` and the
/// hash lookup for them.
fn mr_files_server(files: Vec<(&'static str, Vec<u8>)>) -> impl FnOnce(&str) -> Handler {
    move |base| {
        let base = base.to_string();
        let by_sha1: HashMap<String, (&'static str, Vec<u8>)> = files
            .iter()
            .map(|(n, b)| (sha1_hex(b), (*n, b.clone())))
            .collect();
        Arc::new(move |r: &Req| {
            let path = r.path.split('?').next().unwrap();
            if let Some(name) = path.strip_prefix("/files/") {
                return match files.iter().find(|(n, _)| *n == name) {
                    Some((_, b)) => (200, b.clone()),
                    None => (404, Vec::new()),
                };
            }
            if path == "/version_files" && r.method == "POST" {
                let body: Value = serde_json::from_slice(&r.body).unwrap();
                let mut out = serde_json::Map::new();
                for h in body["hashes"].as_array().unwrap() {
                    let h = h.as_str().unwrap();
                    if let Some((name, bytes)) = by_sha1.get(h) {
                        out.insert(
                            h.to_string(),
                            json!({
                                "id": format!("ver-{name}"), "project_id": format!("proj-{name}"),
                                "name": name, "version_number": "1.0", "version_type": "release",
                                "date_published": "2024-01-01T00:00:00Z",
                                "game_versions": ["1.20.1"], "loaders": ["fabric"],
                                "files": [{
                                    "url": format!("{base}/files/{name}"), "filename": name,
                                    "primary": true, "size": bytes.len(),
                                    "hashes": {"sha1": h, "sha512": sha512_hex(bytes)}
                                }],
                                "dependencies": []
                            }),
                        );
                    }
                }
                return json_ok(Value::Object(out));
            }
            (404, b"{}".to_vec())
        })
    }
}

fn cf_manifest_json() -> Vec<u8> {
    serde_json::to_vec(&json!({
        "manifestType": "minecraftModpack", "manifestVersion": 1,
        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.16.9", "primary": true}]},
        "name": "CF Pack", "overrides": "overrides",
        "files": [
            {"projectID": 100, "fileID": 1001, "required": true},
            {"projectID": 500, "fileID": 5001, "required": true}
        ]
    }))
    .unwrap()
}

async fn cf_fx() -> Fx {
    let projects = vec![
        cf_project(100, "alpha", 6, Some(true)),
        cf_project(500, "secret", 6, Some(false)),
    ];
    let fab = ["1.21.1", "Fabric", "Client"];
    let files = vec![
        cf_file(1001, 100, "alpha-1.jar", &fab),
        cf_file(5001, 500, "secret-1.jar", &fab),
    ];
    let (base, _) = serve(cf_server(projects, files, |_, _| None)).await;
    fx_with(&dead(), &base, Some(KEY))
}

// ---------------------------------------------------------------- import: plans

#[tokio::test]
async fn plans_detect_the_kind_by_content_not_by_name() {
    let f = fx();
    let dir = f.scratch();

    // A Modrinth pack under a misleading name.
    let index = mrpack_index(
        "MR Pack",
        &[
            json!({"path": "mods/a.jar", "hashes": {"sha1": "x", "sha512": "y"},
                   "downloads": ["https://cdn.modrinth.com/a.jar"], "fileSize": 100}),
            json!({"path": "mods/b.jar", "hashes": {"sha1": "x", "sha512": "y"},
                   "downloads": ["https://cdn.modrinth.com/b.jar"], "fileSize": 50,
                   "env": {"client": "unsupported", "server": "required"}}),
        ],
    );
    let p = write_zip(
        &dir,
        "pack.bin",
        &[("modrinth.index.json", &index), ("overrides/o.txt", b"1")],
    );
    let plan = f
        .svc
        .instance_import_plan(p.display().to_string())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&plan).unwrap(),
        json!({"kind": "mrpack", "name": "MR Pack", "mcVersion": "1.21.1", "loader": "fabric",
               "loaderVersion": "0.16.9", "fileCount": 1, "sizeBytes": 100, "blockedFiles": []})
    );

    // A Prism instance inside a top-level folder, named .mrpack.
    let p = prism_zip(
        &dir,
        "inst.mrpack",
        "Cool Instance/",
        ".minecraft",
        fabric_loader(),
    );
    let plan = f
        .svc
        .instance_import_plan(p.display().to_string())
        .await
        .unwrap();
    assert_eq!(
        (
            plan.kind,
            plan.name.as_str(),
            plan.mc_version.as_str(),
            plan.loader,
            plan.loader_version.as_deref()
        ),
        (
            ImportKind::Prism,
            "Cool Pack",
            "1.20.1",
            Loader::Fabric,
            Some("0.15.7")
        )
    );
    // mods/a.jar, options.txt, config/x.toml: logs and our bookkeeping are skipped.
    assert_eq!(plan.file_count, 3);
    assert_eq!(plan.size_bytes, Some(5 + 10 + 3));

    // Without a top-level folder and with `minecraft/`; Forge with the game version in front.
    let forge = json!([{"uid": "net.minecraftforge", "version": "1.20.1-47.2.0"}]);
    let p = prism_zip(&dir, "inst.zip", "", "minecraft", forge);
    let plan = f
        .svc
        .instance_import_plan(p.display().to_string())
        .await
        .unwrap();
    assert_eq!(
        (plan.loader, plan.loader_version.as_deref(), plan.file_count),
        (Loader::Forge, Some("47.2.0"), 3)
    );

    // Quilt wins over the Fabric component it sits next to; plain vanilla has no loader.
    let quilt = json!([
        {"uid": "net.fabricmc.fabric-loader", "version": "0.15.7"},
        {"uid": "org.quiltmc.quilt-loader", "version": "0.26.0"}
    ]);
    let p = prism_zip(&dir, "q.zip", "", ".minecraft", quilt);
    let plan = f
        .svc
        .instance_import_plan(p.display().to_string())
        .await
        .unwrap();
    assert_eq!(
        (plan.loader, plan.loader_version.as_deref()),
        (Loader::Quilt, Some("0.26.0"))
    );
    let p = prism_zip(&dir, "v.zip", "", ".minecraft", json!([]));
    let plan = f
        .svc
        .instance_import_plan(p.display().to_string())
        .await
        .unwrap();
    assert_eq!((plan.loader, plan.loader_version), (Loader::Vanilla, None));

    // A CurseForge pack under a .zip name needs the API: blocked files come back.
    let cf = cf_fx().await;
    let cf_before = cf.profile_ids();
    let dir = cf.scratch();
    let p = write_zip(
        &dir,
        "x.zip",
        &[
            ("manifest.json", &cf_manifest_json()),
            ("overrides/config/x.toml", b"x"),
        ],
    );
    let plan = cf
        .svc
        .instance_import_plan(p.display().to_string())
        .await
        .unwrap();
    assert_eq!(
        (
            plan.kind,
            plan.name.as_str(),
            plan.loader,
            plan.loader_version.as_deref(),
            plan.file_count
        ),
        (
            ImportKind::Curseforge,
            "CF Pack",
            Loader::Fabric,
            Some("0.16.9"),
            2
        )
    );
    assert_eq!(plan.blocked_files.len(), 1);
    let b = &plan.blocked_files[0];
    assert_eq!(
        (b.file_name.as_str(), b.state),
        ("secret-1.jar", BlockedState::Pending)
    );
    // Planning changes nothing.
    assert_eq!(cf.profile_ids(), cf_before);
    // Without a CurseForge key the plan says so with a stable code.
    let keyless = fx();
    let p2 = write_zip(
        &keyless.scratch(),
        "x.zip",
        &[("manifest.json", &cf_manifest_json())],
    );
    assert_eq!(
        keyless
            .svc
            .instance_import_plan(p2.display().to_string())
            .await
            .unwrap_err()
            .code,
        "curseforge.no_api_key"
    );
}

#[tokio::test]
async fn plans_reject_bad_files_with_stable_codes() {
    let f = fx();
    let before = f.profile_ids();
    let dir = f.scratch();
    let code = |p: &Path| {
        let svc = f.svc.clone();
        let p = p.display().to_string();
        async move { svc.instance_import_plan(p).await.unwrap_err().code }
    };

    let junk = dir.join("junk.mrpack");
    std::fs::write(&junk, b"this is not a zip").unwrap();
    assert_eq!(code(&junk).await, "import.not_an_archive");

    let nothing = write_zip(&dir, "n.zip", &[("readme.txt", b"hi")]);
    assert_eq!(code(&nothing).await, "import.unsupported_format");

    // manifest.json that is not a CurseForge pack, and a broken one.
    let other = write_zip(
        &dir,
        "o.zip",
        &[("manifest.json", br#"{"manifestType": "other"}"#)],
    );
    assert_eq!(code(&other).await, "import.unsupported_format");
    let broken = write_zip(&dir, "b.zip", &[("manifest.json", b"{nope")]);
    assert_eq!(code(&broken).await, "import.unsupported_format");

    // Prism with a broken or incomplete mmc-pack.json.
    let bad_json = write_zip(
        &dir,
        "p1.zip",
        &[("instance.cfg", b"name=x"), ("mmc-pack.json", b"{")],
    );
    assert_eq!(code(&bad_json).await, "import.invalid_pack");
    let no_mc = write_zip(
        &dir,
        "p2.zip",
        &[
            ("instance.cfg", b"name=x"),
            ("mmc-pack.json", br#"{"components": []}"#),
        ],
    );
    assert_eq!(code(&no_mc).await, "import.invalid_pack");
    let unsafe_v = write_zip(
        &dir,
        "p3.zip",
        &[
            ("instance.cfg", b"name=x"),
            (
                "mmc-pack.json",
                br#"{"components": [{"uid": "net.minecraft", "version": "../../x"}]}"#,
            ),
        ],
    );
    assert_eq!(code(&unsafe_v).await, "import.invalid_pack");
    // A modrinth.index.json that is not JSON.
    let bad_index = write_zip(&dir, "m.zip", &[("modrinth.index.json", b"nope")]);
    assert!(f
        .svc
        .instance_import_plan(bad_index.display().to_string())
        .await
        .is_err());

    // Paths: relative, missing, folder, link, too large.
    assert_eq!(
        f.svc
            .instance_import_plan("pack.mrpack".into())
            .await
            .unwrap_err()
            .code,
        "import.not_a_file"
    );
    assert_eq!(
        f.svc
            .instance_import_plan(String::new())
            .await
            .unwrap_err()
            .code,
        "import.not_a_file"
    );
    assert_eq!(code(&dir.join("missing.zip")).await, "import.not_found");
    assert_eq!(code(&dir).await, "import.not_a_file");
    let link = dir.join("link.zip");
    std::os::unix::fs::symlink(&nothing, &link).unwrap();
    assert_eq!(
        code(&link).await,
        "import.not_a_file",
        "links are never followed"
    );
    let big = dir.join("big.zip");
    std::fs::File::create(&big)
        .unwrap()
        .set_len(3 * 1024 * 1024 * 1024)
        .unwrap();
    assert_eq!(code(&big).await, "import.too_large");
    assert_eq!(f.profile_ids(), before);
}

#[tokio::test]
async fn hostile_archives_are_rejected_before_anything_is_written() {
    let f = fx();
    let dir = f.scratch();
    let before = f.profile_ids();

    // Traversal in a Modrinth pack's overrides: refused at plan time and on import.
    let index = mrpack_index("Evil", &[]);
    let evil = write_zip(
        &dir,
        "evil.mrpack",
        &[
            ("modrinth.index.json", &index),
            ("overrides/../../escape.txt", b"no"),
        ],
    );
    assert!(f
        .svc
        .instance_import_plan(evil.display().to_string())
        .await
        .is_err());
    assert!(f
        .svc
        .instance_import(evil.display().to_string(), None)
        .await
        .is_err());

    // Traversal in a Prism instance.
    let pack = mmc_pack(json!([]));
    let evil_prism = write_zip(
        &dir,
        "evil-prism.zip",
        &[
            ("instance.cfg", b"name=x"),
            ("mmc-pack.json", &pack),
            (".minecraft/../../escape.txt", b"no"),
        ],
    );
    assert_eq!(
        f.svc
            .instance_import_plan(evil_prism.display().to_string())
            .await
            .unwrap_err()
            .code,
        "import.unsafe_archive"
    );
    assert_eq!(
        f.svc
            .instance_import(evil_prism.display().to_string(), None)
            .await
            .unwrap_err()
            .code,
        "import.unsafe_archive"
    );

    // A symlink entry.
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let o = zip::write::SimpleFileOptions::default();
    w.start_file("instance.cfg", o).unwrap();
    w.write_all(b"name=x").unwrap();
    w.start_file("mmc-pack.json", o).unwrap();
    w.write_all(&pack).unwrap();
    w.add_symlink(".minecraft/mods", "/etc", o).unwrap();
    let linky = dir.join("linky.zip");
    std::fs::write(&linky, w.finish().unwrap().into_inner()).unwrap();
    assert_eq!(
        f.svc
            .instance_import_plan(linky.display().to_string())
            .await
            .unwrap_err()
            .code,
        "import.unsafe_archive"
    );

    // CurseForge overrides with traversal (checked without any API call).
    let cf = cf_fx().await;
    let evil_cf = write_zip(
        &cf.scratch(),
        "evil-cf.zip",
        &[
            ("manifest.json", &cf_manifest_json()),
            ("overrides/../../escape.txt", b"no"),
        ],
    );
    assert_eq!(
        cf.svc
            .instance_import_plan(evil_cf.display().to_string())
            .await
            .unwrap_err()
            .code,
        "import.unsafe_archive"
    );
    assert_eq!(f.profile_ids(), before, "no profile was created");
    assert!(!dir.parent().unwrap().join("escape.txt").exists());
    assert!(!f.paths().instances().join("escape.txt").exists());
}

// ---------------------------------------------------------------- import

#[tokio::test]
async fn importing_a_prism_instance_creates_the_profile_and_its_files() {
    let f = fx();
    let dir = f.scratch();
    let p = prism_zip(
        &dir,
        "inst.zip",
        "Cool Instance/",
        ".minecraft",
        fabric_loader(),
    );
    let view = f
        .svc
        .instance_import(p.display().to_string(), Some("  My Import ".into()))
        .await
        .unwrap();
    assert_eq!(
        (
            view.name.as_str(),
            view.mc_version.as_str(),
            view.loader,
            view.loader_version.as_deref(),
            view.ram
        ),
        (
            "My Import",
            "1.20.1",
            Loader::Fabric,
            Some("0.15.7"),
            Some(6144)
        )
    );
    let inst = f.inst(&view.id);
    assert_eq!(std::fs::read(inst.join("mods/a.jar")).unwrap(), b"jar-a");
    assert_eq!(
        std::fs::read(inst.join("options.txt")).unwrap(),
        b"lang:de_de"
    );
    assert_eq!(std::fs::read(inst.join("config/x.toml")).unwrap(), b"x=1");
    assert!(!inst.join("logs").exists(), "logs are not imported");
    assert!(!inst.join(".mangoclient-blocked.json").exists());
    assert_eq!(view.mod_count, 1, "the pack's jars are ordinary records");
    // icon.png of the archive is the instance icon.
    assert!(view.cover_url.is_some());
    assert_eq!(view.icon_source.as_deref(), Some("modpack"));
    // Progress goes out for the new profile and ends complete.
    let ev = f.install_events(&view.id);
    assert_eq!(ev.last(), Some(&(1, 1)));
    // The name comes from the file when the user gives none.
    let again = f
        .svc
        .instance_import(p.display().to_string(), None)
        .await
        .unwrap();
    assert_eq!(again.name, "Cool Pack");
    assert!(p.exists(), "the user's file is never deleted");
}

#[tokio::test]
async fn importing_a_modrinth_pack_downloads_checks_and_cleans_up_after_failure() {
    let jar = b"mr:a.jar".to_vec();
    let (base, seen) = serve(mr_files_server(vec![("a.jar", jar.clone())])).await;
    let f = fx_with(&base, &dead(), None);
    let dir = f.scratch();
    let file = |hash: &str| {
        json!({"path": "mods/a.jar", "hashes": {"sha512": hash},
               "downloads": [format!("{base}/files/a.jar")], "fileSize": jar.len()})
    };
    let png = png_bytes();
    let good = write_zip(
        &dir,
        "good.mrpack",
        &[
            (
                "modrinth.index.json",
                &mrpack_index("Imported MR", &[file(&sha512_hex(&jar))]),
            ),
            ("overrides/options.txt", b"lang:en_us"),
            ("overrides/.mangoclient-blocked.json", b"{\"files\": []}"),
            ("icon.png", &png),
        ],
    );
    let view = f
        .svc
        .instance_import(good.display().to_string(), None)
        .await
        .unwrap();
    assert_eq!(
        (
            view.name.as_str(),
            view.mc_version.as_str(),
            view.loader,
            view.loader_version.as_deref()
        ),
        ("Imported MR", "1.21.1", Loader::Fabric, Some("0.16.9"))
    );
    let inst = f.inst(&view.id);
    assert_eq!(std::fs::read(inst.join("mods/a.jar")).unwrap(), jar);
    assert_eq!(
        std::fs::read(inst.join("options.txt")).unwrap(),
        b"lang:en_us"
    );
    assert!(
        !inst.join(".mangoclient-blocked.json").exists(),
        "bookkeeping is never taken from a pack"
    );
    assert_eq!(view.mod_count, 1);
    assert!(view.cover_url.is_some());
    assert_eq!(f.install_events(&view.id).last(), Some(&(1, 1)));
    assert!(good.exists());

    // A wrong hash fails the import and leaves no profile or half-written folder.
    let bad = write_zip(
        &dir,
        "bad.mrpack",
        &[(
            "modrinth.index.json",
            &mrpack_index("Bad", &[file(&sha512_hex(b"other"))]),
        )],
    );
    let before = f.profile_ids();
    assert!(f
        .svc
        .instance_import(bad.display().to_string(), None)
        .await
        .is_err());
    assert_eq!(f.profile_ids(), before);
    assert_eq!(
        std::fs::read_dir(f.paths().instances()).unwrap().count(),
        before.len()
    );
    assert!(bad.exists());
    assert!(seen
        .lock()
        .unwrap()
        .iter()
        .any(|r| r.path == "/files/a.jar"));
}

#[tokio::test]
async fn importing_a_curseforge_pack_keeps_blocked_files_for_the_user() {
    let f = cf_fx().await;
    let dir = f.scratch();
    let p = write_zip(
        &dir,
        "cf.zip",
        &[
            ("manifest.json", &cf_manifest_json()),
            ("overrides/config/x.toml", b"x = 1"),
            ("overrides/.mangoclient-blocked.json", b"{\"files\": []}"),
        ],
    );
    let view = f
        .svc
        .instance_import(p.display().to_string(), None)
        .await
        .unwrap();
    assert_eq!(
        (
            view.name.as_str(),
            view.mc_version.as_str(),
            view.loader,
            view.loader_version.as_deref()
        ),
        ("CF Pack", "1.21.1", Loader::Fabric, Some("0.16.9"))
    );
    let inst = f.inst(&view.id);
    assert_eq!(
        std::fs::read(inst.join("mods/alpha-1.jar")).unwrap(),
        crate::tests_p4::cf_bytes("alpha-1.jar")
    );
    assert_eq!(std::fs::read(inst.join("config/x.toml")).unwrap(), b"x = 1");
    assert!(!inst.join("mods/secret-1.jar").exists());
    // Persisted like modpack_install does: the sidecar lists the blocked file.
    let blocked = f.svc.blocked_files(view.id.clone()).unwrap();
    assert_eq!(blocked.len(), 1);
    assert_eq!(blocked[0].file_name, "secret-1.jar");
    assert_eq!(blocked[0].state, BlockedState::Pending);
    assert_eq!(
        f.svc
            .app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|p| p.id == view.id)
            .unwrap()
            .pending_blocked,
        1
    );
    assert!(f.sink.warnings().iter().any(|w| w.contains("secret-1.jar")));
    assert!(p.exists(), "the user's file is kept");
}

// ---------------------------------------------------------------- export

/// A fabric instance with every kind of file the export policy treats
/// differently, plus a Modrinth mock that hosts two of the jars.
struct ExportWorld {
    f: Fx,
    id: String,
    base: String,
}

async fn export_world() -> ExportWorld {
    let (base, _) = serve(mr_files_server(vec![
        ("matched.jar", b"matched-bytes".to_vec()),
        ("cfmatched.jar", b"cfmatched-bytes".to_vec()),
    ]))
    .await;
    let f = fx_with(&base, &dead(), None);
    let id = f.profile("My Pack", "1.20.1", "fabric", Some("0.16.9"));
    f.put(&id, "mods/matched.jar", b"matched-bytes");
    f.put(&id, "mods/cfmatched.jar", b"cfmatched-bytes");
    f.put(&id, "mods/mine.jar", b"my-own-jar");
    f.put(&id, "mods/cf-only.jar", b"cf-only-bytes");
    f.put(&id, "mods/manual-cf.jar", b"hand-downloaded");
    f.put(&id, "mods/MangoConfig-1.0.0+mc1.20.1.jar", b"mango");
    f.put(&id, "mods/off.jar.disabled", b"disabled-local");
    f.put(&id, "resourcepacks/pack.zip", b"rp");
    f.put(&id, "config/x.toml", b"a = 1");
    f.put(&id, "config/sub/y.json", b"{}");
    f.put(&id, "config/mangoconfig.json", b"{\"secret\": 1}");
    f.put(
        &id,
        "options.txt",
        b"lang:de_de\nlastServer:play.example.com\nfov:70\n",
    );
    f.put(&id, "servers.dat", b"servers");
    f.put(&id, "saves/World/level.dat", b"world");
    f.put(&id, "logs/latest.log", b"log");
    f.put(&id, "crash-reports/crash.txt", b"crash");
    f.put(&id, "screenshots/s.png", b"png");
    f.put(&id, ".mangoclient-blocked.json", b"{}");
    // Reconcile, then mark the CurseForge-sourced records.
    f.svc.content_list(id.clone()).await.unwrap();
    let mut mods = f.svc.profile(&id).unwrap().mods;
    for m in &mut mods {
        if matches!(m.filename.as_deref(), Some("cf-only.jar" | "cfmatched.jar")) {
            m.local = None;
            m.provider = Some("curseforge".into());
            m.project_id = Some("100".into());
        }
    }
    f.svc.save_mods(&id, &mods).unwrap();
    blocked::save(
        &f.paths(),
        &id,
        &[BlockedFile {
            name: "Manual".into(),
            file_name: "manual-cf.jar".into(),
            url: "https://www.curseforge.com/minecraft/mc-mods/manual".into(),
            sha1: None,
            size: None,
            kind: ContentType::Mod,
            state: BlockedState::Installed,
            modrinth: None,
            found_path: None,
        }],
    )
    .unwrap();
    ExportWorld { f, id, base }
}

fn export_args(w: &ExportWorld, config: bool, out: &Path) -> (String, ExportFormat, bool, String) {
    (
        w.id.clone(),
        ExportFormat::Mrpack,
        config,
        out.display().to_string(),
    )
}

#[tokio::test]
async fn export_policy_modrinth_files_by_reference_local_files_as_overrides() {
    let w = export_world().await;
    let out = w.f.scratch().join("pack.mrpack");
    let (id, fmt, cfg, outp) = export_args(&w, false, &out);
    let res = w.f.svc.instance_export(id, fmt, cfg, outp).await.unwrap();
    assert_eq!(res.path, out.display().to_string());
    assert_eq!(res.size_bytes, std::fs::metadata(&out).unwrap().len());
    no_part_files(&w.f.scratch());

    let index: Value = serde_json::from_str(&zip_text(&out, "modrinth.index.json")).unwrap();
    assert_eq!(index["formatVersion"], 1);
    assert_eq!(index["game"], "minecraft");
    assert_eq!(index["name"], "My Pack");
    assert_eq!(
        index["dependencies"],
        json!({"minecraft": "1.20.1", "fabric-loader": "0.16.9"})
    );
    let files = index["files"].as_array().unwrap();
    let paths: Vec<&str> = files.iter().map(|f| f["path"].as_str().unwrap()).collect();
    assert_eq!(
        paths,
        ["mods/cfmatched.jar", "mods/matched.jar"],
        "hosted on Modrinth, sorted"
    );
    let m = &files[1];
    assert_eq!(m["hashes"]["sha1"], sha1_hex(b"matched-bytes"));
    assert_eq!(m["hashes"]["sha512"], sha512_hex(b"matched-bytes"));
    assert_eq!(m["fileSize"], b"matched-bytes".len());
    assert_eq!(
        m["downloads"],
        json!([format!("{}/files/matched.jar", w.base)])
    );
    assert_eq!(
        m["env"],
        json!({"client": "required", "server": "required"})
    );

    // Local content that Modrinth does not host travels as overrides; nothing
    // from CurseForge, no launcher or private data, no config without the flag.
    assert_eq!(
        zip_names(&out),
        [
            "modrinth.index.json",
            "overrides/mods/mine.jar",
            "overrides/mods/off.jar.disabled",
            "overrides/resourcepacks/pack.zip",
        ]
    );
    // The warning names the CurseForge files that were left out.
    let w1 = res.warnings.join("\n");
    assert!(w1.contains("cf only"), "{w1}");
    assert!(w1.contains("CurseForge"));
    assert!(
        w1.contains("manual cf"),
        "a hand-fetched blocked file is left out too: {w1}"
    );
    assert_eq!(res.warnings.len(), 2, "{:?}", res.warnings);
    assert!(w.f.sink.warnings().iter().any(|l| l.contains("CurseForge")));
}

#[tokio::test]
async fn export_with_config_adds_settings_but_never_private_data() {
    let w = export_world().await;
    let out = w.f.scratch().join("cfg.mrpack");
    let (id, fmt, cfg, outp) = export_args(&w, true, &out);
    w.f.svc.instance_export(id, fmt, cfg, outp).await.unwrap();
    let names = zip_names(&out);
    for want in [
        "overrides/config/x.toml",
        "overrides/config/sub/y.json",
        "overrides/options.txt",
        "overrides/mods/mine.jar",
    ] {
        assert!(names.contains(&want.to_string()), "{want} in {names:?}");
    }
    for never in [
        "servers.dat",
        "saves",
        "logs",
        "crash-reports",
        "screenshots",
        ".mangoclient",
        "MangoConfig",
        "mangoconfig",
        "accounts",
        "cf-only",
        "manual-cf",
    ] {
        assert!(
            !names.iter().any(|n| n.contains(never)),
            "{never} must not be exported: {names:?}"
        );
    }
    // The address of the last server is not shared.
    let options = zip_text(&out, "overrides/options.txt");
    assert!(
        options.contains("lang:de_de") && options.contains("fov:70"),
        "{options}"
    );
    assert!(!options.contains("lastServer"), "{options}");
}

#[tokio::test]
async fn export_validates_the_target_and_writes_atomically() {
    let w = export_world().await;
    let dir = w.f.scratch();
    let svc = &w.f.svc;
    let run = |out: String| {
        let (svc, id) = (svc.clone(), w.id.clone());
        async move {
            svc.instance_export(id, ExportFormat::Mrpack, false, out)
                .await
        }
    };
    for bad in [
        "pack.mrpack".to_string(),
        String::new(),
        dir.join("no-such-folder/pack.mrpack").display().to_string(),
        dir.display().to_string(),
        w.f.inst(&w.id).join("inside.mrpack").display().to_string(),
        w.f.inst(&w.id)
            .join("mods/inside.mrpack")
            .display()
            .to_string(),
        "/".to_string(),
        format!("{}/a\0b", dir.display()),
    ] {
        assert_eq!(
            run(bad.clone()).await.unwrap_err().code,
            "export.invalid_path",
            "{bad:?}"
        );
    }
    assert!(svc
        .instance_export(
            "not-a-uuid".into(),
            ExportFormat::Mrpack,
            false,
            dir.join("x").display().to_string()
        )
        .await
        .is_err());
    assert!(!w.f.inst(&w.id).join("inside.mrpack").exists());

    // An existing file is replaced, whole or not at all.
    let out = dir.join("keep.mrpack");
    std::fs::write(&out, b"precious").unwrap();
    run(out.display().to_string()).await.unwrap();
    assert!(zip_names(&out).contains(&"modrinth.index.json".to_string()));
    // Modrinth unreachable while a Modrinth record needs a download link: the
    // old file stays untouched and nothing else is left behind.
    let dead_world = {
        let f = fx();
        let id = f.profile("P", "1.20.1", "fabric", Some("0.16.9"));
        f.put(&id, "mods/a.jar", b"a");
        f.svc.content_list(id.clone()).await.unwrap();
        let mut mods = f.svc.profile(&id).unwrap().mods;
        mods[0].local = None;
        mods[0].project_id = Some("AABBCCDD".into());
        mods[0].version_id = Some("v1".into());
        f.svc.save_mods(&id, &mods).unwrap();
        (f, id)
    };
    let keep = dead_world.0.scratch().join("keep.mrpack");
    std::fs::write(&keep, b"precious").unwrap();
    let e = dead_world
        .0
        .svc
        .instance_export(
            dead_world.1.clone(),
            ExportFormat::Mrpack,
            false,
            keep.display().to_string(),
        )
        .await
        .unwrap_err();
    assert_eq!(e.code, "export.lookup_failed");
    assert_eq!(std::fs::read(&keep).unwrap(), b"precious");
    no_part_files(&dead_world.0.scratch());

    // Only local files and no network: they are included directly, with a notice.
    let f = fx();
    let id = f.profile("L", "1.20.1", "vanilla", None);
    f.put(&id, "mods/a.jar", b"a");
    let out = f.scratch().join("local.mrpack");
    let res = f
        .svc
        .instance_export(id, ExportFormat::Mrpack, false, out.display().to_string())
        .await
        .unwrap();
    assert!(zip_names(&out).contains(&"overrides/mods/a.jar".to_string()));
    assert!(res
        .warnings
        .iter()
        .any(|w| w.contains("Modrinth could not be reached")));
    let index: Value = serde_json::from_str(&zip_text(&out, "modrinth.index.json")).unwrap();
    assert_eq!(
        index["dependencies"],
        json!({"minecraft": "1.20.1"}),
        "vanilla has no loader entry"
    );
}

#[tokio::test]
async fn export_needs_a_loader_version() {
    let f = fx();
    let id = f.profile("NoVersion", "1.20.1", "fabric", None);
    let out = f.scratch().join("x.mrpack");
    let e = f
        .svc
        .instance_export(id, ExportFormat::Mrpack, false, out.display().to_string())
        .await
        .unwrap_err();
    assert_eq!(e.code, "export.loader_unresolved");
    assert!(!out.exists());
}

#[tokio::test]
async fn export_then_import_gives_the_same_instance() {
    let w = export_world().await;
    // Give it an image icon so it travels too.
    let icon = w.f.scratch().join("icon.png");
    std::fs::write(&icon, png_bytes()).unwrap();
    w.f.svc
        .profile_icon_set_image(w.id.clone(), icon.display().to_string())
        .await
        .unwrap();
    let out = w.f.scratch().join("round.mrpack");
    let (id, fmt, cfg, outp) = export_args(&w, true, &out);
    w.f.svc.instance_export(id, fmt, cfg, outp).await.unwrap();

    let plan =
        w.f.svc
            .instance_import_plan(out.display().to_string())
            .await
            .unwrap();
    assert_eq!(
        (
            plan.kind,
            plan.name.as_str(),
            plan.mc_version.as_str(),
            plan.loader,
            plan.loader_version.as_deref()
        ),
        (
            ImportKind::Mrpack,
            "My Pack",
            "1.20.1",
            Loader::Fabric,
            Some("0.16.9")
        )
    );
    assert_eq!(plan.file_count, 2);
    assert!(plan.blocked_files.is_empty());

    let view =
        w.f.svc
            .instance_import(out.display().to_string(), Some("Copy".into()))
            .await
            .unwrap();
    assert_eq!(
        (
            view.mc_version.as_str(),
            view.loader,
            view.loader_version.as_deref()
        ),
        ("1.20.1", Loader::Fabric, Some("0.16.9"))
    );
    let inst = w.f.inst(&view.id);
    // Hosted jars come back through their hashes, the rest from overrides.
    assert_eq!(
        std::fs::read(inst.join("mods/matched.jar")).unwrap(),
        b"matched-bytes"
    );
    assert_eq!(
        std::fs::read(inst.join("mods/cfmatched.jar")).unwrap(),
        b"cfmatched-bytes"
    );
    assert_eq!(
        std::fs::read(inst.join("mods/mine.jar")).unwrap(),
        b"my-own-jar"
    );
    assert_eq!(std::fs::read(inst.join("config/x.toml")).unwrap(), b"a = 1");
    assert!(!inst.join("mods/cf-only.jar").exists());
    assert!(!inst.join("servers.dat").exists());
    assert!(view.cover_url.is_some(), "the icon travels");
    // And a second export of the copy plans identically (stable round trip).
    let out2 = w.f.scratch().join("round2.mrpack");
    w.f.svc
        .instance_export(
            view.id.clone(),
            ExportFormat::Mrpack,
            true,
            out2.display().to_string(),
        )
        .await
        .unwrap();
    let plan2 =
        w.f.svc
            .instance_import_plan(out2.display().to_string())
            .await
            .unwrap();
    assert_eq!(
        (
            plan2.mc_version,
            plan2.loader,
            plan2.loader_version,
            plan2.file_count
        ),
        (
            plan.mc_version,
            plan.loader,
            plan.loader_version,
            plan.file_count
        )
    );
}

// ---------------------------------------------------------------- crash analysis (service)

fn write_log(f: &Fx, id: &str, rel: &str, text: &str, age_secs: u64) {
    let p = f.put(id, rel, text.as_bytes());
    let t = std::time::SystemTime::now() - std::time::Duration::from_secs(age_secs);
    std::fs::File::options()
        .write(true)
        .open(&p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

const OOM_REPORT: &str = "---- Minecraft Crash Report ----\n// Don't do that.\n\nTime: 2025-01-02 10:00:00\nDescription: Exception in server tick loop\n\njava.lang.OutOfMemoryError: Java heap space\n\tat net.minecraft.class_2818.method_12202(SourceFile:1)\n\nA detailed walkthrough of the error\n\n-- System Details --\n\tMinecraft Version: 1.20.1\n\tJava Version: 17.0.9, Eclipse Adoptium\n";

#[tokio::test]
async fn crash_analyze_picks_the_newest_crash_report_else_latest_log() {
    let f = fx();
    let id = f.profile("C", "1.20.1", "fabric", Some("0.16.9"));
    f.svc
        .profile_update(id.clone(), json!({"ram": 4096}))
        .unwrap();

    // Nothing yet.
    let r = f.svc.crash_analyze(id.clone(), None).await.unwrap();
    assert_eq!(
        (r.source_log, r.causes.len(), r.excerpt.as_str()),
        (None, 0, "")
    );
    assert_eq!(r.summary, "There is no log to analyze yet");
    assert_eq!(r.mc_version.as_deref(), Some("1.20.1"));

    // Only latest.log.
    write_log(
        &f,
        &id,
        "logs/latest.log",
        "[main/INFO]: hi\njava.io.IOException: No space left on device\n",
        5,
    );
    let r = f.svc.crash_analyze(id.clone(), None).await.unwrap();
    assert_eq!(r.source_log.as_deref(), Some("game:latest.log"));
    assert_eq!(r.causes[0].code, "disk_full");

    // Two crash reports: the newest wins over the log.
    write_log(
        &f,
        &id,
        "crash-reports/crash-old.txt",
        "java.io.IOException: No space left on device",
        500,
    );
    write_log(&f, &id, "crash-reports/crash-new.txt", OOM_REPORT, 100);
    let r = f.svc.crash_analyze(id.clone(), None).await.unwrap();
    assert_eq!(r.source_log.as_deref(), Some("crash:crash-new.txt"));
    assert_eq!(r.causes[0].code, "out_of_memory");
    assert!(
        r.causes[0].suggestions[0].contains("4096 MB"),
        "{:?}",
        r.causes[0].suggestions
    );
    assert_eq!(r.java_version.as_deref(), Some("17.0.9"));
    assert_eq!(r.mc_version.as_deref(), Some("1.20.1"));
    assert!(r.excerpt.lines().count() <= 40 && r.excerpt.contains("OutOfMemoryError"));
    assert_eq!(r.summary, "The game ran out of memory");
    let shape = serde_json::to_value(&r).unwrap();
    for key in [
        "summary",
        "causes",
        "javaVersion",
        "mcVersion",
        "excerpt",
        "sourceLog",
    ] {
        assert!(shape.get(key).is_some(), "{key}");
    }
    let cause = &shape["causes"][0];
    for key in [
        "code",
        "title",
        "detail",
        "confidence",
        "suggestions",
        "relatedMods",
    ] {
        assert!(cause.get(key).is_some(), "{key}");
    }
    assert_eq!(cause["confidence"], "high");

    // An explicit id, including an older crash report and bad ids.
    let r = f
        .svc
        .crash_analyze(id.clone(), Some("crash:crash-old.txt".into()))
        .await
        .unwrap();
    assert_eq!(r.causes[0].code, "disk_full");
    let r = f
        .svc
        .crash_analyze(id.clone(), Some("game:latest.log".into()))
        .await
        .unwrap();
    assert_eq!(r.source_log.as_deref(), Some("game:latest.log"));
    for bad in [
        "crash:../x.txt",
        "nope",
        "crash:missing.txt",
        "game:latest.log.exe",
    ] {
        let e = f
            .svc
            .crash_analyze(id.clone(), Some(bad.into()))
            .await
            .unwrap_err();
        assert!(
            ["unknown_log", "not_found"].contains(&e.code.as_str()),
            "{bad}: {}",
            e.code
        );
    }
    assert!(f.svc.crash_analyze("zzz".into(), None).await.is_err());
}

#[tokio::test]
async fn crash_analyze_reads_at_most_the_last_4_mb_and_gz_archives() {
    let f = fx();
    let id = f.profile("C", "1.20.1", "fabric", None);
    // The cause sits at the very end of a 5 MB log: found. At the start: cut off.
    let filler = "[main/INFO]: padding padding padding padding padding\n".repeat(100_000);
    assert!(filler.len() > 5_000_000);
    write_log(
        &f,
        &id,
        "logs/latest.log",
        &format!("java.io.IOException: No space left on device\n{filler}"),
        1,
    );
    let r = f.svc.crash_analyze(id.clone(), None).await.unwrap();
    assert!(r.causes.is_empty(), "the head of a long log is not read");
    write_log(
        &f,
        &id,
        "logs/latest.log",
        &format!("{filler}java.io.IOException: No space left on device\n"),
        1,
    );
    let r = f.svc.crash_analyze(id.clone(), None).await.unwrap();
    assert_eq!(r.causes[0].code, "disk_full");
    assert!(r.excerpt.lines().count() <= 40);

    // A rotated gzip log.
    let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
    gz.write_all(
        b"[main/ERROR]: GLFW error 65542: WGL: The driver does not appear to support OpenGL\n",
    )
    .unwrap();
    f.put(&id, "logs/2025-01-01-1.log.gz", &gz.finish().unwrap());
    let r = f
        .svc
        .crash_analyze(id, Some("game:2025-01-01-1.log.gz".into()))
        .await
        .unwrap();
    assert_eq!(r.causes[0].code, "gl_driver");
}

#[tokio::test]
async fn crash_analyze_maps_related_mods_and_scrubs_the_excerpt() {
    let f = fx();
    let id = f.profile("C", "1.20.1", "fabric", Some("0.16.9"));
    f.put(&id, "mods/sodium-fabric-0.5.8.jar", b"x");
    f.svc.content_list(id.clone()).await.unwrap();
    let mut mods = f.svc.profile(&id).unwrap().mods;
    mods[0].title = Some("Sodium".into());
    mods[0].slug = Some("sodium".into());
    f.svc.save_mods(&id, &mods).unwrap();
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/tester".into());
    let log = format!(
        "[main/INFO]: starting in {home}/.mangoclient\n\
         java --accessToken SUPERSECRETTOKEN1 --version 1.20.1\n\
         \t- Mod 'Iris' (iris) 1.7.0 requires version 0.5.8 or later of mod 'Sodium' (sodium), but only the wrong version is present: 0.5.3!\n"
    );
    write_log(&f, &id, "logs/latest.log", &log, 1);
    let r = f.svc.crash_analyze(id, None).await.unwrap();
    let cause = r
        .causes
        .iter()
        .find(|c| c.code == "mod_dependency")
        .unwrap();
    assert!(
        cause.related_mods.contains(&"Sodium".to_string()),
        "{:?}",
        cause.related_mods
    );
    assert!(cause.related_mods.contains(&"iris".to_string()));
    assert!(!r.excerpt.contains("SUPERSECRETTOKEN1"), "{}", r.excerpt);
    if home.len() > 1 {
        assert!(!r.excerpt.contains(&home), "{}", r.excerpt);
    }
}

// ---------------------------------------------------------------- diagnostics

#[tokio::test]
async fn diagnostics_zip_has_the_facts_and_no_secrets() {
    let f = fx();
    let id = f.profile("D", "1.20.1", "fabric", Some("0.16.9"));
    f.svc.account_add_offline("Notch_Fan".into()).unwrap();
    let home = std::env::var("HOME").unwrap_or_default();
    let latest = format!(
        "[main/INFO]: Setting user: Notch_Fan\n\
         [main/INFO]: Launching with java --accessToken eyJhbGciOiJSUzI1NiJ9.eyJ4dWlkIjoiMTIzNDU2Nzg5In0.c2lnbmF0dXJl --uuid 069a79f444e94726a5befca90e38aaf5 --gameDir {home}/.mangoclient/instances/x\n\
         [main/INFO]: Authorization: Bearer abcDEF1234567890\n"
    );
    write_log(&f, &id, "logs/latest.log", &latest, 5);
    write_log(&f, &id, "crash-reports/crash-2025.txt", OOM_REPORT, 50);
    f.put(&id, "mods/a.jar", b"a");
    f.svc.content_list(id.clone()).await.unwrap();
    let out = f.scratch().join("diag.zip");
    let res = f
        .svc
        .diagnostics_export(id.clone(), out.display().to_string())
        .await
        .unwrap();
    assert_eq!(res.size_bytes, std::fs::metadata(&out).unwrap().len());
    no_part_files(&f.scratch());
    assert_eq!(
        zip_names(&out),
        [
            "crash-reports/crash-2025.txt",
            "logs/latest.log",
            "mods.json",
            "profile.json",
            "system.json"
        ]
    );
    let all: String = zip_names(&out)
        .iter()
        .map(|n| zip_text(&out, n))
        .collect::<Vec<_>>()
        .join("\n");
    for secret in ["Notch_Fan", "eyJhbGci", "069a79f4", "abcDEF1234567890"] {
        assert!(!all.contains(secret), "{secret} leaked");
    }
    if home.len() > 1 {
        assert!(!all.contains(&home), "home directory leaked");
    }
    assert!(zip_text(&out, "logs/latest.log").contains("--accessToken <redacted>"));
    let profile: Value = serde_json::from_str(&zip_text(&out, "profile.json")).unwrap();
    assert_eq!(profile["name"], "D");
    assert_eq!(profile["loader"], "fabric");
    assert!(profile.get("accounts").is_none());
    let system: Value = serde_json::from_str(&zip_text(&out, "system.json")).unwrap();
    assert_eq!(
        system["launcher"]["version"],
        f.svc.app_state().unwrap().version
    );
    assert_eq!(system["os"], std::env::consts::OS);
    assert!(system["totalRamMb"].as_u64().unwrap() > 0);
    assert_eq!(system["java"]["requiredMajor"], 21);
    let mods: Value = serde_json::from_str(&zip_text(&out, "mods.json")).unwrap();
    assert_eq!(mods["count"], 1);
    assert_eq!(mods["mods"][0]["file"], "a.jar");
    assert_eq!(mods["mods"][0]["provider"], "local");
    // The account store's secrets never come near it.
    let state = f.svc.app_state().unwrap();
    assert_eq!(state.accounts.len(), 1);
}

#[tokio::test]
async fn diagnostics_caps_log_sizes_and_validates_the_target() {
    let f = fx();
    let id = f.profile("D", "1.20.1", "vanilla", None);
    let big = "[main/INFO]: some line of a very long log file here\n".repeat(120_000);
    assert!(big.len() > 5_000_000);
    write_log(&f, &id, "logs/latest.log", &big, 1);
    let out = f.scratch().join("d.zip");
    let res = f
        .svc
        .diagnostics_export(id.clone(), out.display().to_string())
        .await
        .unwrap();
    let log = zip_text(&out, "logs/latest.log");
    assert!(log.len() <= 2 * 1024 * 1024 + 4, "{}", log.len());
    assert!(res
        .warnings
        .iter()
        .any(|w| w.contains("logs/latest.log was cut")));
    // No logs at all is fine: the bundle is just smaller.
    let id2 = f.profile("E", "1.20.1", "vanilla", None);
    let out2 = f.scratch().join("e.zip");
    let res = f
        .svc
        .diagnostics_export(id2.clone(), out2.display().to_string())
        .await
        .unwrap();
    assert!(!zip_names(&out2).contains(&"logs/latest.log".to_string()));
    assert!(res.warnings.iter().any(|w| w.contains("does not exist")));
    for bad in [
        "d.zip".to_string(),
        f.scratch().display().to_string(),
        f.scratch().join("nope/d.zip").display().to_string(),
        f.inst(&id2).join("d.zip").display().to_string(),
    ] {
        assert_eq!(
            f.svc
                .diagnostics_export(id2.clone(), bad.clone())
                .await
                .unwrap_err()
                .code,
            "export.invalid_path",
            "{bad}"
        );
    }
    no_part_files(&f.scratch());
}

// ---------------------------------------------------------------- dispatch

#[tokio::test]
async fn the_new_commands_are_reachable_through_dispatch() {
    use crate::dispatch::{invoke, COMMANDS};
    for c in [
        "instance_export",
        "instance_import_plan",
        "instance_import",
        "crash_analyze",
        "diagnostics_export",
    ] {
        assert!(COMMANDS.contains(&c), "{c}");
    }
    let f = fx();
    let id = f.profile("Z", "1.20.1", "fabric", Some("0.16.9"));
    let dir = f.scratch();
    let out = dir.join("d.zip");
    let r = invoke(
        &f.svc,
        "diagnostics_export",
        json!({"profileId": id, "outPath": out.display().to_string()}),
    )
    .await
    .unwrap();
    assert_eq!(r["path"], out.display().to_string());
    assert!(r["sizeBytes"].as_u64().unwrap() > 0);
    assert!(r["warnings"].as_array().is_some());
    let r = invoke(&f.svc, "crash_analyze", json!({"profileId": id}))
        .await
        .unwrap();
    assert_eq!(r["sourceLog"], Value::Null);
    let pack = dir.join("p.mrpack");
    let r = invoke(
        &f.svc,
        "instance_export",
        json!({"profileId": id, "format": "mrpack", "includeConfig": false, "outPath": pack.display().to_string()}),
    )
    .await
    .unwrap();
    assert!(r["sizeBytes"].as_u64().unwrap() > 0);
    let plan = invoke(
        &f.svc,
        "instance_import_plan",
        json!({"filePath": pack.display().to_string()}),
    )
    .await
    .unwrap();
    assert_eq!(plan["kind"], "mrpack");
    assert_eq!(plan["name"], "Z");
    let view = invoke(
        &f.svc,
        "instance_import",
        json!({"filePath": pack.display().to_string(), "name": "Z2"}),
    )
    .await
    .unwrap();
    assert_eq!(view["name"], "Z2");
    assert_eq!(view["tags"], json!([]));
    // Bad arguments are `bad_args`, never a panic.
    for (cmd, args) in [
        (
            "instance_export",
            json!({"profileId": id, "format": "zip", "includeConfig": true, "outPath": "/x"}),
        ),
        ("instance_export", json!({"profileId": id})),
        ("instance_import_plan", json!({"path": "/x"})),
        ("crash_analyze", json!({"profileId": id, "extra": 1})),
    ] {
        assert_eq!(
            invoke(&f.svc, cmd, args).await.unwrap_err().code,
            "bad_args",
            "{cmd}"
        );
    }
}
