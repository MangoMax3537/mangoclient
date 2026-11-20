#![allow(clippy::unwrap_used)]
//! Part 5 of `docs/ipc.md`: blocked CurseForge files (Modrinth hash lookup,
//! persisted list, download-folder watcher). Mock servers and temp dirs only.

use crate::blocked::{is_temp_name, scan, Tick, WatchState};
use crate::tests_p2::{json_ok, serve, Req};
use crate::tests_p4::{cf_file, cf_project, cf_server, zip_bytes, CfFile, Sink, KEY};
use crate::*;
use mango_core::{Paths, Secret};
use serde_json::{json, Value};
use sha1::{Digest, Sha1};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const SECRET: &[u8] = b"SECRET-BYTES";
const MULTI: &[u8] = b"MULTI-LOADER-BYTES";
const NOHASH: &[u8] = b"NO-HASH-BYTES";
const SHADER: &[u8] = b"SHADER-BYTES";

fn sha1(b: &[u8]) -> String {
    hex::encode(Sha1::digest(b))
}

fn sha512(b: &[u8]) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha512::digest(b))
}

fn mr_version(
    base: &str,
    id: &str,
    project: &str,
    file: &str,
    bytes: &[u8],
    loaders: &[&str],
) -> Value {
    json!({
        "id": id, "project_id": project, "name": id, "version_number": "1.0",
        "version_type": "release", "date_published": "2024-01-01T00:00:00Z",
        "game_versions": ["1.21.1"], "loaders": loaders,
        "files": [{"url": format!("{base}/files/{file}"), "filename": file, "primary": true,
                   "size": bytes.len(), "hashes": {"sha1": sha1(bytes), "sha512": sha512(bytes)}}],
        "dependencies": []
    })
}

/// Modrinth knows SECRET (one loader) and MULTI (two loaders).
async fn mr_mock(files_broken: Arc<AtomicBool>) -> (String, Arc<Mutex<Vec<Req>>>) {
    serve(move |base| {
        let base = base.to_string();
        Arc::new(move |r: &Req| {
            let path = r.path.split('?').next().unwrap();
            if r.method == "POST" && path == "/version_files" {
                let body: Value = serde_json::from_slice(&r.body).unwrap();
                let mut out = serde_json::Map::new();
                for h in body["hashes"].as_array().unwrap() {
                    let h = h.as_str().unwrap();
                    if h == sha1(SECRET) {
                        out.insert(
                            h.into(),
                            mr_version(&base, "mrv-secret", "mr-secret", "secret-modrinth.jar", SECRET, &["fabric"]),
                        );
                    } else if h == sha1(MULTI) {
                        out.insert(
                            h.into(),
                            mr_version(&base, "mrv-multi", "mr-multi", "multi-modrinth.jar", MULTI, &["fabric", "forge"]),
                        );
                    }
                }
                return json_ok(Value::Object(out));
            }
            if path == "/projects" {
                return json_ok(json!([{
                    "id": "mr-secret", "slug": "secret-mod", "title": "Secret Mod", "project_type": "mod",
                    "icon_url": "https://cdn.modrinth.com/data/x/icon.png"
                }]));
            }
            if let Some(name) = path.strip_prefix("/files/") {
                if files_broken.load(Ordering::SeqCst) {
                    return (404, b"{}".to_vec());
                }
                return match name {
                    "secret-modrinth.jar" => (200, SECRET.to_vec()),
                    "multi-modrinth.jar" => (200, MULTI.to_vec()),
                    _ => (404, b"{}".to_vec()),
                };
            }
            (404, b"{}".to_vec())
        })
    })
    .await
}

fn with_body(mut f: CfFile, body: &[u8]) -> CfFile {
    f.body = Some(body.to_vec());
    f
}

