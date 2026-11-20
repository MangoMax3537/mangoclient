#![allow(clippy::unwrap_used)]
//! Part 2 of `docs/ipc.md`: content, discover, servers, media, logs, stats,
//! storage, updates and the events. No real network: local mock servers.

use crate::image_protocol::{self, Image, ImageError};
use crate::service::lock;
use crate::*;
use mango_core::Paths;
use mango_servers::{nbt, IconStore};
use serde_json::{json, Value};
use sha2::{Digest, Sha512};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

// ---------- fixtures ----------

#[derive(Default)]
struct MockSink(Mutex<Vec<Event>>);

impl EventSink for MockSink {
    fn emit(&self, event: Event) {
        self.0.lock().unwrap().push(event);
    }
}

impl MockSink {
    fn events(&self) -> Vec<Event> {
        self.0.lock().unwrap().clone()
    }
}

struct Fx {
    tmp: tempfile::TempDir,
    svc: AppService,
    sink: Arc<MockSink>,
    /// A selected fabric 1.21.1 profile.
    pid: String,
}

fn fx_host(host: Option<Host>, tweak: impl FnOnce(&mut ServiceOptions)) -> Fx {
    let tmp = tempfile::tempdir().unwrap();
    let sink = Arc::new(MockSink::default());
    let mut opts = ServiceOptions {
        open_folders: false,
        update_dev: true,
        auto_update_check: false,
        fetch: mango_net::FetchOptions {
            retries: 0,
            ..mango_net::FetchOptions::default()
        },
        partners: Vec::new(),
        download_policy: Some(mango_net::HostPolicy::allow_all()),
        ..ServiceOptions::default()
    };
    tweak(&mut opts);
    let paths = Paths::new(tmp.path());
    let svc = match host {
        Some(h) => AppService::with_host(paths, sink.clone(), opts, h).unwrap(),
        None => AppService::new(paths, sink.clone(), opts).unwrap(),
    };
    let p = svc
        .profile_create("T".into(), "1.21.1".into(), "fabric".into(), None)
        .unwrap();
    svc.profile_select(p.id.clone()).unwrap();
    Fx {
        tmp,
        svc,
        sink,
        pid: p.id,
    }
}

fn fx() -> Fx {
    fx_host(None, |_| {})
}

impl Fx {
    fn paths(&self) -> &Paths {
        self.svc.paths()
    }
    fn dir(&self) -> PathBuf {
        self.paths().instance_dir(&self.pid).unwrap()
    }
}

async fn wait_for(mut f: impl FnMut() -> bool) {
    for _ in 0..300 {
        if f() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("condition not reached in time");
}

// ---------- mock http ----------

#[derive(Debug, Clone)]
pub(crate) struct Req {
    pub(crate) method: String,
    pub(crate) path: String,
    pub(crate) body: Vec<u8>,
}

pub(crate) type Handler = Arc<dyn Fn(&Req) -> (u16, Vec<u8>) + Send + Sync>;

pub(crate) async fn serve(make: impl FnOnce(&str) -> Handler) -> (String, Arc<Mutex<Vec<Req>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let handler = make(&base);
    let seen: Arc<Mutex<Vec<Req>>> = Arc::default();
    let log = seen.clone();
    tokio::spawn(async move {
        loop {
            let Ok((mut sock, _)) = listener.accept().await else {
                return;
            };
            let (handler, log) = (handler.clone(), log.clone());
            tokio::spawn(async move {
                let mut buf = Vec::new();
                let mut tmp = [0u8; 4096];
                let head_end = loop {
                    if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => return,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                };
                let head = String::from_utf8_lossy(&buf[..head_end]).to_string();
                let len: usize = head
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .and_then(|v| v.trim().parse().ok())
                    })
                    .unwrap_or(0);
                while buf.len() < head_end + len {
                    match sock.read(&mut tmp).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => buf.extend_from_slice(&tmp[..n]),
                    }
                }
                let mut first = head.split_whitespace();
                let req = Req {
                    method: first.next().unwrap_or("").to_string(),
                    path: first.next().unwrap_or("/").to_string(),
                    body: buf[head_end..].to_vec(),
                };
                let (status, body) = handler(&req);
                log.lock().unwrap().push(req);
                let mut out = format!(
                    "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                    body.len()
                )
                .into_bytes();
                out.extend_from_slice(&body);
                let _ = sock.write_all(&out).await;
            });
        }
    });
    (base, seen)
}

pub(crate) fn json_ok(v: Value) -> (u16, Vec<u8>) {
    (200, serde_json::to_vec(&v).unwrap())
}

fn pct_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap()
}

fn sha512(b: &[u8]) -> String {
    hex::encode(Sha512::digest(b))
}

fn version_json(base: &str, id: &str, num: &str, date: &str, file: &str, bytes: &[u8]) -> Value {
    json!({
        "id": id, "project_id": "p1", "name": format!("Cool {num}"), "version_number": num,
        "version_type": "release", "date_published": date, "downloads": 7,
        "game_versions": ["1.21.1"], "loaders": ["fabric"],
        "files": [{"url": format!("{base}/files/{file}"), "filename": file, "primary": true,
                   "size": bytes.len(), "hashes": {"sha512": sha512(bytes)}}],
        "dependencies": []
    })
}

fn project_json(id: &str, kind: &str) -> Value {
    json!({
        "id": id, "slug": "cool", "title": "Cool", "project_type": kind,
        "icon_url": "https://cdn.modrinth.com/data/p1/icon.png",
        "description": "Short", "body": "# Long", "downloads": 1234, "followers": 56,
        "categories": ["optimization"], "game_versions": ["1.21.1"], "loaders": ["fabric"],
        "updated": "2024-02-02T00:00:00Z",
        "gallery": [{"url": "https://cdn.modrinth.com/data/p1/images/a.png"}],
        "license": {"id": "MIT", "name": "MIT License"},
        "source_url": "https://github.com/x/y", "issues_url": "http://insecure.example/issues"
    })
}

fn write_mod(fx: &Fx, rec: Value) {
    fx.svc
        .inner
        .with_store(|s| {
            let mut mods = s.get_profile(&fx.pid).unwrap().mods.clone();
            mods.push(serde_json::from_value(rec).unwrap());
            s.update_profile(&fx.pid, &json!({"mods": mods}))
                .map(|_| ())
        })
        .unwrap();
}

// ---------- events and wire shapes ----------

#[test]
fn new_events_have_contract_names_and_camel_case_payloads() {
    let e = Event::InstallProgress(InstallProgressEvent {
        profile_id: "p".into(),
        project_id: "m".into(),
        done: 1,
        total: 2,
        label: "x".into(),
    });
    assert_eq!(e.name(), "install:progress");
    assert_eq!(
        e.payload(),
        json!({"profileId": "p", "projectId": "m", "done": 1, "total": 2, "label": "x"})
    );
    let e = Event::ServersPinged(ServerStatus {
        id: "s".into(),
        online: true,
        ping: Some(5),
        motd: "m".into(),
        players: Some(ServerPlayers { online: 1, max: 2 }),
        version: None,
    });
    assert_eq!(e.name(), "servers:pinged");
    assert_eq!(
        e.payload(),
        json!({"id": "s", "online": true, "ping": 5, "motd": "m",
               "players": {"online": 1, "max": 2}, "version": null})
    );
    let e = Event::ContentChanged(ContentChangedEvent {
        profile_id: "p".into(),
    });
    assert_eq!(e.name(), "content:changed");
    assert_eq!(e.payload(), json!({"profileId": "p"}));
    let st = UpdateStatus::from(&mango_updater::UpdateStatus {
        state: mango_updater::UpdateState::Downloading {
            version: "2.0.0".into(),
            percent: 40,
        },
        current: "1.0.0".into(),
        applying: false,
    });
    let e = Event::UpdateStatus(st);
    assert_eq!(e.name(), "update:status");
    assert_eq!(
        e.payload(),
        json!({"state": "downloading", "percent": 40, "version": "2.0.0"})
    );
}

#[test]
fn update_status_maps_every_state() {
    use mango_updater::{DisabledReason, ErrorCode, UpdateState};
    let map = |state| {
        serde_json::to_value(UpdateStatus::from(&mango_updater::UpdateStatus {
            state,
            current: "1.0.0".into(),
            applying: false,
        }))
        .unwrap()
    };
    assert_eq!(map(UpdateState::Idle), json!({"state": "idle"}));
    assert_eq!(map(UpdateState::Checking), json!({"state": "checking"}));
    assert_eq!(map(UpdateState::Current), json!({"state": "current"}));
    assert_eq!(
        map(UpdateState::Ready {
            version: "2.0.0".into()
        }),
        json!({"state": "ready", "version": "2.0.0"})
    );
    for (code, wire) in [
        (ErrorCode::NoRelease, "no-release"),
        (ErrorCode::Offline, "offline"),
        (ErrorCode::Other, "other"),
    ] {
        assert_eq!(
            map(UpdateState::Error {
                code,
                message: "m".into()
            }),
            json!({"state": "error", "errorCode": wire})
        );
    }
    assert_eq!(
        map(UpdateState::Disabled {
            reason: DisabledReason::Unsupported
        }),
        json!({"state": "disabled", "disabledReason": "unsupported"})
    );
}

