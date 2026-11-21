#![allow(clippy::unwrap_used)]

type Captured = (
    InstallOptions,
    Arc<Mutex<Vec<InstallProgress>>>,
    Arc<Mutex<Vec<String>>>,
);

use crate::*;
use mango_core::Paths;
use mango_net::{hash_file, FetchOptions, HashAlgo};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

type Routes = Arc<Mutex<HashMap<String, Vec<u8>>>>;

struct Server {
    base: String,
    routes: Routes,
    hits: Arc<Mutex<Vec<String>>>,
}

impl Server {
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let routes: Routes = Arc::default();
        let hits: Arc<Mutex<Vec<String>>> = Arc::default();
        let (r2, h2) = (routes.clone(), hits.clone());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let (routes, hits) = (r2.clone(), h2.clone());
                tokio::spawn(async move {
                    let mut buf = Vec::new();
                    let mut tmp = [0u8; 1024];
                    while !buf.windows(4).any(|w| w == b"\r\n\r\n") {
                        match sock.read(&mut tmp).await {
                            Ok(0) | Err(_) => return,
                            Ok(n) => buf.extend_from_slice(&tmp[..n]),
                        }
                    }
                    let head = String::from_utf8_lossy(&buf).to_string();
                    let path = head.split_whitespace().nth(1).unwrap_or("/").to_string();
                    hits.lock().unwrap().push(path.clone());
                    let body = routes.lock().unwrap().get(&path).cloned();
                    let (status, body) = match body {
                        Some(b) => (200, b),
                        None => (404, Vec::new()),
                    };
                    let mut out = format!(
                        "HTTP/1.1 {status} X\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    out.extend_from_slice(&body);
                    let _ = sock.write_all(&out).await;
                    let _ = sock.shutdown().await;
                });
            }
        });
        Server { base, routes, hits }
    }

    fn put(&self, path: &str, body: impl Into<Vec<u8>>) {
        self.routes.lock().unwrap().insert(path.into(), body.into());
    }

    fn hit_count(&self, path: &str) -> usize {
        self.hits
            .lock()
            .unwrap()
            .iter()
            .filter(|p| *p == path)
            .count()
    }
}

fn fast() -> FetchOptions {
    FetchOptions {
        retries: 0,
        backoff: Duration::from_millis(1),
        timeout: Duration::from_secs(5),
        ..Default::default()
    }
}

async fn sha1(bytes: &[u8]) -> String {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("x");
    std::fs::write(&f, bytes).unwrap();
    hash_file(&f, HashAlgo::Sha1).await.unwrap()
}

fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut w = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    for (name, data) in files {
        w.start_file(*name, zip::write::SimpleFileOptions::default())
            .unwrap();
        w.write_all(data).unwrap();
    }
    w.finish().unwrap().into_inner()
}

fn linux() -> HostEnv {
    HostEnv::new(HostOs::Linux, HostArch::X64)
}

fn options(srv: &Server) -> Captured {
    let progress: Arc<Mutex<Vec<InstallProgress>>> = Arc::default();
    let logs: Arc<Mutex<Vec<String>>> = Arc::default();
    let (p, l) = (progress.clone(), logs.clone());
    let opts = InstallOptions {
        concurrency: 3,
        fetch: fast(),
        endpoints: Endpoints {
            manifest: format!("{}/manifest.json", srv.base),
            resources: format!("{}/res", srv.base),
        },
        env: linux(),
        on_progress: Some(Arc::new(move |e| p.lock().unwrap().push(e))),
        on_log: Some(Arc::new(move |s| l.lock().unwrap().push(s.to_string()))),
    };
    (opts, progress, logs)
}

#[tokio::test]
async fn manifest_cache_force_and_offline_fallback() {
    let srv = Server::start().await;
    srv.put(
        "/m.json",
        br#"{"versions":[{"id":"1.0","url":"u"}]}"#.to_vec(),
    );
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let url = format!("{}/m.json", srv.base);

    let m = get_version_manifest(&paths, &url, false, &fast())
        .await
        .unwrap();
    assert_eq!(m["versions"][0]["id"], "1.0");
    get_version_manifest(&paths, &url, false, &fast())
        .await
        .unwrap();
    assert_eq!(srv.hit_count("/m.json"), 1, "second call served from cache");
    assert!(paths.cache().join("version_manifest.json").exists());
    get_version_manifest(&paths, &url, true, &fast())
        .await
        .unwrap();
    assert_eq!(srv.hit_count("/m.json"), 2);

    // Offline: unreachable host with a forced refresh falls back to the cache.
    let dead = "http://127.0.0.1:1/m.json";
    let m = get_version_manifest(&paths, dead, true, &fast())
        .await
        .unwrap();
    assert_eq!(m["versions"][0]["id"], "1.0");

    let empty = tempfile::tempdir().unwrap();
    let err = get_version_manifest(&Paths::new(empty.path()), dead, true, &fast())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "minecraft.net");
}