fn manifest() -> Vec<u8> {
    let file = |p: u64, f: u64| json!({"projectID": p, "fileID": f, "required": true});
    serde_json::to_vec(&json!({
        "minecraft": {"version": "1.21.1", "modLoaders": [{"id": "fabric-0.16.9", "primary": true}]},
        "name": "The Pack", "overrides": "overrides",
        "files": [file(100, 1001), file(500, 5001), file(501, 5011), file(502, 5021), file(503, 5031), file(504, 5041)]
    }))
    .unwrap()
}

async fn cf_mock() -> (String, Arc<Mutex<Vec<Req>>>) {
    let fab = ["1.21.1", "Fabric", "Client"];
    let projects = vec![
        cf_project(100, "alpha", 6, Some(true)),
        cf_project(500, "secret", 6, Some(false)),
        cf_project(501, "pretty", 6552, Some(false)),
        cf_project(502, "multi", 6, Some(false)),
        cf_project(503, "nohash", 6, Some(false)),
        cf_project(504, "evil", 6, Some(false)),
        cf_project(1000, "the-pack", 4471, None),
    ];
    let mut nohash = with_body(cf_file(5031, 503, "nohash-1.jar", &fab), NOHASH);
    nohash.hash = false;
    let mut pack = cf_file(10001, 1000, "the-pack-1.zip", &["1.21.1", "Fabric"]);
    pack.body = Some(zip_bytes(&[
        ("manifest.json", &manifest()),
        ("overrides/config/x.toml", b"x = 1"),
    ]));
    let files = vec![
        cf_file(1001, 100, "alpha-1.jar", &fab),
        with_body(cf_file(5001, 500, "secret-1.jar", &fab), SECRET),
        with_body(
            cf_file(5011, 501, "pretty-blocked.zip", &["1.21.1"]),
            SHADER,
        ),
        with_body(cf_file(5021, 502, "multi-1.jar", &fab), MULTI),
        nohash,
        cf_file(5041, 504, "../evil.jar", &fab),
        pack,
    ];
    serve(cf_server(projects, files, |_, _| None)).await
}

struct P5 {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<Sink>,
    dl: PathBuf,
    mr_seen: Arc<Mutex<Vec<Req>>>,
    cf_seen: Arc<Mutex<Vec<Req>>>,
    files_broken: Arc<AtomicBool>,
}

fn opts(mr: &str, cf: &str, downloads: &Path) -> ServiceOptions {
    ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        partners: Vec::new(),
        icon_hosts: Vec::new(),
        modrinth_api: mr.to_string(),
        curseforge_api: cf.to_string(),
        curseforge_key: Some(Secret::new(KEY)),
        curseforge: mango_curseforge::Options {
            retries: 0,
            strict_download_urls: false,
            ..mango_curseforge::Options::default()
        },
        blocked_poll_interval: Duration::from_millis(25),
        downloads_dir: Some(downloads.to_path_buf()),
        ..ServiceOptions::default()
    }
}

async fn setup() -> P5 {
    let tmp = tempfile::tempdir().unwrap();
    let dl = tmp.path().join("dl");
    std::fs::create_dir_all(&dl).unwrap();
    let files_broken = Arc::new(AtomicBool::new(false));
    let (mr, mr_seen) = mr_mock(files_broken.clone()).await;
    let (cf, cf_seen) = cf_mock().await;
    let sink = Arc::new(Sink::default());
    let svc = AppService::new(
        Paths::new(tmp.path().join("data")),
        sink.clone(),
        opts(&mr, &cf, &dl),
    )
    .unwrap();
    svc.config_set(json!({"blockedModsWatchDirs": [dl.display().to_string()]}))
        .unwrap();
    P5 {
        tmp,
        svc,
        sink,
        dl,
        mr_seen,
        cf_seen,
        files_broken,
    }
}