#[test]
fn accounts_carry_a_head_url() {
    let f = fx();
    let a = f.svc.account_add_offline("Steve_1".into()).unwrap();
    assert_eq!(a.head_url, format!("mangoimg://head/{}", a.uuid));
    let v = serde_json::to_value(&a).unwrap();
    assert_eq!(v["headUrl"], a.head_url);
    let st = f.svc.app_state().unwrap();
    assert_eq!(st.accounts[0].head_url, a.head_url);
    // And it resolves (Steve fallback: nothing cached yet).
    assert!(matches!(
        image_protocol::resolve(f.paths(), &a.head_url),
        Ok(Image::Bytes { .. })
    ));
}

// ---------- local content ----------

#[tokio::test]
async fn content_list_syncs_the_mods_folder_and_persists() {
    let f = fx();
    let mods = f.dir().join("mods");
    std::fs::write(mods.join("foo.jar"), b"x").unwrap();
    std::fs::write(mods.join("bar.jar.disabled"), b"y").unwrap();
    std::fs::write(mods.join("readme.txt"), b"z").unwrap();
    let items = f.svc.content_list(f.pid.clone()).await.unwrap();
    assert_eq!(items.len(), 2);
    let foo = items.iter().find(|i| i.filename == "foo.jar").unwrap();
    assert!(foo.local && foo.enabled && foo.project_id == "local:foo.jar");
    assert_eq!(foo.kind, ContentType::Mod);
    assert!(foo.update_available.is_none() && foo.required_by.is_empty() && !foo.dependency);
    let bar = items.iter().find(|i| i.filename == "bar.jar").unwrap();
    assert!(!bar.enabled);
    let v = serde_json::to_value(foo).unwrap();
    for k in [
        "projectId",
        "slug",
        "title",
        "type",
        "versionId",
        "versionNumber",
        "filename",
        "enabled",
        "local",
        "iconUrl",
        "requiredBy",
        "dependency",
        "updateAvailable",
        "installedAt",
    ] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
    // Persisted into the profile.
    assert_eq!(f.svc.app_state().unwrap().profiles[1].mod_count, 2);
    // A jar deleted outside the launcher disappears.
    std::fs::remove_file(mods.join("foo.jar")).unwrap();
    let items = f.svc.content_list(f.pid.clone()).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(
        f.svc.content_list("nope".into()).await.unwrap_err().code,
        "invalid_profile_id"
    );
    assert_eq!(
        f.svc
            .content_list("12345678-1234-1234-1234-123456789abc".into())
            .await
            .unwrap_err()
            .code,
        "profile_not_found"
    );
}

#[tokio::test]
async fn toggling_and_uninstalling_local_content() {
    let f = fx();
    let jar = f.dir().join("mods/foo.jar");
    std::fs::write(&jar, b"x").unwrap();
    f.svc.content_list(f.pid.clone()).await.unwrap();
    let r = f
        .svc
        .content_toggle(f.pid.clone(), "local:foo.jar".into(), false)
        .await
        .unwrap();
    assert_eq!(r.affected, vec!["local:foo.jar".to_string()]);
    assert!(!r.items[0].enabled);
    assert!(!jar.exists() && jar.with_extension("jar.disabled").exists());
    let r = f
        .svc
        .content_toggle(f.pid.clone(), "local:foo.jar".into(), true)
        .await
        .unwrap();
    assert!(r.items[0].enabled && jar.exists());
    assert_eq!(
        f.svc
            .content_toggle(f.pid.clone(), "local:missing.jar".into(), true)
            .await
            .unwrap_err()
            .code,
        "content_not_found"
    );
    let u = f
        .svc
        .content_uninstall(f.pid.clone(), "local:foo.jar".into())
        .await
        .unwrap();
    assert!(!u.blocked && u.dependents.is_empty() && u.items.is_empty());
    assert!(!jar.exists());
    assert_eq!(
        f.svc
            .content_uninstall(f.pid.clone(), "local:foo.jar".into())
            .await
            .unwrap_err()
            .code,
        "content_not_found"
    );
}

#[test]
fn content_open_folder_maps_types_to_instance_folders() {
    let f = fx();
    for (kind, dir) in [
        (ContentType::Mod, "mods"),
        (ContentType::Shader, "shaderpacks"),
        (ContentType::Resourcepack, "resourcepacks"),
        (ContentType::Datapack, "datapacks"),
    ] {
        let p = f.svc.content_open_folder(f.pid.clone(), kind).unwrap();
        assert!(Path::new(&p).is_dir() && p.ends_with(dir), "{p}");
    }
    assert!(f
        .svc
        .content_open_folder("../x".into(), ContentType::Mod)
        .is_err());
}

// ---------- Modrinth ----------

fn mod_server() -> impl FnOnce(&str) -> Handler {
    |base| {
        let base = base.to_string();
        Arc::new(move |req: &Req| {
            let path = req.path.split('?').next().unwrap();
            match path {
                "/project/p1" => json_ok(project_json("p1", "mod")),
                "/project/p1/members" => json_ok(json!([
                    {"role": "Member", "user": {"username": "bob"}},
                    {"role": "Owner", "user": {"username": "alice"}}
                ])),
                "/project/p1/version" => json_ok(json!([
                    version_json(
                        &base,
                        "v2",
                        "2.0",
                        "2024-03-01T00:00:00Z",
                        "cool-2.jar",
                        b"two"
                    ),
                    version_json(
                        &base,
                        "v1",
                        "1.0",
                        "2024-01-01T00:00:00Z",
                        "cool.jar",
                        b"jar-bytes-1"
                    ),
                ])),
                "/version/v1" => json_ok(version_json(
                    &base,
                    "v1",
                    "1.0",
                    "2024-01-01T00:00:00Z",
                    "cool.jar",
                    b"jar-bytes-1",
                )),
                "/files/cool.jar" => (200, b"jar-bytes-1".to_vec()),
                _ => (404, b"{}".to_vec()),
            }
        })
    }
}

#[tokio::test]
async fn install_toggle_check_updates_and_uninstall_against_a_mock_modrinth() {
    let (base, _seen) = serve(mod_server()).await;
    let f = fx_host(None, |o| o.modrinth_api = base.clone());

    // Install picks the newest release for fabric/1.21.1 ("v2" file is not
    // served), so pin v1.
    let items = f
        .svc
        .modrinth_install(f.pid.clone(), "p1".into(), Some("v1".into()))
        .await
        .unwrap();
    assert_eq!(items.len(), 1);
    let it = &items[0];
    assert_eq!(
        (
            it.project_id.as_str(),
            it.title.as_str(),
            it.filename.as_str()
        ),
        ("p1", "Cool", "cool.jar")
    );
    assert_eq!(it.version_id.as_deref(), Some("v1"));
    assert!(!it.local && it.enabled && !it.dependency);
    assert!(it
        .icon_url
        .as_deref()
        .unwrap()
        .starts_with("mangoimg://icon/"));
    assert_eq!(
        std::fs::read(f.dir().join("mods/cool.jar")).unwrap(),
        b"jar-bytes-1"
    );
    // Persisted.
    let st = f.svc.app_state().unwrap();
    assert_eq!(
        st.profiles
            .iter()
            .find(|p| p.id == f.pid)
            .unwrap()
            .mod_count,
        1
    );
    // install:progress: starts at 0/1 and ends at done 1/1.
    let progress: Vec<_> = f
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::InstallProgress(p) => Some(p),
            _ => None,
        })
        .collect();
    assert!(progress.len() >= 3, "{progress:?}");
    assert!(progress
        .iter()
        .all(|p| p.profile_id == f.pid && p.project_id == "p1"));
    assert_eq!((progress[0].done, progress[0].total), (0, 1));
    let last = progress.last().unwrap();
    assert_eq!((last.done, last.total, last.label.as_str()), (1, 1, "done"));

    // Search shows it as installed in this profile.
    // Toggle (record already has dependency metadata: no extra requests).
    let t = f
        .svc
        .content_toggle(f.pid.clone(), "p1".into(), false)
        .await
        .unwrap();
    assert_eq!(t.affected, vec!["p1".to_string()]);
    assert!(!t.items[0].enabled);
    assert!(f.dir().join("mods/cool.jar.disabled").is_file());
    f.svc
        .content_toggle(f.pid.clone(), "p1".into(), true)
        .await
        .unwrap();

    // Updates: v2 is newer than the installed v1.
    let updates = f.svc.content_check_updates(f.pid.clone()).await.unwrap();
    assert_eq!(updates.len(), 1);
    let u = &updates[0];
    assert_eq!(
        (
            u.project_id.as_str(),
            u.from.as_deref(),
            u.to.as_str(),
            u.version_id.as_str()
        ),
        ("p1", Some("1.0"), "2.0", "v2")
    );
    assert!(!u.conflict && !u.compatibility);
    let v = serde_json::to_value(u).unwrap();
    assert_eq!(v["type"], "mod");
    assert!(v["requiredBy"].as_array().unwrap().is_empty());
    // content_list now reports the available update.
    let list = f.svc.content_list(f.pid.clone()).await.unwrap();
    assert_eq!(list[0].update_available.as_deref(), Some("2.0"));

    // Uninstall removes file and record.
    let un = f
        .svc
        .content_uninstall(f.pid.clone(), "p1".into())
        .await
        .unwrap();
    assert!(!un.blocked && un.items.is_empty());
    assert!(!f.dir().join("mods/cool.jar").exists());
}