#[tokio::test]
async fn stale_manifest_cache_is_refetched() {
    let srv = Server::start().await;
    srv.put("/m.json", br#"{"versions":[]}"#.to_vec());
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    std::fs::create_dir_all(paths.cache()).unwrap();
    let f = paths.cache().join("version_manifest.json");
    std::fs::write(&f, br#"{"versions":[{"id":"old"}]}"#).unwrap();
    let old = std::time::SystemTime::now() - Duration::from_secs(7 * 3600);
    std::fs::File::options()
        .write(true)
        .open(&f)
        .unwrap()
        .set_modified(old)
        .unwrap();
    let m = get_version_manifest(&paths, &format!("{}/m.json", srv.base), false, &fast())
        .await
        .unwrap();
    assert_eq!(m["versions"], json!([]));
}

struct Fixture {
    version: Value,
}

async fn vanilla_fixture(srv: &Server) -> Fixture {
    let client = b"client-jar-bytes".to_vec();
    srv.put("/client.jar", client.clone());
    let lib = b"lib-bytes".to_vec();
    srv.put("/lib/a.jar", lib.clone());
    let native = zip_of(&[
        ("libfoo.so", b"so"),
        ("META-INF/MANIFEST.MF", b"m"),
        ("readme.txt", b"r"),
        ("sub/libbar.so", b"bar"),
        ("skip/libskip.so", b"x"),
    ]);
    srv.put("/lib/n.jar", native.clone());
    let a1 = b"asset-one".to_vec();
    let a2 = b"asset-two".to_vec();
    let (h1, h2) = (sha1(&a1).await, sha1(&a2).await);
    srv.put(&format!("/res/{}/{}", &h1[..2], h1), a1.clone());
    srv.put(&format!("/res/{}/{}", &h2[..2], h2), a2.clone());
    let h3 = "ab".repeat(20); // never served: asset failure is skipped
    let index = json!({"objects": {
        "minecraft/a.txt": {"hash": h1, "size": a1.len()},
        "b.txt": {"hash": h2, "size": a2.len()},
        "gone.txt": {"hash": h3, "size": 3},
        "evil": {"hash": "../../x", "size": 1}
    }});
    let index_bytes = serde_json::to_vec(&index).unwrap();
    srv.put("/idx.json", index_bytes.clone());
    srv.put("/log.xml", b"<Configuration/>".to_vec());

    let version = json!({
        "id": "1.21", "type": "release", "mainClass": "net.minecraft.client.main.Main",
        "downloads": {"client": {"url": format!("{}/client.jar", srv.base),
            "sha1": sha1(&client).await, "size": client.len()}},
        "assets": "17",
        "assetIndex": {"id": "17", "url": format!("{}/idx.json", srv.base),
            "sha1": sha1(&index_bytes).await, "size": index_bytes.len()},
        "logging": {"client": {"argument": "-Dlog4j.configurationFile=${path}",
            "file": {"id": "client-1.12.xml", "url": format!("{}/log.xml", srv.base), "size": 16}}},
        "libraries": [
            {"name": "a:b:1", "downloads": {"artifact": {"path": "a/b/1/b-1.jar",
                "url": format!("{}/lib/a.jar", srv.base), "sha1": sha1(&lib).await, "size": lib.len()}}},
            {"name": "c:missing:1", "downloads": {"artifact": {"path": "c/missing/1/missing-1.jar",
                "url": format!("{}/lib/none.jar", srv.base), "size": 4}}},
            {"name": "org.lwjgl:lwjgl:3.3.3:natives-linux", "extract": {"exclude": ["skip/"]},
             "downloads": {"artifact": {"path": "org/lwjgl/n.jar",
                "url": format!("{}/lib/n.jar", srv.base), "sha1": sha1(&native).await, "size": native.len()}}},
            {"name": "org.lwjgl:lwjgl:3.3.3:natives-windows",
             "rules": [{"action": "allow", "os": {"name": "windows"}}],
             "downloads": {"artifact": {"path": "org/lwjgl/w.jar", "url": format!("{}/lib/w.jar", srv.base), "size": 1}}}
        ]
    });
    srv.put("/v/1.21.json", serde_json::to_vec(&version).unwrap());
    srv.put(
        "/manifest.json",
        serde_json::to_vec(
            &json!({"versions": [{"id": "1.21", "url": format!("{}/v/1.21.json", srv.base)}]}),
        )
        .unwrap(),
    );
    Fixture { version }
}

#[tokio::test]
async fn installs_vanilla_end_to_end() {
    let srv = Server::start().await;
    let fx = vanilla_fixture(&srv).await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let (opts, progress, logs) = options(&srv);

    let r = install_version(&paths, "1.21", &opts).await.unwrap();
    assert_eq!(r.version["id"], fx.version["id"]);
    assert_eq!(std::fs::read(&r.client_jar).unwrap(), b"client-jar-bytes");
    // a:b, the (failed) missing lib, and the modern natives jar (26.x finds natives via the classpath)
    assert_eq!(r.classpath.len(), 3);
    assert!(r.classpath.iter().any(|p| p.ends_with("org/lwjgl/n.jar")));
    assert!(r.natives_dir.join("java").is_dir());
    assert!(paths.versions().join("1.21/1.21.json").exists());

    // natives: flat, so/dll/dylib only, no META-INF, excludes honoured
    let mut names: Vec<String> = std::fs::read_dir(&r.natives_dir)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names.retain(|n| !matches!(n.as_str(), "java" | "jna" | "lwjgl" | "netty"));
    assert_eq!(names, [".mangoclient-stamp", "libbar.so", "libfoo.so"]);
    assert_eq!(
        srv.hit_count("/lib/w.jar"),
        0,
        "foreign natives never downloaded"
    );

    // missing library logged, install carries on
    assert!(logs
        .lock()
        .unwrap()
        .iter()
        .any(|l| l.contains("Library failed: c:missing:1")));
    // assets: good ones stored, bad ones skipped
    assert!(paths
        .asset_objects()
        .join(&sha1(b"asset-one").await[..2])
        .is_dir());
    let logs = logs.lock().unwrap().clone();
    assert!(logs.iter().any(|l| l.contains("Asset failed: gone.txt")));
    assert!(logs.iter().any(|l| l.contains("Asset failed: evil")));
    assert_eq!(r.assets_index_id, "17");
    assert!(!r.is_legacy_assets);
    assert!(paths.asset_indexes().join("17.json").exists());
    let arg = r.logging_arg.unwrap();
    assert!(arg.starts_with("-Dlog4j.configurationFile=") && arg.ends_with("client-1.12.xml"));
    assert!(paths.asset_log_configs().join("client-1.12.xml").exists());

    let p = progress.lock().unwrap().clone();
    let phases: Vec<Phase> = p.iter().map(|e| e.phase).collect();
    for ph in [Phase::Client, Phase::Libraries, Phase::Assets, Phase::Done] {
        assert!(phases.contains(&ph));
    }
    let last_lib = p
        .iter()
        .rev()
        .find(|e| e.phase == Phase::Libraries)
        .unwrap();
    assert_eq!((last_lib.done, last_lib.total), (3, 3));

    // second run: everything valid, nothing re-downloaded
    let before = srv.hit_count("/client.jar") + srv.hit_count("/lib/a.jar");
    install_version(&paths, "1.21", &opts).await.unwrap();
    assert_eq!(
        before,
        srv.hit_count("/client.jar") + srv.hit_count("/lib/a.jar")
    );
}

#[tokio::test]
async fn natives_stamp_guards_and_wipes() {
    let dir = tempfile::tempdir().unwrap();
    let archive = dir.path().join("n.zip");
    std::fs::write(&archive, zip_of(&[("a.dll", b"1")])).unwrap();
    let lib = |sha: &str| LibraryFile {
        name: "n:n:1".into(),
        url: String::new(),
        path: archive.clone(),
        sha1: Some(sha.into()),
        size: None,
        exclude: vec![],
    };
    let root = dir.path().join("natives");
    let log = |_: &str| {};
    let d = extract_natives(&root, "v", &[lib("s1")], &log)
        .await
        .unwrap();
    std::fs::write(d.join("stray.so"), b"x").unwrap();
    extract_natives(&root, "v", &[lib("s1")], &log)
        .await
        .unwrap();
    assert!(d.join("stray.so").exists(), "same stamp keeps directory");
    extract_natives(&root, "v", &[lib("s2")], &log)
        .await
        .unwrap();
    assert!(!d.join("stray.so").exists(), "stamp change wipes");
    assert!(d.join("a.dll").exists());
}

#[tokio::test]
async fn loader_without_client_download_copies_parent_jar() {
    let srv = Server::start().await;
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let write = |id: &str, v: Value| {
        let d = paths.versions().join(id);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join(format!("{id}.json")),
            serde_json::to_vec(&v).unwrap(),
        )
        .unwrap();
        d
    };
    let pd = write(
        "1.21",
        json!({"id": "1.21", "mainClass": "V", "libraries": []}),
    );
    std::fs::write(pd.join("1.21.jar"), b"parent-jar").unwrap();
    write(
        "fabric-loader-0.16-1.21",
        json!({"id": "fabric-loader-0.16-1.21", "inheritsFrom": "1.21", "mainClass": "K", "libraries": []}),
    );
    let (opts, _, _) = options(&srv);
    let r = install_version(&paths, "fabric-loader-0.16-1.21", &opts)
        .await
        .unwrap();
    assert_eq!(std::fs::read(&r.client_jar).unwrap(), b"parent-jar");
    assert_eq!(r.version["mainClass"], "K");
    assert_eq!(r.assets_index_id, "legacy");
    assert!(r.is_legacy_assets);
}

#[tokio::test]
async fn legacy_assets_are_copied_to_virtual_and_resources() {
    for (flag, check) in [
        ("virtual", "assets/virtual/pre-1.6/icons/i.png"),
        ("map_to_resources", "assets/resources/icons/i.png"),
    ] {
        let srv = Server::start().await;
        let body = b"icon".to_vec();
        let h = sha1(&body).await;
        srv.put(&format!("/res/{}/{}", &h[..2], h), body);
        let mut index = json!({"objects": {"icons/i.png": {"hash": h, "size": 4}, "../escape": {"hash": h, "size": 4}}});
        index[flag] = json!(true);
        srv.put("/idx.json", serde_json::to_vec(&index).unwrap());
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        let d = paths.versions().join("old");
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(
            d.join("old.json"),
            serde_json::to_vec(&json!({"id": "old", "mainClass": "M", "libraries": [],
                "assetIndex": {"id": "pre-1.6", "url": format!("{}/idx.json", srv.base)}}))
            .unwrap(),
        )
        .unwrap();
        let (opts, _, logs) = options(&srv);
        let r = install_version(&paths, "old", &opts).await.unwrap();
        assert!(r.is_legacy_assets);
        assert_eq!(std::fs::read(dir.path().join(check)).unwrap(), b"icon");
        assert!(!dir.path().join("assets/escape").exists());
        assert!(logs
            .lock()
            .unwrap()
            .iter()
            .any(|l| l.contains("unsafe asset name")));
    }
}

#[tokio::test]
async fn unknown_and_circular_versions_error() {
    let srv = Server::start().await;
    srv.put("/manifest.json", br#"{"versions":[]}"#.to_vec());
    let dir = tempfile::tempdir().unwrap();
    let paths = Paths::new(dir.path());
    let (opts, _, _) = options(&srv);
    let e = install_version(&paths, "9.9", &opts).await.unwrap_err();
    assert_eq!(e.code(), "minecraft.unknown_version");
    let e = install_version(&paths, "../x", &opts).await.unwrap_err();
    assert_eq!(e.code(), "minecraft.invalid_version_id");

    for id in ["a", "b"] {
        let d = paths.versions().join(id);
        std::fs::create_dir_all(&d).unwrap();
        let other = if id == "a" { "b" } else { "a" };
        std::fs::write(
            d.join(format!("{id}.json")),
            format!(r#"{{"id":"{id}","inheritsFrom":"{other}"}}"#),
        )
        .unwrap();
    }
    let e = install_version(&paths, "a", &opts).await.unwrap_err();
    assert_eq!(e.code(), "minecraft.circular_inheritance");
}