impl P5 {
    async fn install(&self) -> ProfileView {
        self.svc
            .modpack_install(
                Provider::Curseforge,
                "1000".into(),
                None,
                Some("Pack".into()),
            )
            .await
            .unwrap()
    }
    fn pending(&self, id: &str) -> usize {
        self.svc
            .app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|x| x.id == id)
            .unwrap()
            .pending_blocked
    }
    fn instance(&self, id: &str) -> PathBuf {
        self.svc.paths().instance_dir(id).unwrap()
    }
    fn state(&self, id: &str, file: &str) -> BlockedState {
        self.svc
            .blocked_files(id.into())
            .unwrap()
            .into_iter()
            .find(|f| f.file_name == file)
            .unwrap()
            .state
    }
    fn posts(&self) -> Vec<Value> {
        self.mr_seen
            .lock()
            .unwrap()
            .iter()
            .filter(|r| r.method == "POST")
            .map(|r| serde_json::from_slice(&r.body).unwrap())
            .collect()
    }
    /// Skip everything except `keep`.
    fn skip_all_but(&self, id: &str, keep: &[&str]) {
        for f in self.svc.blocked_files(id.into()).unwrap() {
            if !keep.contains(&f.file_name.as_str()) && f.state == BlockedState::Pending {
                self.svc.blocked_skip(id.into(), f.file_name).unwrap();
            }
        }
    }
    async fn ticks(&self, id: &str, st: &mut WatchState, n: usize) -> bool {
        let mut done = false;
        for _ in 0..n {
            done = matches!(self.svc.watch_tick(id, st).await, Tick::Done);
        }
        done
    }
}