#[tokio::test]
async fn uninstall_is_blocked_by_dependents_and_toggle_follows_the_tree() {
    let f = fx();
    let mods = f.dir().join("mods");
    std::fs::write(mods.join("lib.jar"), b"l").unwrap();
    std::fs::write(mods.join("app.jar"), b"a").unwrap();
    let rec = |id: &str, file: &str, deps: Value| {
        json!({"projectId": id, "title": id, "type": "mod", "filename": file,
               "file": mods.join(file).display().to_string(), "enabled": true,
               "requiredDependencies": deps})
    };
    write_mod(&f, rec("lib", "lib.jar", json!([])));
    write_mod(
        &f,
        rec(
            "app",
            "app.jar",
            json!([{"projectId": "lib", "versionId": null}]),
        ),
    );

    let list = f.svc.content_list(f.pid.clone()).await.unwrap();
    let lib = list.iter().find(|i| i.project_id == "lib").unwrap();
    assert_eq!(lib.required_by, vec!["app".to_string()]);

    let blocked = f
        .svc
        .content_uninstall(f.pid.clone(), "lib".into())
        .await
        .unwrap();
    assert!(blocked.blocked);
    assert_eq!(blocked.dependents, vec!["app".to_string()]);
    assert_eq!(blocked.items.len(), 2);
    assert!(mods.join("lib.jar").exists());

    // Disabling the library also disables what needs it.
    let t = f
        .svc
        .content_toggle(f.pid.clone(), "lib".into(), false)
        .await
        .unwrap();
    assert_eq!(t.affected.len(), 2);
    assert!(t.items.iter().all(|i| !i.enabled));
    assert!(mods.join("app.jar.disabled").exists() && mods.join("lib.jar.disabled").exists());
    // Enabling the app enables its requirement first.
    let t = f
        .svc
        .content_toggle(f.pid.clone(), "app".into(), true)
        .await
        .unwrap();
    assert_eq!(t.affected, vec!["lib".to_string(), "app".to_string()]);

    assert!(
        !f.svc
            .content_uninstall(f.pid.clone(), "app".into())
            .await
            .unwrap()
            .blocked
    );
    assert!(
        !f.svc
            .content_uninstall(f.pid.clone(), "lib".into())
            .await
            .unwrap()
            .blocked
    );
}

#[tokio::test]
async fn search_maps_hits_and_clamps_parameters() {
    let (base, seen) = serve(|_| {
        Arc::new(|req: &Req| {
            if !req.path.starts_with("/search") {
                return (404, b"{}".to_vec());
            }
            json_ok(json!({
                "hits": [
                    {"project_id": "p1", "slug": "cool", "title": "Cool", "author": "alice",
                     "description": "d", "project_type": "mod",
                     "icon_url": "https://cdn.modrinth.com/data/p1/icon.png",
                     "downloads": 10, "follows": 3, "categories": ["fabric", "quilt", "optimization"],
                     "versions": ["1.21", "1.21.1"], "date_modified": "2024-01-01T00:00:00Z"},
                    {"project_id": "p2", "slug": "pack", "title": "Pack", "author": "bob",
                     "description": "", "project_type": "modpack", "icon_url": null,
                     "downloads": 1, "follows": 0, "categories": [], "versions": [],
                     "date_modified": ""},
                    {"slug": "no-id"}
                ],
                "total_hits": 42, "offset": 20, "limit": 2
            }))
        })
    })
    .await;
    let f = fx_host(None, |o| o.modrinth_api = base.clone());
    write_mod(
        &f,
        json!({"projectId": "p1", "title": "Cool", "type": "mod"}),
    );
    let r = f
        .svc
        .modrinth_search(SearchParams {
            provider: Provider::Modrinth,
            query: "cool".into(),
            project_type: "mod".into(),
            loader: Some("fabric".into()),
            mc_version: Some("1.21.1".into()),
            categories: Some(vec!["optimization".into()]),
            sort: "downloads".into(),
            offset: 20,
            limit: 500,
        })
        .await
        .unwrap();
    assert_eq!((r.total, r.offset, r.hits.len()), (42, 20, 2));
    let h = &r.hits[0];
    assert_eq!(
        (h.project_id.as_str(), h.author.as_str(), h.kind.as_str()),
        ("p1", "alice", "mod")
    );
    assert_eq!(h.loaders, vec!["fabric", "quilt"]);
    assert_eq!(h.game_versions, vec!["1.21", "1.21.1"]);
    assert_eq!(h.installed_in, vec![f.pid.clone()]);
    assert!(h
        .icon_url
        .as_deref()
        .unwrap()
        .starts_with("mangoimg://icon/"));
    assert_eq!(r.hits[1].kind, "modpack");
    assert!(r.hits[1].icon_url.is_none() && r.hits[1].installed_in.is_empty());
    let v = serde_json::to_value(h).unwrap();
    for k in [
        "projectId",
        "slug",
        "title",
        "author",
        "description",
        "type",
        "iconUrl",
        "downloads",
        "follows",
        "categories",
        "gameVersions",
        "loaders",
        "dateModified",
        "installedIn",
    ] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
    let req = seen.lock().unwrap()[0].clone();
    let q = pct_decode(&req.path);
    assert!(
        q.contains("limit=100") && q.contains("offset=20") && q.contains("index=downloads"),
        "{q}"
    );
    assert!(
        q.contains(r#"["project_type:mod"]"#) && q.contains(r#"["categories:fabric"]"#),
        "{q}"
    );
    assert!(
        q.contains(r#"["versions:1.21.1"]"#) && q.contains(r#"["categories:optimization"]"#),
        "{q}"
    );

    let base_params = SearchParams {
        provider: Provider::Modrinth,
        query: String::new(),
        project_type: "mod".into(),
        loader: None,
        mc_version: None,
        categories: None,
        sort: "relevance".into(),
        offset: 0,
        limit: 10,
    };
    for bad in [
        SearchParams {
            project_type: "plugin".into(),
            ..base_params.clone()
        },
        SearchParams {
            sort: "random".into(),
            ..base_params.clone()
        },
        SearchParams {
            loader: Some("evil".into()),
            ..base_params.clone()
        },
    ] {
        assert_eq!(
            f.svc.modrinth_search(bad).await.unwrap_err().code,
            "validation"
        );
    }
    // A shader search carries no loader facet and skips the mod-only one.
    f.svc
        .modrinth_search(SearchParams {
            project_type: "shader".into(),
            ..base_params
        })
        .await
        .unwrap();
    let q = pct_decode(&seen.lock().unwrap().last().unwrap().path);
    assert!(
        q.contains("project_type:shader") && !q.contains("categories:"),
        "{q}"
    );
}

#[tokio::test]
async fn search_params_deserialize_from_the_ui_shape() {
    let p: SearchParams = serde_json::from_value(json!({
        "query": "x", "projectType": "resourcepack", "sort": "newest", "offset": 0, "limit": 5
    }))
    .unwrap();
    assert_eq!(p.project_type, "resourcepack");
    assert!(p.loader.is_none() && p.categories.is_none());
    let p: SearchParams = serde_json::from_value(json!({"projectType": "mod"})).unwrap();
    assert_eq!(
        (p.sort.as_str(), p.limit, p.query.as_str()),
        ("relevance", 30, "")
    );
}

#[tokio::test]
async fn project_detail_and_versions() {
    let (base, _s) = serve(mod_server()).await;
    let f = fx_host(None, |o| o.modrinth_api = base.clone());
    let d = f.svc.modrinth_project("p1".into()).await.unwrap();
    assert_eq!(d.hit.author, "alice");
    assert_eq!((d.hit.downloads, d.hit.follows), (1234, 56));
    assert_eq!(d.body, "# Long");
    assert_eq!(d.license.as_deref(), Some("MIT License"));
    assert_eq!(d.source_url.as_deref(), Some("https://github.com/x/y"));
    assert_eq!(d.issues_url, None, "non-https links are dropped");
    assert_eq!(d.gallery.len(), 1);
    assert!(d.gallery[0].starts_with("mangoimg://icon/"));
    let v = serde_json::to_value(&d).unwrap();
    assert_eq!(v["projectId"], "p1");
    assert_eq!(v["body"], "# Long");
    assert!(v.get("hit").is_none(), "ProjectDetail is flattened");

    assert_eq!(
        f.svc.modrinth_project("a/b".into()).await.unwrap_err().code,
        "validation"
    );

    // Without a profile everything counts as compatible; newest first.
    let all = f.svc.modrinth_versions("p1".into(), None).await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|v| v.compatible));
    assert_eq!(all[0].version_id, "v2");
    assert_eq!(all[0].kind, "release");
    assert_eq!(all[0].downloads, 7);
    let w = serde_json::to_value(&all[0]).unwrap();
    for k in [
        "versionId",
        "versionNumber",
        "name",
        "type",
        "gameVersions",
        "loaders",
        "datePublished",
        "downloads",
        "compatible",
    ] {
        assert!(w.get(k).is_some(), "missing {k}");
    }
    // A forge profile on another game version matches neither.
    let other = f
        .svc
        .profile_create("F".into(), "1.20.1".into(), "forge".into(), None)
        .unwrap();
    let r = f
        .svc
        .modrinth_versions("p1".into(), Some(other.id))
        .await
        .unwrap();
    assert!(r.iter().all(|v| !v.compatible));
    let r = f
        .svc
        .modrinth_versions("p1".into(), Some(f.pid.clone()))
        .await
        .unwrap();
    assert!(r.iter().all(|v| v.compatible));
}