async fn wait_for(what: &str, mut cond: impl FnMut() -> bool) {
    for _ in 0..600 {
        if cond() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("timed out waiting for {what}");
}

fn names(files: &[BlockedFile]) -> Vec<&str> {
    files.iter().map(|f| f.file_name.as_str()).collect()
}

// ---------------------------------------------------------------- plan and install

#[tokio::test]
async fn the_plan_matches_on_modrinth_by_sha1_and_rejects_multi_loader_builds() {
    let f = setup().await;
    let plan = f
        .svc
        .modpack_plan(Provider::Curseforge, "1000".into(), None)
        .await
        .unwrap();
    let by = |n: &str| {
        plan.blocked_files
            .iter()
            .find(|b| b.file_name == n)
            .unwrap()
    };

    let secret = by("secret-1.jar");
    assert_eq!(secret.state, BlockedState::MatchedModrinth);
    assert_eq!(
        secret.modrinth,
        Some(BlockedModrinth {
            project_id: "mr-secret".into(),
            version_id: "mrv-secret".into(),
            title: "Secret Mod".into()
        })
    );
    assert_eq!(secret.name, "P500");
    assert_eq!(secret.sha1.as_deref(), Some(sha1(SECRET).as_str()));
    assert_eq!(
        by("multi-1.jar").state,
        BlockedState::Pending,
        "two loaders: the user decides"
    );
    assert!(by("multi-1.jar").modrinth.is_none());
    let nohash = by("nohash-1.jar");
    assert_eq!(
        (nohash.state, nohash.sha1.clone(), nohash.size),
        (BlockedState::Pending, None, Some(NOHASH.len() as u64))
    );
    let shader = by("pretty-blocked.zip");
    assert_eq!(
        (shader.state, shader.kind),
        (BlockedState::Pending, ContentType::Shader)
    );
    assert!(shader.url.contains("curseforge.com"));
    assert!(
        names(&plan.blocked_files).iter().all(|n| !n.contains("..")),
        "an unusable file name never reaches the list"
    );
    assert_eq!(plan.blocked_files.len(), 4);

    // One batch call, sha1, only files that have a hash.
    let posts = f.posts();
    assert_eq!(posts.len(), 1);
    assert_eq!(posts[0]["algorithm"], "sha1");
    let mut sent: Vec<String> = posts[0]["hashes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|h| h.as_str().unwrap().to_string())
        .collect();
    sent.sort();
    let mut want = vec![sha1(SECRET), sha1(MULTI), sha1(SHADER)];
    want.sort();
    assert_eq!(sent, want);

    // Wire shape of one entry.
    assert_eq!(
        serde_json::to_value(secret).unwrap(),
        json!({
            "name": "P500", "fileName": "secret-1.jar", "url": secret.url, "sha1": sha1(SECRET),
            "size": SECRET.len(), "type": "mod", "state": "matched_modrinth",
            "modrinth": {"projectId": "mr-secret", "versionId": "mrv-secret", "title": "Secret Mod"},
            "foundPath": null
        })
    );
}

#[tokio::test]
async fn the_modrinth_fallback_can_be_switched_off() {
    let f = setup().await;
    let view = f
        .svc
        .config_set(json!({"blockedModsModrinthFallback": false}))
        .unwrap();
    assert!(!view.blocked_mods_modrinth_fallback);
    let plan = f
        .svc
        .modpack_plan(Provider::Curseforge, "1000".into(), None)
        .await
        .unwrap();
    assert!(plan
        .blocked_files
        .iter()
        .all(|b| b.state == BlockedState::Pending && b.modrinth.is_none()));
    assert!(f.posts().is_empty(), "Modrinth is not even asked");
    let p = f.install().await;
    assert!(!f.instance(&p.id).join("mods/secret-modrinth.jar").exists());
    assert_eq!(p.pending_blocked, 4);
}

#[tokio::test]
async fn install_takes_matches_from_modrinth_and_persists_the_rest() {
    let f = setup().await;
    let p = f.install().await;
    let dir = f.instance(&p.id);

    // The match comes from Modrinth, verified, never from CurseForge.
    assert_eq!(
        std::fs::read(dir.join("mods/secret-modrinth.jar")).unwrap(),
        SECRET
    );
    assert!(f
        .cf_seen
        .lock()
        .unwrap()
        .iter()
        .all(|r| !r.path.starts_with("/cdn/secret") && !r.path.starts_with("/cdn/multi")));
    let rec = f
        .svc
        .profile(&p.id)
        .unwrap()
        .mods
        .into_iter()
        .find(|m| m.filename.as_deref() == Some("secret-modrinth.jar"))
        .unwrap();
    assert_eq!(rec.provider.as_deref(), Some("modrinth"));
    assert_eq!(rec.project_id.as_deref(), Some("mr-secret"));
    assert_eq!(rec.version_id.as_deref(), Some("mrv-secret"));
    assert_eq!(rec.loaders, Some(vec!["fabric".to_string()]));
    assert_eq!(rec.local, None);
    assert!(!dir.join("mods/secret-1.jar").exists());

    // The rest waits; the counter and the list say so.
    assert_eq!(p.pending_blocked, 3);
    let files = f.svc.blocked_files(p.id.clone()).unwrap();
    let state = |n: &str| files.iter().find(|b| b.file_name == n).unwrap().state;
    assert_eq!(state("secret-1.jar"), BlockedState::MatchedModrinth);
    assert_eq!(state("multi-1.jar"), BlockedState::Pending);
    assert_eq!(state("nohash-1.jar"), BlockedState::Pending);
    assert_eq!(state("pretty-blocked.zip"), BlockedState::Pending);
    assert!(f
        .sink
        .warnings()
        .iter()
        .any(|w| w.contains("multi-1.jar") && w.contains("curseforge.com")));
    assert!(!f.sink.warnings().iter().any(|w| w.contains("secret-1.jar")));
    assert!(f.sink.events().iter().any(
        |e| matches!(e, Event::BlockedChanged(c) if c.profile_id == p.id && c.files.len() == 4)
    ));
    assert_eq!(
        f.svc
            .app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|x| x.id == p.id)
            .unwrap()
            .pending_blocked,
        3
    );

    // Survives a restart: a second service over the same data dir.
    let again = AppService::new(
        f.svc.paths().clone(),
        Arc::new(Sink::default()),
        ServiceOptions {
            open_folders: false,
            update_dev: true,
            auto_update_check: false,
            partners: Vec::new(),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(again.blocked_files(p.id.clone()).unwrap(), files);
    assert_eq!(
        again
            .app_state()
            .unwrap()
            .profiles
            .iter()
            .find(|x| x.id == p.id)
            .unwrap()
            .pending_blocked,
        3
    );
    // And the profile JSON carries the counter.
    assert_eq!(serde_json::to_value(&p).unwrap()["pendingBlocked"], 3);
}

#[tokio::test]
async fn a_failed_modrinth_download_falls_back_to_pending() {
    let f = setup().await;
    f.files_broken.store(true, Ordering::SeqCst);
    let p = f.install().await;
    assert_eq!(f.state(&p.id, "secret-1.jar"), BlockedState::Pending);
    assert!(!f.instance(&p.id).join("mods/secret-modrinth.jar").exists());
    assert_eq!(p.pending_blocked, 4);
    assert!(f.sink.warnings().iter().any(|w| w.contains("secret-1.jar")));
}

#[tokio::test]
async fn a_tampered_sidecar_cannot_smuggle_unsafe_names() {
    let f = setup().await;
    let p = f.install().await;
    let file = f.instance(&p.id).join(".mangoclient-blocked.json");
    let mut raw: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    let mut evil = raw["files"][1].clone();
    evil["fileName"] = json!("../../evil.jar");
    raw["files"].as_array_mut().unwrap().push(evil);
    std::fs::write(&file, serde_json::to_vec(&raw).unwrap()).unwrap();
    let list = f.svc.blocked_files(p.id.clone()).unwrap();
    assert_eq!(list.len(), 4);
    assert!(names(&list).iter().all(|n| !n.contains('/')));
    assert!(f
        .svc
        .blocked_skip(p.id.clone(), "../../evil.jar".into())
        .is_err());
    assert!(!f.tmp.path().join("evil.jar").exists());
}

// ---------------------------------------------------------------- watcher, step by step

#[tokio::test]
async fn a_complete_matching_file_is_copied_and_the_source_stays() {
    let f = setup().await;
    let p = f.install().await;
    let mut st = WatchState::default();
    std::fs::write(f.dl.join("whatever (1).jar"), MULTI).unwrap();

    assert!(!f.ticks(&p.id, &mut st, 1).await);
    assert_eq!(
        f.state(&p.id, "multi-1.jar"),
        BlockedState::Pending,
        "first sighting: size not yet confirmed"
    );
    assert!(!f.ticks(&p.id, &mut st, 1).await);
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Installed);

    assert_eq!(
        std::fs::read(f.instance(&p.id).join("mods/multi-1.jar")).unwrap(),
        MULTI
    );
    assert_eq!(
        std::fs::read(f.dl.join("whatever (1).jar")).unwrap(),
        MULTI,
        "copied, not moved"
    );
    assert!(std::fs::read_dir(f.instance(&p.id).join("mods"))
        .unwrap()
        .all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .contains("mango-part")));
    assert_eq!(
        f.svc
            .blocked_files(p.id.clone())
            .unwrap()
            .iter()
            .find(|b| b.file_name == "multi-1.jar")
            .unwrap()
            .found_path
            .as_deref(),
        Some(f.dl.join("whatever (1).jar").to_str().unwrap())
    );

    // The transitions were announced, in order, and the mod list picked it up.
    let seq: Vec<BlockedState> = f
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::BlockedChanged(c) => c
                .files
                .into_iter()
                .find(|b| b.file_name == "multi-1.jar")
                .map(|b| b.state),
            _ => None,
        })
        .collect();
    assert_eq!(
        seq,
        vec![
            BlockedState::Pending,
            BlockedState::Found,
            BlockedState::Installed
        ]
    );
    assert!(f
        .svc
        .profile(&p.id)
        .unwrap()
        .mods
        .iter()
        .any(|m| m.filename.as_deref() == Some("multi-1.jar")));
    assert_eq!(f.pending(&p.id), 2);
}

#[tokio::test]
async fn temp_download_files_are_ignored_until_renamed() {
    for n in [
        "a.jar.crdownload",
        "a.PART",
        "a.tmp",
        "a.download",
        "a.jar.opdownload",
    ] {
        assert!(is_temp_name(n), "{n}");
    }
    assert!(!is_temp_name("a.jar"));
    let f = setup().await;
    let p = f.install().await;
    let mut st = WatchState::default();
    let partial = f.dl.join("multi-1.jar.crdownload");
    std::fs::write(&partial, MULTI).unwrap();
    f.ticks(&p.id, &mut st, 4).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Pending);
    assert!(!f.instance(&p.id).join("mods/multi-1.jar").exists());

    std::fs::rename(&partial, f.dl.join("multi-1.jar")).unwrap();
    f.ticks(&p.id, &mut st, 2).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Installed);
}

#[tokio::test]
async fn a_file_whose_size_is_still_changing_is_ignored() {
    let f = setup().await;
    let p = f.install().await;
    let mut st = WatchState::default();
    let file = f.dl.join("multi-1.jar");
    std::fs::write(&file, &MULTI[..5]).unwrap();
    f.ticks(&p.id, &mut st, 1).await;
    // The download finishes between two polls: complete and correct, yet the
    // size differs from the last poll.
    std::fs::write(&file, MULTI).unwrap();
    f.ticks(&p.id, &mut st, 1).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Pending);
    assert!(!f.instance(&p.id).join("mods/multi-1.jar").exists());
    f.ticks(&p.id, &mut st, 1).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Installed);
}