#[tokio::test]
async fn versions_mark_loader_and_game_version_mismatches() {
    let (base, _s) = serve(|_| {
        Arc::new(|req: &Req| match req.path.split('?').next().unwrap() {
            "/project/p1" => json_ok(project_json("p1", "mod")),
            "/project/p1/version" => json_ok(json!([
                {"id": "old", "version_number": "0.1", "version_type": "beta",
                 "date_published": "2023-01-01T00:00:00Z", "game_versions": ["1.20.1"],
                 "loaders": ["fabric"], "files": []},
                {"id": "wrong", "version_number": "1.0", "version_type": "alpha",
                 "date_published": "2024-06-01T00:00:00Z", "game_versions": ["1.21.1"],
                 "loaders": ["forge"], "files": []},
                {"id": "good", "version_number": "1.1", "version_type": "release",
                 "date_published": "2024-05-01T00:00:00Z", "game_versions": ["1.21.1"],
                 "loaders": ["fabric", "quilt"], "files": []},
            ])),
            _ => (404, b"{}".to_vec()),
        })
    })
    .await;
    let f = fx_host(None, |o| o.modrinth_api = base.clone());
    let r = f
        .svc
        .modrinth_versions("p1".into(), Some(f.pid.clone()))
        .await
        .unwrap();
    let flags: Vec<_> = r
        .iter()
        .map(|v| (v.version_id.as_str(), v.compatible))
        .collect();
    assert_eq!(
        flags,
        vec![("good", true), ("wrong", false), ("old", false)]
    );
    assert_eq!(r[1].kind, "alpha");
    assert_eq!(r[2].kind, "beta");
    // Quilt accepts Fabric builds.
    let q = f
        .svc
        .profile_create("Q".into(), "1.21.1".into(), "quilt".into(), None)
        .unwrap();
    let r = f
        .svc
        .modrinth_versions("p1".into(), Some(q.id))
        .await
        .unwrap();
    assert_eq!(r[0].version_id, "good");
    assert!(r[0].compatible);
}

fn mrpack(base_file: &str, bytes: &[u8], with_override: bool) -> Vec<u8> {
    let index = json!({
        "formatVersion": 1, "game": "minecraft", "versionId": "1", "name": "Pack",
        "files": [{
            "path": "mods/a.jar",
            "hashes": {"sha512": sha512(bytes)},
            "downloads": [base_file], "fileSize": bytes.len(),
            "env": {"client": "required", "server": "required"}
        }],
        "dependencies": {"minecraft": "1.20.1", "fabric-loader": "0.15.0"}
    });
    let mut cur = std::io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut cur);
        let o = zip::write::SimpleFileOptions::default();
        z.start_file("modrinth.index.json", o).unwrap();
        z.write_all(index.to_string().as_bytes()).unwrap();
        if with_override {
            z.start_file("overrides/config/x.txt", o).unwrap();
            z.write_all(b"hello").unwrap();
        }
        z.finish().unwrap();
    }
    cur.into_inner()
}

fn pack_server(file_status: u16) -> impl FnOnce(&str) -> Handler {
    move |base| {
        let base = base.to_string();
        let jar = b"a-jar".to_vec();
        let pack = mrpack(&format!("{base}/files/a.jar"), &jar, true);
        Arc::new(move |req: &Req| {
            let path = req.path.split('?').next().unwrap();
            let pack_version = json!({
                "id": "vp", "project_id": "pack", "name": "Pack 1", "version_number": "1",
                "version_type": "release", "date_published": "2024-01-01T00:00:00Z",
                "game_versions": ["1.20.1"], "loaders": ["fabric"],
                "files": [{"url": format!("{base}/files/pack.mrpack"), "filename": "pack.mrpack",
                           "primary": true, "size": pack.len(),
                           "hashes": {"sha512": sha512(&pack)}}],
                "dependencies": []
            });
            match path {
                "/project/pack" => json_ok(json!({
                    "id": "pack", "slug": "pack", "title": "The Pack", "project_type": "modpack"
                })),
                "/project/cool" => json_ok(project_json("cool", "mod")),
                "/project/pack/version" => json_ok(json!([pack_version])),
                "/version/vp" => json_ok(pack_version),
                "/files/pack.mrpack" => (200, pack.clone()),
                "/files/a.jar" => (file_status, jar.clone()),
                _ => (404, b"{}".to_vec()),
            }
        })
    }
}

#[tokio::test]
async fn modpack_install_creates_a_profile_with_loader_and_version() {
    let (base, _s) = serve(pack_server(200)).await;
    let f = fx_host(None, |o| o.modrinth_api = base.clone());
    let p = f
        .svc
        .modrinth_install_modpack("pack".into(), None, None)
        .await
        .unwrap();
    assert_eq!(p.name, "The Pack");
    assert_eq!(p.mc_version, "1.20.1");
    assert_eq!(p.loader, mango_core::Loader::Fabric);
    assert_eq!(p.loader_version.as_deref(), Some("0.15.0"));
    assert_eq!(p.mod_count, 1, "the pack's jar becomes content");
    let dir = f.paths().instance_dir(&p.id).unwrap();
    assert_eq!(std::fs::read(dir.join("mods/a.jar")).unwrap(), b"a-jar");
    assert_eq!(std::fs::read(dir.join("config/x.txt")).unwrap(), b"hello");
    assert_eq!(f.svc.app_state().unwrap().profiles.len(), 3);
    let progress: Vec<_> = f
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::InstallProgress(i) => Some(i),
            _ => None,
        })
        .collect();
    assert!(progress
        .iter()
        .all(|i| i.profile_id == p.id && i.project_id == "pack"));
    assert!(progress
        .iter()
        .any(|i| i.label == "a.jar" && i.done == 1 && i.total == 1));
    let last = progress.last().unwrap();
    assert_eq!((last.done, last.total), (1, 1));

    // Custom name and a non-modpack are handled.
    let q = f
        .svc
        .modrinth_install_modpack("pack".into(), Some("vp".into()), Some("  Mine  ".into()))
        .await
        .unwrap();
    assert_eq!(q.name, "Mine");
    assert_eq!(
        f.svc
            .modrinth_install_modpack("cool".into(), None, None)
            .await
            .unwrap_err()
            .code,
        "not_a_modpack"
    );
    assert_eq!(
        f.svc
            .modrinth_install(f.pid.clone(), "pack".into(), None)
            .await
            .unwrap_err()
            .code,
        "use_modpack_install"
    );
}

#[tokio::test]
async fn failed_modpack_install_leaves_no_profile_behind() {
    let (base, _s) = serve(pack_server(404)).await;
    let f = fx_host(None, |o| o.modrinth_api = base.clone());
    let before = f.svc.app_state().unwrap().profiles.len();
    let err = f
        .svc
        .modrinth_install_modpack("pack".into(), None, None)
        .await
        .unwrap_err();
    assert!(!err.code.is_empty());
    assert_eq!(f.svc.app_state().unwrap().profiles.len(), before);
    let dirs = std::fs::read_dir(f.paths().instances()).unwrap().count();
    assert_eq!(dirs, before, "the half-made instance folder is removed");
}

#[tokio::test]
async fn install_rejects_bad_ids() {
    let f = fx();
    for (p, v) in [("a/b", None), ("ok", Some("../x"))] {
        let e = f
            .svc
            .modrinth_install(f.pid.clone(), p.into(), v.map(Into::into))
            .await
            .unwrap_err();
        assert_eq!(e.code, "validation");
    }
    assert_eq!(
        f.svc
            .modrinth_install("nope".into(), "p1".into(), None)
            .await
            .unwrap_err()
            .code,
        "invalid_profile_id"
    );
}

// ---------- remote images ----------

#[tokio::test]
async fn remote_icons_are_fetched_once_from_allowed_hosts_only() {
    let hits = Arc::new(AtomicUsize::new(0));
    let h2 = hits.clone();
    let (base, _s) = serve(move |_| {
        Arc::new(move |req: &Req| {
            h2.fetch_add(1, Ordering::SeqCst);
            match req.path.as_str() {
                "/icon.png" => (200, b"\x89PNG\r\n\x1a\nrest".to_vec()),
                "/page.html" => (200, b"<html>".to_vec()),
                _ => (404, Vec::new()),
            }
        })
    })
    .await;
    let f = fx_host(None, |o| {
        o.icon_hosts = vec!["127.0.0.1".into()];
        o.icon_allow_http = true;
    });
    let url = format!("{base}/icon.png");
    let m = image_protocol::remote_url(&url);
    let Ok(Image::File { path, mime }) = f.svc.image(&m).await else {
        panic!("icon should be fetched and served");
    };
    assert_eq!(mime, "image/png");
    assert!(std::fs::read(&path).unwrap().starts_with(b"\x89PNG"));
    assert!(path.starts_with(std::fs::canonicalize(f.paths().cache().join("mod-icons")).unwrap()));
    // Cached: no second request.
    f.svc.image(&m).await.unwrap();
    assert_eq!(hits.load(Ordering::SeqCst), 1);
    // Not an image, 404, and a host that is not on the list.
    for bad in [format!("{base}/page.html"), format!("{base}/missing.png")] {
        assert_eq!(
            f.svc
                .image(&image_protocol::remote_url(&bad))
                .await
                .unwrap_err(),
            ImageError::NotFound
        );
    }
    let strict = fx();
    assert_eq!(
        strict.svc.image(&m).await.unwrap_err(),
        ImageError::NotFound,
        "default allow-list is cdn.modrinth.com over https"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 3);
}