#[tokio::test]
async fn a_file_with_the_wrong_hash_is_refused_and_the_entry_stays_pending() {
    let f = setup().await;
    let p = f.install().await;
    let mut st = WatchState::default();
    // Same name, same size, other bytes.
    let fake = vec![b'X'; MULTI.len()];
    std::fs::write(f.dl.join("multi-1.jar"), &fake).unwrap();
    f.ticks(&p.id, &mut st, 4).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Pending);
    assert!(!f.instance(&p.id).join("mods/multi-1.jar").exists());
    assert_eq!(
        std::fs::read(f.dl.join("multi-1.jar")).unwrap(),
        fake,
        "the user's file is left alone"
    );
    // The genuine file under another name is still found.
    std::fs::write(f.dl.join("multi-1 (1).jar"), MULTI).unwrap();
    f.ticks(&p.id, &mut st, 2).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Installed);
    assert_eq!(
        std::fs::read(f.instance(&p.id).join("mods/multi-1.jar")).unwrap(),
        MULTI
    );
}

#[tokio::test]
async fn without_a_hash_the_exact_name_and_size_must_match() {
    let f = setup().await;
    let p = f.install().await;
    let mut st = WatchState::default();
    std::fs::write(f.dl.join("other-name.jar"), NOHASH).unwrap();
    std::fs::write(f.dl.join("nohash-1.jar"), b"short").unwrap();
    f.ticks(&p.id, &mut st, 3).await;
    assert_eq!(f.state(&p.id, "nohash-1.jar"), BlockedState::Pending);
    std::fs::write(f.dl.join("nohash-1.jar"), NOHASH).unwrap();
    f.ticks(&p.id, &mut st, 3).await;
    assert_eq!(f.state(&p.id, "nohash-1.jar"), BlockedState::Installed);
    assert_eq!(
        std::fs::read(f.instance(&p.id).join("mods/nohash-1.jar")).unwrap(),
        NOHASH
    );
}