// ---------- servers ----------

fn tiny_png() -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, 1, 1);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .unwrap()
        .write_image_data(&[1, 2, 3, 255])
        .unwrap();
    out
}

fn b64(b: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(b)
}

fn compound(fields: &[(&str, &str)]) -> nbt::Value {
    nbt::Value::Compound(
        fields
            .iter()
            .map(|(k, v)| nbt::Entry::string(*k, *v))
            .collect(),
    )
}

async fn write_servers_dat(dir: &Path, items: Vec<nbt::Value>) {
    let root = nbt::Root {
        name: String::new(),
        entries: vec![nbt::Entry::new(
            "servers",
            nbt::Value::List {
                element_type: nbt::TAG_COMPOUND,
                items,
            },
        )],
    };
    mango_servers::write_server_list(&dir.join("servers.dat"), &root)
        .await
        .unwrap();
}

#[tokio::test]
async fn servers_list_puts_partners_first_and_dedupes() {
    let png = tiny_png();
    let f = fx_host(None, |o| {
        o.partners = vec![mango_servers::FeaturedServer {
            id: "partner".into(),
            name: "Partner".into(),
            address: "Play.Partner.net".into(),
        }]
    });
    // The durable store knows the partner's favicon.
    f.svc.inner.icons.remember(
        "play.partner.net",
        &format!("data:image/png;base64,{}", b64(&png)),
    );
    write_servers_dat(
        &f.dir(),
        vec![
            compound(&[("name", "§6★ §rPartner"), ("ip", "play.partner.net:25565")]),
            compound(&[
                ("name", "§aMy §lServer"),
                ("ip", "mc.example.com"),
                ("icon", &b64(&png)),
            ]),
            compound(&[("name", "Dup"), ("ip", "MC.example.com.")]),
            compound(&[("ip", "bare.example.com:25570")]),
            compound(&[("name", "No address")]),
        ],
    )
    .await;

    let list = f.svc.servers_list().await.unwrap();
    let ids: Vec<_> = list.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "partner",
            "user:mc.example.com",
            "user:bare.example.com:25570"
        ]
    );
    assert!(list[0].partner && list[0].starred);
    assert!(!list[1].partner && !list[1].starred);
    assert_eq!(list[0].name, "Partner");
    assert_eq!(list[1].name, "My Server");
    assert_eq!(list[2].name, "bare.example.com:25570");
    assert_eq!(list[1].address, "mc.example.com");
    // Icons are mangoimg files holding the PNG; servers without one have none.
    for i in [0, 1] {
        let url = list[i].icon_url.as_deref().unwrap();
        let Ok(Image::File { path, .. }) = image_protocol::resolve(f.paths(), url) else {
            panic!("icon must resolve: {url}");
        };
        assert_eq!(std::fs::read(path).unwrap(), png);
    }
    assert!(list[2].icon_url.is_none());
    let v = serde_json::to_value(&list[0]).unwrap();
    for k in ["id", "name", "address", "partner", "starred", "iconUrl"] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
    // The player's file was only read.
    let dat = std::fs::read(f.dir().join("servers.dat")).unwrap();
    let again = f.svc.servers_list().await.unwrap();
    assert_eq!(again, list);
    assert_eq!(std::fs::read(f.dir().join("servers.dat")).unwrap(), dat);
}

#[tokio::test]
async fn servers_list_without_a_servers_dat_is_just_the_partners() {
    let f = fx_host(None, |o| {
        o.partners = vec![mango_servers::FeaturedServer {
            id: "p".into(),
            name: "P".into(),
            address: "p.example.com".into(),
        }]
    });
    let list = f.svc.servers_list().await.unwrap();
    assert_eq!(list.len(), 1);
    assert!(list[0].icon_url.is_none());
    assert!(!f.dir().join("servers.dat").exists());
    // Corrupt file: still the partners, no error.
    std::fs::write(f.dir().join("servers.dat"), b"garbage").unwrap();
    assert_eq!(f.svc.servers_list().await.unwrap().len(), 1);
}

async fn slp_server(motd: &'static str) -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    tokio::spawn(async move {
        loop {
            let Ok((mut s, _)) = listener.accept().await else {
                return;
            };
            tokio::spawn(async move {
                async fn varint(s: &mut TcpStream) -> usize {
                    let (mut n, mut shift) = (0usize, 0);
                    loop {
                        let b = s.read_u8().await.unwrap();
                        n |= usize::from(b & 0x7f) << shift;
                        if b & 0x80 == 0 {
                            return n;
                        }
                        shift += 7;
                    }
                }
                for _ in 0..2 {
                    let len = varint(&mut s).await;
                    let mut body = vec![0u8; len];
                    s.read_exact(&mut body).await.unwrap();
                }
                let json = format!(
                    r#"{{"version":{{"name":"1.21","protocol":767}},"players":{{"online":3,"max":20}},"description":{{"text":"{motd}"}}}}"#
                );
                let mut body = Vec::new();
                mango_servers::ping::write_varint(&mut body, 0);
                mango_servers::ping::write_varint(&mut body, json.len() as i32);
                body.extend_from_slice(json.as_bytes());
                let mut out = Vec::new();
                mango_servers::ping::write_varint(&mut out, body.len() as i32);
                out.extend_from_slice(&body);
                let _ = s.write_all(&out).await;
            });
        }
    });
    port
}

#[tokio::test]
async fn servers_ping_reports_status_and_offline() {
    let port = slp_server("§6Hello").await;
    let f = fx_host(None, |o| o.ping.timeout = Duration::from_secs(3));
    let addr = format!("127.0.0.1:{port}");
    let s = f.svc.servers_ping(addr.clone()).await.unwrap();
    assert_eq!(s.id, format!("user:{addr}"));
    assert!(s.online);
    assert_eq!(s.motd, "Hello");
    assert_eq!(s.players, Some(ServerPlayers { online: 3, max: 20 }));
    assert_eq!(s.version.as_deref(), Some("1.21"));
    assert!(s.ping.is_some());
    let v = serde_json::to_value(&s).unwrap();
    assert_eq!(v["players"], json!({"online": 3, "max": 20}));

    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let off = f
        .svc
        .servers_ping(format!("127.0.0.1:{dead}"))
        .await
        .unwrap();
    assert!(!off.online && off.ping.is_none() && off.players.is_none() && off.version.is_none());
    for bad in ["", "   ", "a b", "x\ny", &"h".repeat(300)] {
        assert_eq!(
            f.svc.servers_ping(bad.into()).await.unwrap_err().code,
            "validation"
        );
    }
}

#[tokio::test]
async fn servers_ping_all_streams_results_as_events() {
    let (pa, pb) = (slp_server("A").await, slp_server("B").await);
    let f = fx_host(None, |o| {
        o.ping.timeout = Duration::from_secs(3);
        o.partners = vec![mango_servers::FeaturedServer {
            id: "pa".into(),
            name: "A".into(),
            address: format!("127.0.0.1:{pa}"),
        }];
    });
    write_servers_dat(
        &f.dir(),
        vec![compound(&[
            ("name", "B"),
            ("ip", &format!("127.0.0.1:{pb}")),
        ])],
    )
    .await;
    f.svc.servers_ping_all().await.unwrap();
    let sink = f.sink.clone();
    wait_for(|| {
        sink.events()
            .iter()
            .filter(|e| matches!(e, Event::ServersPinged(_)))
            .count()
            == 2
    })
    .await;
    let mut got: Vec<_> = f
        .sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::ServersPinged(s) => Some((s.id, s.motd, s.online)),
            _ => None,
        })
        .collect();
    got.sort();
    assert_eq!(
        got,
        vec![
            ("pa".to_string(), "A".to_string(), true),
            (format!("user:127.0.0.1:{pb}"), "B".to_string(), true),
        ]
    );
}

// ---------- screenshots ----------

#[derive(Default)]
struct Recorder {
    revealed: Mutex<Vec<PathBuf>>,
    copied: Mutex<Vec<PathBuf>>,
    fail_copy: bool,
}

impl OpenFolder for Recorder {
    fn open(&self, _dir: &Path) -> std::io::Result<()> {
        Ok(())
    }
    fn reveal(&self, file: &Path) -> std::io::Result<()> {
        self.revealed.lock().unwrap().push(file.to_path_buf());
        Ok(())
    }
}

impl ClipboardImage for Recorder {
    fn copy_image(&self, file: &Path) -> Result<(), String> {
        if self.fail_copy {
            return Err("clipboard busy".into());
        }
        self.copied.lock().unwrap().push(file.to_path_buf());
        Ok(())
    }
}

fn set_mtime(p: &Path, secs_ago: u64) {
    let t = SystemTime::now() - Duration::from_secs(secs_ago);
    std::fs::File::options()
        .write(true)
        .open(p)
        .unwrap()
        .set_modified(t)
        .unwrap();
}

#[tokio::test]
async fn screenshots_list_and_actions() {
    let rec = Arc::new(Recorder::default());
    let host = Host {
        opener: rec.clone(),
        clipboard: rec.clone(),
        ..Host::system(false)
    };
    let f = fx_host(Some(host), |_| {});
    assert!(f
        .svc
        .screenshots_list(f.pid.clone())
        .await
        .unwrap()
        .is_empty());
    let dir = f.dir().join("screenshots");
    std::fs::create_dir_all(dir.join("sub.png")).unwrap();
    for (name, ago) in [("old.png", 500), ("new.PNG", 10), ("mid.jpg", 100)] {
        std::fs::write(dir.join(name), name.as_bytes()).unwrap();
        set_mtime(&dir.join(name), ago);
    }
    std::fs::write(dir.join("notes.txt"), b"x").unwrap();
    let list = f.svc.screenshots_list(f.pid.clone()).await.unwrap();
    let names: Vec<_> = list.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, ["new.PNG", "mid.jpg", "old.png"]);
    assert_eq!(list[0].size, 7);
    assert!(list[0].taken > list[2].taken);
    let Ok(Image::File { path, .. }) = image_protocol::resolve(f.paths(), &list[0].url) else {
        panic!("screenshot url must resolve");
    };
    assert_eq!(std::fs::read(path).unwrap(), b"new.PNG");
    let v = serde_json::to_value(&list[0]).unwrap();
    assert!(v["size"].is_number() && v["taken"].is_number() && v["url"].is_string());

    f.svc
        .screenshot_action(f.pid.clone(), "mid.jpg".into(), ScreenshotAction::Reveal)
        .await
        .unwrap();
    f.svc
        .screenshot_action(f.pid.clone(), "mid.jpg".into(), ScreenshotAction::Copy)
        .await
        .unwrap();
    assert_eq!(
        rec.revealed.lock().unwrap().as_slice(),
        [dir.join("mid.jpg")]
    );
    assert_eq!(rec.copied.lock().unwrap().as_slice(), [dir.join("mid.jpg")]);
    f.svc
        .screenshot_action(f.pid.clone(), "old.png".into(), ScreenshotAction::Delete)
        .await
        .unwrap();
    assert!(!dir.join("old.png").exists());
    assert_eq!(
        f.svc.screenshots_list(f.pid.clone()).await.unwrap().len(),
        2
    );

    for bad in [
        "../old.png",
        "a/b.png",
        "..",
        "",
        "x.exe",
        "notes.txt",
        "sub\\x.png",
    ] {
        let e = f
            .svc
            .screenshot_action(f.pid.clone(), bad.into(), ScreenshotAction::Delete)
            .await
            .unwrap_err();
        assert_eq!(e.code, "invalid_name", "{bad}");
    }
    for name in ["gone.png", "sub.png"] {
        let e = f
            .svc
            .screenshot_action(f.pid.clone(), name.into(), ScreenshotAction::Delete)
            .await
            .unwrap_err();
        assert_eq!(e.code, "not_found", "{name}");
    }
    assert!(dir.join("sub.png").is_dir());
    assert_eq!(
        f.svc
            .screenshot_action("nope".into(), "a.png".into(), ScreenshotAction::Delete)
            .await
            .unwrap_err()
            .code,
        "invalid_profile_id"
    );
}

#[tokio::test]
async fn copy_failures_surface_as_errors_and_default_host_has_no_clipboard() {
    let rec = Arc::new(Recorder {
        fail_copy: true,
        ..Recorder::default()
    });
    let f = fx_host(
        Some(Host {
            opener: rec.clone(),
            clipboard: rec,
            ..Host::system(false)
        }),
        |_| {},
    );
    let dir = f.dir().join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.png"), b"x").unwrap();
    let e = f
        .svc
        .screenshot_action(f.pid.clone(), "a.png".into(), ScreenshotAction::Copy)
        .await
        .unwrap_err();
    assert_eq!(
        (e.code.as_str(), e.message.as_str()),
        ("clipboard", "clipboard busy")
    );

    let plain = fx();
    let dir = plain.dir().join("screenshots");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.png"), b"x").unwrap();
    assert_eq!(
        plain
            .svc
            .screenshot_action(plain.pid.clone(), "a.png".into(), ScreenshotAction::Copy)
            .await
            .unwrap_err()
            .code,
        "clipboard"
    );
    // Reveal through the (disabled) system opener is a harmless no-op.
    plain
        .svc
        .screenshot_action(plain.pid.clone(), "a.png".into(), ScreenshotAction::Reveal)
        .await
        .unwrap();
    let a: ScreenshotAction = serde_json::from_value(json!("copy")).unwrap();
    assert_eq!(a, ScreenshotAction::Copy);
    assert!(serde_json::from_value::<ScreenshotAction>(json!("zip")).is_err());
}

// ---------- logs ----------

fn gz(text: &str) -> Vec<u8> {
    let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    e.write_all(text.as_bytes()).unwrap();
    e.finish().unwrap()
}

fn log_fixture(f: &Fx) -> PathBuf {
    let dir = f.dir();
    std::fs::create_dir_all(dir.join("logs")).unwrap();
    std::fs::create_dir_all(dir.join("crash-reports")).unwrap();
    std::fs::write(dir.join("logs/latest.log"), "latest text\n").unwrap();
    set_mtime(&dir.join("logs/latest.log"), 1000);
    std::fs::write(dir.join("logs/2024-01-01-1.log.gz"), gz("archived text\n")).unwrap();
    set_mtime(&dir.join("logs/2024-01-01-1.log.gz"), 10);
    std::fs::write(dir.join("logs/debug.md"), "ignored").unwrap();
    std::fs::write(dir.join("crash-reports/crash-1.txt"), "boom").unwrap();
    set_mtime(&dir.join("crash-reports/crash-1.txt"), 20);
    let name = f.svc.app_state().unwrap().profiles[1].name.clone();
    let launcher = mango_runtime::log_file_for(&f.paths().logs(), &name, &f.pid);
    std::fs::create_dir_all(launcher.parent().unwrap()).unwrap();
    std::fs::write(&launcher, "launcher text\n").unwrap();
    set_mtime(&launcher, 5);
    launcher
}

#[tokio::test]
async fn logs_list_read_and_delete() {
    let f = fx();
    assert!(f.svc.logs_list(f.pid.clone()).await.unwrap().is_empty());
    let launcher = log_fixture(&f);
    let list = f.svc.logs_list(f.pid.clone()).await.unwrap();
    let ids: Vec<_> = list.iter().map(|l| l.id.clone()).collect();
    let launcher_id = format!(
        "launcher:{}",
        launcher.file_name().unwrap().to_string_lossy()
    );
    assert_eq!(
        ids,
        [
            "game:latest.log".to_string(),
            launcher_id.clone(),
            "game:2024-01-01-1.log.gz".to_string(),
            "crash:crash-1.txt".to_string(),
        ],
        "latest.log leads, the rest is newest first"
    );
    assert_eq!(list[0].kind, LogKind::Game);
    assert_eq!(list[1].kind, LogKind::Launcher);
    assert_eq!(list[3].kind, LogKind::Crash);
    assert_eq!(list[3].size, 4);
    let v = serde_json::to_value(&list[3]).unwrap();
    assert_eq!(v["kind"], "crash");
    assert!(v["modified"].is_number());

    let r = f
        .svc
        .logs_read(f.pid.clone(), "game:latest.log".into())
        .await
        .unwrap();
    assert_eq!((r.text.as_str(), r.truncated), ("latest text\n", false));
    let r = f
        .svc
        .logs_read(f.pid.clone(), "game:2024-01-01-1.log.gz".into())
        .await
        .unwrap();
    assert_eq!(r.text, "archived text\n");
    let r = f
        .svc
        .logs_read(f.pid.clone(), launcher_id.clone())
        .await
        .unwrap();
    assert_eq!(r.text, "launcher text\n");
    assert_eq!(
        serde_json::to_value(&r).unwrap(),
        json!({"text": "launcher text\n", "truncated": false})
    );

    // latest.log can never be deleted; others can.
    assert_eq!(
        f.svc
            .logs_delete(f.pid.clone(), "game:latest.log".into())
            .await
            .unwrap_err()
            .code,
        "log_protected"
    );
    f.svc
        .logs_delete(f.pid.clone(), "crash:crash-1.txt".into())
        .await
        .unwrap();
    f.svc.logs_delete(f.pid.clone(), launcher_id).await.unwrap();
    assert_eq!(
        f.svc
            .logs_delete(f.pid.clone(), "crash:crash-1.txt".into())
            .await
            .unwrap_err()
            .code,
        "not_found"
    );
    assert_eq!(f.svc.logs_list(f.pid.clone()).await.unwrap().len(), 2);
}