#[tokio::test]
async fn shader_files_land_in_their_own_folder() {
    let f = setup().await;
    let p = f.install().await;
    let mut st = WatchState::default();
    std::fs::write(f.dl.join("p.zip"), SHADER).unwrap();
    f.ticks(&p.id, &mut st, 2).await;
    assert_eq!(
        f.state(&p.id, "pretty-blocked.zip"),
        BlockedState::Installed
    );
    assert_eq!(
        std::fs::read(f.instance(&p.id).join("shaderpacks/pretty-blocked.zip")).unwrap(),
        SHADER
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_are_never_followed() {
    let f = setup().await;
    let p = f.install().await;
    f.svc
        .config_set(json!({"blockedModsCheckSubfolders": true}))
        .unwrap();
    let elsewhere = f.tmp.path().join("elsewhere");
    std::fs::create_dir_all(&elsewhere).unwrap();
    std::fs::write(elsewhere.join("multi.jar"), MULTI).unwrap();
    std::os::unix::fs::symlink(elsewhere.join("multi.jar"), f.dl.join("link.jar")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, f.dl.join("linkdir")).unwrap();
    let seen = scan(std::slice::from_ref(&f.dl), true);
    assert!(seen.is_empty(), "{seen:?}");
    let mut st = WatchState::default();
    f.ticks(&p.id, &mut st, 4).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Pending);
}

#[tokio::test]
async fn subfolders_are_only_searched_when_enabled() {
    let f = setup().await;
    let p = f.install().await;
    std::fs::create_dir_all(f.dl.join("sub/deeper")).unwrap();
    std::fs::write(f.dl.join("sub/deeper/multi-1.jar"), MULTI).unwrap();
    let mut st = WatchState::default();
    f.ticks(&p.id, &mut st, 3).await;
    assert_eq!(
        f.state(&p.id, "multi-1.jar"),
        BlockedState::Pending,
        "depth 1 by default"
    );
    f.svc
        .config_set(json!({"blockedModsCheckSubfolders": true}))
        .unwrap();
    f.ticks(&p.id, &mut st, 2).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Installed);
}

#[tokio::test]
async fn a_skipped_file_is_never_installed_and_unknown_names_are_refused() {
    let f = setup().await;
    let p = f.install().await;
    let list = f
        .svc
        .blocked_skip(p.id.clone(), "multi-1.jar".into())
        .unwrap();
    assert_eq!(
        list.iter()
            .find(|b| b.file_name == "multi-1.jar")
            .unwrap()
            .state,
        BlockedState::Skipped
    );
    assert_eq!(f.pending(&p.id), 2);
    assert!(f.sink.events().iter().any(|e| matches!(e, Event::BlockedChanged(c) if c.files.iter().any(|b| b.state == BlockedState::Skipped))));
    assert_eq!(
        f.svc
            .blocked_skip(p.id.clone(), "nope.jar".into())
            .unwrap_err()
            .code,
        "blocked_file_not_found"
    );
    assert_eq!(
        f.svc
            .blocked_skip("12345678-1234-1234-1234-123456789abc".into(), "x".into())
            .unwrap_err()
            .code,
        "profile_not_found"
    );

    let mut st = WatchState::default();
    std::fs::write(f.dl.join("multi-1.jar"), MULTI).unwrap();
    f.ticks(&p.id, &mut st, 3).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Skipped);
    assert!(!f.instance(&p.id).join("mods/multi-1.jar").exists());
    // Skipping an already skipped file changes nothing.
    assert!(f
        .svc
        .blocked_skip(p.id.clone(), "multi-1.jar".into())
        .is_ok());
}

// ---------------------------------------------------------------- watcher, real timer

#[tokio::test]
async fn the_watcher_picks_up_a_file_that_appears_later_and_stops_itself() {
    let f = setup().await;
    let p = f.install().await;
    f.skip_all_but(&p.id, &["multi-1.jar"]);
    let dirs = f.svc.blocked_watch_start(p.id.clone()).await.unwrap();
    assert_eq!(dirs, vec![f.dl.display().to_string()]);
    assert!(f.svc.blocked_watching(&p.id));
    // Starting twice keeps the one watcher.
    f.svc.blocked_watch_start(p.id.clone()).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(f.state(&p.id, "multi-1.jar"), BlockedState::Pending);

    std::fs::write(f.dl.join("multi-1.jar.crdownload"), &MULTI[..4]).unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;
    std::fs::write(f.dl.join("multi-1.jar"), MULTI).unwrap();
    std::fs::remove_file(f.dl.join("multi-1.jar.crdownload")).unwrap();

    wait_for("the file to be installed", || {
        f.state(&p.id, "multi-1.jar") == BlockedState::Installed
    })
    .await;
    wait_for("the watcher to stop", || !f.svc.blocked_watching(&p.id)).await;
    assert_eq!(
        std::fs::read(f.instance(&p.id).join("mods/multi-1.jar")).unwrap(),
        MULTI
    );
    assert!(f.dl.join("multi-1.jar").exists());
    assert_eq!(f.pending(&p.id), 0);
}