#[tokio::test]
async fn log_ids_are_basename_only_and_kind_checked() {
    let f = fx();
    log_fixture(&f);
    for bad in [
        "bad",
        "",
        ":x.log",
        "game:",
        "game:../latest.log",
        "game:../../x.log",
        "game:sub/latest.log",
        "game:sub\\latest.log",
        "game:.",
        "game:..",
        "game:x.exe",
        "crash:latest.log.png",
        "launcher:other.log",
        "mods:latest.log",
        "GAME:latest.log",
    ] {
        for res in [
            f.svc.logs_read(f.pid.clone(), bad.into()).await.map(|_| ()),
            f.svc.logs_delete(f.pid.clone(), bad.into()).await,
            f.svc
                .logs_upload(f.pid.clone(), bad.into())
                .await
                .map(|_| ()),
        ] {
            assert_eq!(res.unwrap_err().code, "unknown_log", "{bad}");
        }
    }
    assert!(f.dir().join("logs/latest.log").exists());
    assert_eq!(
        f.svc
            .logs_read(f.pid.clone(), "game:missing.log".into())
            .await
            .unwrap_err()
            .code,
        "io"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn symlinked_logs_are_ignored() {
    let f = fx();
    log_fixture(&f);
    let secret = f.tmp.path().join("secret.log");
    std::fs::write(&secret, "secret").unwrap();
    std::os::unix::fs::symlink(&secret, f.dir().join("logs/link.log")).unwrap();
    let list = f.svc.logs_list(f.pid.clone()).await.unwrap();
    assert!(!list.iter().any(|l| l.name == "link.log"));
    assert!(f
        .svc
        .logs_read(f.pid.clone(), "game:link.log".into())
        .await
        .is_err());
    assert!(f
        .svc
        .logs_delete(f.pid.clone(), "game:link.log".into())
        .await
        .is_err());
    assert!(secret.exists());
}

#[test]
fn read_tail_keeps_the_end_of_plain_and_gzip_logs() {
    let tmp = tempfile::tempdir().unwrap();
    let text: String = (0..1000).map(|i| format!("line {i}\n")).collect();
    let plain = tmp.path().join("a.log");
    std::fs::write(&plain, &text).unwrap();
    let r = crate::media::read_tail(&plain, 100).unwrap();
    assert!(r.truncated && r.text.len() == 100 && r.text.ends_with("line 999\n"));
    assert!(text.ends_with(&r.text));
    let r = crate::media::read_tail(&plain, 1_000_000).unwrap();
    assert!(!r.truncated && r.text == text);

    let packed = tmp.path().join("a.log.gz");
    std::fs::write(&packed, gz(&text)).unwrap();
    let r = crate::media::read_tail(&packed, 100).unwrap();
    assert!(r.truncated && r.text.len() == 100 && r.text.ends_with("line 999\n"));
    let r = crate::media::read_tail(&packed, 1_000_000).unwrap();
    assert!(!r.truncated && r.text == text);
    // Exactly at the limit is not truncated.
    let r = crate::media::read_tail(&plain, text.len()).unwrap();
    assert!(!r.truncated);
    // Corrupt gzip fails cleanly; an invalid UTF-8 cut is replaced, not fatal.
    std::fs::write(tmp.path().join("bad.log.gz"), b"not gzip").unwrap();
    assert!(crate::media::read_tail(&tmp.path().join("bad.log.gz"), 10).is_err());
    std::fs::write(tmp.path().join("u.log"), "ééé".as_bytes()).unwrap();
    let r = crate::media::read_tail(&tmp.path().join("u.log"), 3).unwrap();
    assert!(r.truncated && r.text.contains('\u{fffd}'));
}

#[tokio::test]
async fn big_logs_are_shown_from_their_end() {
    let f = fx();
    std::fs::create_dir_all(f.dir().join("logs")).unwrap();
    let mut big = vec![b'a'; crate::media::MAX_LOG_BYTES];
    big.extend_from_slice(b"TAIL");
    std::fs::write(f.dir().join("logs/latest.log"), &big).unwrap();
    let r = f
        .svc
        .logs_read(f.pid.clone(), "game:latest.log".into())
        .await
        .unwrap();
    assert!(r.truncated);
    assert_eq!(r.text.len(), crate::media::MAX_LOG_BYTES);
    assert!(r.text.ends_with("TAIL"));
}

fn decode_form_content(body: &[u8]) -> String {
    let s = String::from_utf8(body.to_vec()).unwrap();
    pct_decode(s.strip_prefix("content=").unwrap())
}

#[tokio::test]
async fn logs_upload_posts_the_last_25000_lines_to_mclogs() {
    let (base, seen) = serve(|_| {
        Arc::new(|req: &Req| {
            assert_eq!(req.method, "POST");
            json_ok(json!({"success": true, "url": "https://mclo.gs/abc", "id": "abc"}))
        })
    })
    .await;
    let f = fx_host(None, |o| o.mclogs_api = format!("{base}/1/log"));
    std::fs::create_dir_all(f.dir().join("logs")).unwrap();
    let text: String = (0..25_010).map(|i| format!("line {i} é&=+\n")).collect();
    std::fs::write(f.dir().join("logs/latest.log"), &text).unwrap();
    let up = f
        .svc
        .logs_upload(f.pid.clone(), "game:latest.log".into())
        .await
        .unwrap();
    assert_eq!(up.url, "https://mclo.gs/abc");
    assert_eq!(
        serde_json::to_value(&up).unwrap(),
        json!({"url": "https://mclo.gs/abc"})
    );
    let req = seen.lock().unwrap()[0].clone();
    assert_eq!(req.path, "/1/log");
    let body = decode_form_content(&req.body);
    assert_eq!(body.lines().count(), 25_000);
    assert!(body.starts_with("line 10 é&=+"));
    assert!(body.ends_with("line 25009 é&=+"));

    // A short log is sent whole.
    std::fs::write(f.dir().join("logs/latest.log"), "a\nb\n").unwrap();
    f.svc
        .logs_upload(f.pid.clone(), "game:latest.log".into())
        .await
        .unwrap();
    assert_eq!(decode_form_content(&seen.lock().unwrap()[1].body), "a\nb\n");
}

#[tokio::test]
async fn logs_upload_scrubs_secrets_and_paths_before_sending() {
    let (base, seen) = serve(|_| {
        Arc::new(|_: &Req| json_ok(json!({"success": true, "url": "https://mclo.gs/x", "id": "x"})))
    })
    .await;
    let f = fx_host(None, |o| o.mclogs_api = format!("{base}/1/log"));
    std::fs::create_dir_all(f.dir().join("logs")).unwrap();
    let token = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0.abcdefghijklmnop";
    let log = format!(
        "[main/INFO]: Setting user: Steve\n\
         [main/INFO]: --accessToken SUPERSECRETTOKEN123\n\
         Authorization: Bearer {token}\n\
         Loading from /home/someuser/.mangoclient/instances/x\n\
         contact me@example.com\n"
    );
    std::fs::write(f.dir().join("logs/latest.log"), log).unwrap();
    f.svc
        .logs_upload(f.pid.clone(), "game:latest.log".into())
        .await
        .unwrap();
    let body = decode_form_content(&seen.lock().unwrap()[0].body);
    for leak in [
        "SUPERSECRETTOKEN123",
        token,
        "/home/someuser",
        "me@example.com",
    ] {
        assert!(!body.contains(leak), "leaked {leak}: {body}");
    }
    assert!(body.contains("Setting user"), "ordinary lines stay");
}

#[tokio::test]
async fn logs_upload_reports_refusals_and_unusable_answers() {
    for (status, body, expect) in [
        (
            400,
            json!({"success": false, "error": "Log too large"}),
            "Log too large",
        ),
        (
            500,
            json!({"nope": 1}),
            "mclo.gs refused the upload (HTTP 500)",
        ),
        (
            200,
            json!({"success": true, "url": "javascript:alert(1)"}),
            "mclo.gs returned no usable link",
        ),
        (
            200,
            json!({"success": true}),
            "mclo.gs returned no usable link",
        ),
    ] {
        let (base, _s) = serve(move |_| {
            let body = body.clone();
            Arc::new(move |_: &Req| (status, serde_json::to_vec(&body).unwrap()))
        })
        .await;
        let f = fx_host(None, |o| o.mclogs_api = format!("{base}/1/log"));
        std::fs::create_dir_all(f.dir().join("logs")).unwrap();
        std::fs::write(f.dir().join("logs/latest.log"), "x").unwrap();
        let e = f
            .svc
            .logs_upload(f.pid.clone(), "game:latest.log".into())
            .await
            .unwrap_err();
        assert_eq!(
            (e.code.as_str(), e.message.as_str()),
            ("upload_failed", expect)
        );
    }
    // Unreachable endpoint.
    let dead = {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        l.local_addr().unwrap().port()
    };
    let f = fx_host(None, |o| {
        o.mclogs_api = format!("http://127.0.0.1:{dead}/1/log")
    });
    std::fs::create_dir_all(f.dir().join("logs")).unwrap();
    std::fs::write(f.dir().join("logs/latest.log"), "x").unwrap();
    assert_eq!(
        f.svc
            .logs_upload(f.pid.clone(), "game:latest.log".into())
            .await
            .unwrap_err()
            .code,
        "net"
    );
}

// ---------- stats ----------

#[test]
fn stats_summary_counts_the_window() {
    let f = fx();
    let other = f
        .svc
        .profile_create("Other".into(), "1.21.1".into(), "vanilla".into(), None)
        .unwrap();
    let stats = mango_mango::stats::Stats::new(f.paths());
    stats.record_ending_now(&f.pid, 120_000).unwrap();
    stats.record_ending_now(&f.pid, 120_000).unwrap();
    stats.record_ending_now(&other.id, 60_000).unwrap();
    stats.record_ending_now(&other.id, 5_000).unwrap(); // too short: ignored
    let old = chrono::Local::now().timestamp_millis() - 40 * 24 * 3600 * 1000;
    stats.record(&f.pid, 999_000, old).unwrap();

    let s = f.svc.stats_summary(7).unwrap();
    assert_eq!(s.days.len(), 7);
    assert_eq!(s.days.last().unwrap().ms, 300_000);
    assert_eq!(s.days[0].ms, 0);
    assert_eq!(
        (s.total_ms, s.sessions, s.longest_ms),
        (300_000, 3, 120_000)
    );
    let mine = s
        .per_profile
        .iter()
        .find(|p| p.profile_id == f.pid)
        .unwrap();
    assert_eq!(
        (mine.ms, mine.sessions, mine.name.as_str()),
        (240_000, 2, "T")
    );
    let theirs = s
        .per_profile
        .iter()
        .find(|p| p.profile_id == other.id)
        .unwrap();
    assert_eq!((theirs.ms, theirs.sessions), (60_000, 1));
    assert_eq!(
        s.per_profile.len(),
        2,
        "profiles without play in the window are left out"
    );
    let v = serde_json::to_value(&s).unwrap();
    for k in ["days", "perProfile", "totalMs", "sessions", "longestMs"] {
        assert!(v.get(k).is_some(), "missing {k}");
    }
    assert!(v["perProfile"][0].get("profileId").is_some());

    let wide = f.svc.stats_summary(60).unwrap();
    assert_eq!(
        (wide.days.len(), wide.sessions, wide.longest_ms),
        (60, 4, 999_000)
    );
    assert_eq!(f.svc.stats_summary(0).unwrap().days.len(), 1);
    assert_eq!(f.svc.stats_summary(100_000).unwrap().days.len(), 366);
}

#[test]
fn stats_without_a_journal_are_empty() {
    let f = fx();
    let s = f.svc.stats_summary(30).unwrap();
    assert_eq!(
        (s.days.len(), s.total_ms, s.sessions, s.longest_ms),
        (30, 0, 0, 0)
    );
    assert!(s.per_profile.is_empty());
}

// ---------- storage ----------

#[tokio::test]
async fn storage_usage_and_clear() {
    let f = fx();
    let p = f.paths();
    std::fs::create_dir_all(p.versions().join("1.21")).unwrap();
    std::fs::write(p.versions().join("1.21/a.jar"), [0u8; 10]).unwrap();
    std::fs::write(p.cache().join("c.bin"), [0u8; 5]).unwrap();
    std::fs::write(f.dir().join("world.dat"), [0u8; 7]).unwrap();
    let u = f.svc.storage_usage().await.unwrap();
    let keys: Vec<_> = u.iter().map(|x| x.key.as_str()).collect();
    assert_eq!(
        keys,
        [
            "instances",
            "versions",
            "libraries",
            "assets",
            "runtimes",
            "natives",
            "cache",
            "logs"
        ]
    );
    let get = |k: StorageKey| u.iter().find(|x| x.key == k).unwrap();
    assert_eq!(get(StorageKey::Versions).bytes, 10);
    assert_eq!(get(StorageKey::Cache).bytes, 5);
    assert!(get(StorageKey::Instances).bytes >= 7);
    assert!(!get(StorageKey::Instances).clearable);
    assert!(u
        .iter()
        .filter(|x| x.key != StorageKey::Instances)
        .all(|x| x.clearable));
    let v = serde_json::to_value(get(StorageKey::Cache)).unwrap();
    assert_eq!(v, json!({"key": "cache", "bytes": 5, "clearable": true}));

    let after = f.svc.storage_clear(StorageKey::Versions).await.unwrap();
    assert_eq!(
        after
            .iter()
            .find(|x| x.key == StorageKey::Versions)
            .unwrap()
            .bytes,
        0
    );
    assert!(p.versions().is_dir() && !p.versions().join("1.21").exists());
    assert_eq!(
        after
            .iter()
            .find(|x| x.key == StorageKey::Cache)
            .unwrap()
            .bytes,
        5
    );

    // Worlds are never cleared.
    assert_eq!(
        f.svc
            .storage_clear(StorageKey::Instances)
            .await
            .unwrap_err()
            .code,
        "not_clearable"
    );
    assert!(f.dir().join("world.dat").exists());

    // Refused while a game launches or runs.
    lock(&f.svc.inner.launching).insert(f.pid.clone());
    assert_eq!(
        f.svc
            .storage_clear(StorageKey::Cache)
            .await
            .unwrap_err()
            .code,
        "game_running"
    );
    assert!(p.cache().join("c.bin").exists());
    lock(&f.svc.inner.launching).clear();
    f.svc.storage_clear(StorageKey::Cache).await.unwrap();
    assert!(!p.cache().join("c.bin").exists());

    assert!(serde_json::from_value::<StorageKey>(json!("worlds")).is_err());
    assert_eq!(
        serde_json::from_value::<StorageKey>(json!("logs")).unwrap(),
        StorageKey::Logs
    );
}

#[cfg(unix)]
#[tokio::test]
async fn storage_does_not_follow_symlinks() {
    let f = fx();
    let big = f.tmp.path().join("elsewhere");
    std::fs::create_dir_all(&big).unwrap();
    std::fs::write(big.join("huge.bin"), vec![0u8; 5000]).unwrap();
    std::os::unix::fs::symlink(&big, f.paths().cache().join("link")).unwrap();
    let u = f.svc.storage_usage().await.unwrap();
    assert!(u.iter().find(|x| x.key == StorageKey::Cache).unwrap().bytes < 1000);
    f.svc.storage_clear(StorageKey::Cache).await.unwrap();
    assert!(
        big.join("huge.bin").exists(),
        "clearing removes the link, not its target"
    );
}

// ---------- updater, telemetry, watcher ----------

#[tokio::test]
async fn update_commands_and_status_events() {
    let f = fx();
    assert_eq!(f.svc.update_status().state, "idle");
    assert!(!f.svc.update_install().unwrap());
    f.svc.config_set(json!({"telemetry": false})).unwrap();
    f.svc.start_background();
    let st = f.svc.update_check().await.unwrap();
    assert_eq!((st.state, st.disabled_reason), ("disabled", Some("dev")));
    assert_eq!(f.svc.update_status(), st);
    let sink = f.sink.clone();
    wait_for(|| {
        sink.events()
            .iter()
            .any(|e| matches!(e, Event::UpdateStatus(s) if s.state == "disabled"))
    })
    .await;

    // Not a dev run, but a plain (non-AppImage) Linux install cannot update itself.
    let g = fx_host(None, |o| o.update_dev = false);
    let st = g.svc.update_check().await.unwrap();
    if cfg!(target_os = "linux") && std::env::var_os("APPIMAGE").is_none() {
        assert_eq!(
            (st.state, st.disabled_reason),
            ("disabled", Some("unsupported"))
        );
    }
}

#[tokio::test]
async fn telemetry_starts_only_when_opted_in_and_sends_exactly_three_fields() {
    let (base, seen) = serve(|_| Arc::new(|_: &Req| (200, b"{}".to_vec()))).await;
    let f = fx_host(None, |o| {
        o.api_base = Some(base.clone());
        o.app_version = "9.9.9".into();
    });
    f.svc.config_set(json!({"telemetry": false})).unwrap();
    f.svc.start_background();
    assert!(!f.svc.telemetry_running());
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(
        seen.lock().unwrap().is_empty(),
        "opted out: nothing is sent"
    );

    f.svc.config_set(json!({"telemetry": true})).unwrap();
    assert!(f.svc.telemetry_running());
    let s = seen.clone();
    wait_for(|| !s.lock().unwrap().is_empty()).await;
    let req = seen.lock().unwrap()[0].clone();
    assert_eq!(
        (req.method.as_str(), req.path.as_str()),
        ("POST", "/v1/launcher")
    );
    let body: Value = serde_json::from_slice(&req.body).unwrap();
    let mut keys: Vec<_> = body.as_object().unwrap().keys().cloned().collect();
    keys.sort();
    assert_eq!(keys, ["id", "os", "version"]);
    assert_eq!(body["version"], "9.9.9");
    assert!(!body["id"].as_str().unwrap().is_empty());

    f.svc.config_set(json!({"telemetry": false})).unwrap();
    assert!(!f.svc.telemetry_running());
}

#[test]
fn telemetry_defaults_follow_the_config_without_a_runtime() {
    // No runtime: sync is a no-op instead of a panic.
    let f = fx();
    f.svc.config_set(json!({"telemetry": true})).unwrap();
    assert!(!f.svc.telemetry_running());
}

#[tokio::test]
async fn mods_watcher_reports_changes_after_the_baseline() {
    let f = fx_host(None, |o| o.watch_interval = Duration::from_millis(25));
    f.svc.config_set(json!({"telemetry": false})).unwrap();
    f.svc.start_background();
    tokio::time::sleep(Duration::from_millis(200)).await;
    let changed = |sink: &MockSink| {
        sink.events()
            .into_iter()
            .filter(|e| matches!(e, Event::ContentChanged(_)))
            .count()
    };
    assert_eq!(changed(&f.sink), 0, "the first look is only a baseline");
    std::fs::write(f.dir().join("mods/new.jar"), b"x").unwrap();
    let sink = f.sink.clone();
    wait_for(|| changed(&sink) >= 1).await;
    let ev = f
        .sink
        .events()
        .into_iter()
        .find_map(|e| match e {
            Event::ContentChanged(c) => Some(c),
            _ => None,
        })
        .unwrap();
    assert_eq!(ev.profile_id, f.pid);
    // Quiet again once nothing changes.
    let n = changed(&f.sink);
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(changed(&f.sink), n);
}

#[test]
fn folder_signature_tracks_names_sizes_and_mtimes() {
    let tmp = tempfile::tempdir().unwrap();
    let sig = |p: &Path| crate::background::folder_signature(p);
    let empty = sig(tmp.path());
    assert_eq!(sig(&tmp.path().join("missing")), empty);
    std::fs::write(tmp.path().join("a.jar"), b"1").unwrap();
    let one = sig(tmp.path());
    assert_ne!(one, empty);
    std::fs::write(tmp.path().join("a.jar"), b"12").unwrap();
    assert_ne!(sig(tmp.path()), one);
}