#[tokio::test]
async fn the_watcher_stops_when_everything_left_is_skipped_or_it_is_stopped() {
    let f = setup().await;
    let p = f.install().await;
    f.svc.blocked_watch_start(p.id.clone()).await.unwrap();
    assert!(f.svc.blocked_watching(&p.id));
    f.skip_all_but(&p.id, &[]);
    wait_for("the watcher to stop", || !f.svc.blocked_watching(&p.id)).await;

    // Nothing to wait for: no watcher is started, the folders are still listed.
    let dirs = f.svc.blocked_watch_start(p.id.clone()).await.unwrap();
    assert_eq!(dirs.len(), 1);
    assert!(!f.svc.blocked_watching(&p.id));

    let q = f.install().await;
    f.svc.blocked_watch_start(q.id.clone()).await.unwrap();
    assert!(f.svc.blocked_watching(&q.id));
    f.svc.blocked_watch_stop(q.id.clone()).unwrap();
    assert!(!f.svc.blocked_watching(&q.id));
    f.svc.blocked_watch_stop(q.id.clone()).unwrap();
}

#[tokio::test]
async fn dropping_the_service_ends_its_watchers() {
    let f = setup().await;
    let p = f.install().await;
    f.svc.blocked_watch_start(p.id.clone()).await.unwrap();
    let weak = Arc::downgrade(&f.svc.inner);
    let P5 { svc, .. } = f;
    drop(svc);
    wait_for("the service to be released", || weak.upgrade().is_none()).await;
}

// ---------------------------------------------------------------- config

#[tokio::test]
async fn the_watch_folders_come_from_the_config_then_the_downloads_dir() {
    let f = setup().await;
    let defaults = f.svc.app_state().unwrap().config;
    assert!(defaults.blocked_mods_modrinth_fallback);
    assert!(!defaults.blocked_mods_check_subfolders);
    assert_eq!(
        defaults.blocked_mods_watch_dirs,
        vec![f.dl.display().to_string()]
    );

    assert_eq!(
        f.svc
            .config_set(json!({"blockedModsWatchDirs": ["relative"]}))
            .unwrap_err()
            .code,
        "validation"
    );
    assert_eq!(
        f.svc
            .config_set(json!({"blockedModsWatchDirs": [f.tmp.path().join("missing")]}))
            .unwrap_err()
            .code,
        "validation"
    );
    assert_eq!(
        f.svc.blocked_open_folder().unwrap(),
        f.dl.display().to_string()
    );

    f.svc
        .config_set(json!({"blockedModsWatchDirs": []}))
        .unwrap();
    let p = f.install().await;
    let dirs = f.svc.blocked_watch_start(p.id.clone()).await.unwrap();
    assert_eq!(
        dirs,
        vec![f.dl.display().to_string()],
        "falls back to the injected Downloads folder"
    );
    f.svc.blocked_watch_stop(p.id).unwrap();

    let view = serde_json::to_value(f.svc.app_state().unwrap().config).unwrap();
    assert_eq!(view["blockedModsWatchDirs"], json!([]));
    assert_eq!(view["blockedModsModrinthFallback"], true);
    assert_eq!(view["blockedModsCheckSubfolders"], false);
}

#[tokio::test]
async fn the_event_has_the_contract_name_and_shape() {
    let f = setup().await;
    let p = f.install().await;
    let ev = f
        .sink
        .events()
        .into_iter()
        .find(|e| matches!(e, Event::BlockedChanged(_)))
        .unwrap();
    assert_eq!(ev.name(), "blocked:changed");
    let payload = ev.payload();
    assert_eq!(payload["profileId"], p.id);
    assert_eq!(payload["files"].as_array().unwrap().len(), 4);
    assert!(payload["files"][0]["state"].as_str().is_some());
}
